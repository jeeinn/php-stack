use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;

use super::backup_engine::BackupEngine;
use super::backup_manifest::{check_manifest_version, BackupManifest};
use super::env_parser::EnvFile;
use super::site_manager::{self, ManifestSite};
use super::user_config;

/// 恢复预览信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub manifest: BackupManifest,
    pub file_count: usize,
    /// 备份包中声明的宿主机端口与本机当前占用的冲突列表（提示改端口，不阻断恢复）
    pub port_conflicts: Vec<PortConflict>,
}

/// 端口冲突：备份包要求的宿主机端口已被占用
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortConflict {
    pub service: String,
    pub port: u16,
    pub suggested_port: u16,
}

/// 恢复进度事件
#[derive(Debug, Clone, Serialize)]
pub struct RestoreProgress {
    pub step: String,
    pub percentage: u8,
}

/// 恢复时用户为每个站点指定的新宿主机路径。
///
/// `skipped` 为真表示明确留空，允许继续恢复，但不会把源码解到该路径。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SitePathOverride {
    pub env_key: String,
    pub host_path: String,
    #[serde(default)]
    pub skipped: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub success: bool,
    pub restored_files: Vec<String>,
    pub errors: Vec<String>,
    /// 恢复开始前自动生成的回滚包路径（R2）。
    ///
    /// 恢复是逐文件覆盖现有配置的破坏性操作，中途失败会留下半恢复状态。
    /// 该字段指向恢复前的自动备份，用户可用它一键回退。
    /// 由调用方 `commands::execute_restore` 在创建回滚包后填入。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_path: Option<String>,
}

pub struct RestoreEngine;

/// 校验 ZIP 条目名是否安全（zip-slip 路径遍历防护）
///
/// **刻意不使用 `std::path` 的平台语义**：`Path::new("C:\\Windows\\evil.txt")`
/// 在 Windows 上是绝对路径，在 Linux 上却只是一个普通文件名 —— 而备份包是跨
/// 平台流转的，同一份包的安全判定不能随解压机器漂移（在 Linux 上放行、到
/// Windows 上恢复才会触发的漏洞，等于没有防护）。
///
/// 改为按 ZIP 规范自行判定：APPNOTE 4.4.17 规定条目名以 `/` 为分隔符，
/// 不得含盘符、设备名与反斜杠。三平台行为一致。
///
/// 拒绝四类条目：
/// - 反斜杠（`C:\...`、`\\server\share\...`）—— 规范不允许，且是 Windows 分隔符
/// - Windows 盘符前缀（`C:/...`、`C:evil.txt`）
/// - 绝对路径与 UNC（`/etc/passwd`、`//server/share`）
/// - 含父目录引用（`../`）
///
/// 冒号只在开头按盘符判定，不整体禁止：Unix 文件名允许含冒号
/// （`report:v2.txt`），一刀切会让合法备份无法恢复。
fn is_safe_entry_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\\') || name.starts_with('/') {
        return false;
    }
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !name.split('/').any(|seg| seg == "..")
}

/// 扫描 ZIP 全部条目名；任一非法则整体拒绝（写盘前 / 预览前均可调用）。
fn validate_archive_entry_names<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<(), String> {
    for i in 0..archive.len() {
        let name = archive
            .by_index(i)
            .map_err(|e| format!("failed to read ZIP entry: {e}"))?
            .name()
            .to_string();
        if !is_safe_entry_name(&name) {
            return Err(format!(
                "backup contains an entry with an illegal path, restore rejected: {name}"
            ));
        }
    }
    Ok(())
}

/// 检测宿主机端口是否可绑定（被 Docker / 其它进程占用则视为冲突）
fn is_host_port_free(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// 从 start+1 起找一个未被 reserved 且可绑定的端口
fn find_suggested_port<F>(start: u16, reserved: &HashSet<u16>, is_free: &F) -> u16
where
    F: Fn(u16) -> bool,
{
    let mut candidate = start.saturating_add(1).max(1);
    for _ in 0..200 {
        if !reserved.contains(&candidate) && is_free(candidate) {
            return candidate;
        }
        candidate = candidate.wrapping_add(1).max(1);
    }
    start.saturating_add(1).max(1)
}

/// 读取 manifest.services 的宿主机端口，与本机占用比对；冲突时给出建议端口。
///
/// 仅作预览提示——恢复仍写入原配置；用户可在启动前手动改端口。
fn detect_port_conflicts(manifest: &BackupManifest) -> Vec<PortConflict> {
    detect_port_conflicts_with(manifest, is_host_port_free)
}

fn detect_port_conflicts_with<F>(manifest: &BackupManifest, is_free: F) -> Vec<PortConflict>
where
    F: Fn(u16) -> bool,
{
    let mut claimed: HashSet<u16> = HashSet::new();
    let mut host_ports: Vec<(String, u16)> = Vec::new();

    for svc in &manifest.services {
        for &host_port in svc.ports.keys() {
            host_ports.push((svc.name.clone(), host_port));
            claimed.insert(host_port);
        }
    }

    let mut conflicts = Vec::new();
    let mut reserved = claimed;

    for (service, port) in host_ports {
        if is_free(port) {
            continue;
        }
        let suggested = find_suggested_port(port, &reserved, &is_free);
        reserved.insert(suggested);
        conflicts.push(PortConflict {
            service,
            port,
            suggested_port: suggested,
        });
    }

    conflicts
}

/// 旧备份没有 `sites` 时，用 `.env` 的 `SOURCE_DIR` 合成一条默认站。
fn fill_missing_sites(manifest: &mut BackupManifest, env_text: Option<&str>) {
    if !manifest.sites.is_empty() {
        return;
    }
    let host = env_text
        .and_then(|text| EnvFile::parse(text).ok())
        .and_then(|env| {
            env.get(site_manager::PRIMARY_ENV_KEY)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "./www".to_string());
    manifest.sites = vec![site_manager::synthetic_site(&host)];
}

fn read_entry_string<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some(text)
}

fn ensure_absolute_paths_mapped(
    sites: &[ManifestSite],
    overrides: &[SitePathOverride],
) -> Result<(), String> {
    let missing: Vec<String> = sites
        .iter()
        .filter(|site| site.kind == site_manager::KIND_ABSOLUTE)
        .filter(
            |site| match overrides.iter().find(|item| item.env_key == site.env_key) {
                Some(item) if item.skipped => false,
                Some(item) if !item.host_path.trim().is_empty() => false,
                _ => true,
            },
        )
        .map(|site| format!("{} ({})", site.server_name, site.host_path))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "absolute site paths must be remapped before restore: {}",
            missing.join(", ")
        ))
    }
}

fn apply_path_overrides(project_root: &Path, overrides: &[SitePathOverride]) -> Result<(), String> {
    if overrides.is_empty() {
        return Ok(());
    }
    let path = project_root.join(".env");
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("failed to read .env: {e}"))?;
    let mut env = EnvFile::parse(&text).map_err(|e| format!("failed to parse .env: {e}"))?;
    for item in overrides {
        if item.skipped {
            env.set(&item.env_key, "");
        } else if !item.host_path.trim().is_empty() {
            env.set(
                &item.env_key,
                &site_manager::normalize_host_path(&item.host_path),
            );
        }
    }
    std::fs::write(&path, env.format()).map_err(|e| format!("failed to write remapped .env: {e}"))
}

fn warn_missing_absolute_dirs(
    sites: &[ManifestSite],
    project_root: &Path,
    overrides: &[SitePathOverride],
) -> Vec<String> {
    let mut warnings = Vec::new();
    for site in sites
        .iter()
        .filter(|site| site.kind == site_manager::KIND_ABSOLUTE)
    {
        let Some(item) = overrides.iter().find(|item| item.env_key == site.env_key) else {
            continue;
        };
        if item.skipped || item.host_path.trim().is_empty() {
            warnings.push(format!(
                "site {} path left empty; source files were not extracted",
                site.id
            ));
            continue;
        }
        let dest = site_manager::resolve_host_path(project_root, &item.host_path);
        if !dest.exists() {
            if let Err(e) = std::fs::create_dir_all(&dest) {
                warnings.push(format!(
                    "site {} failed to create directory {}: {e}",
                    site.id,
                    dest.display()
                ));
            }
        }
    }
    warnings
}

impl RestoreEngine {
    /// Parse backup ZIP and return preview info.
    /// Reads manifest.json from ZIP, detects port conflicts, counts files.
    pub fn preview(zip_path: &str) -> Result<RestorePreview, String> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("failed to open backup file: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("failed to parse ZIP file: {e}"))?;

        // 预览阶段即做 zip-slip 扫描，恶意包不必等到执行恢复才暴露
        validate_archive_entry_names(&mut archive)?;

        // Read manifest.json
        let mut manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;
        let env_text = read_entry_string(&mut archive, ".env");
        fill_missing_sites(&mut manifest, env_text.as_deref());

        // Count total files in ZIP (excluding manifest.json itself)
        let file_count = (0..archive.len())
            .filter(|i| {
                archive
                    .by_index(*i)
                    .map(|f| f.name() != "manifest.json")
                    .unwrap_or(false)
            })
            .count();

        Ok(RestorePreview {
            manifest: manifest.clone(),
            file_count,
            port_conflicts: detect_port_conflicts(&manifest),
        })
    }

    /// Verify backup integrity by checking SHA256 checksums.
    /// For each file in manifest.files, read from ZIP and compute SHA256,
    /// compare with recorded hash.
    pub fn verify_integrity(zip_path: &str) -> Result<bool, String> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("failed to open backup file: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("failed to parse ZIP file: {e}"))?;

        validate_archive_entry_names(&mut archive)?;

        let manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;

        for (file_path, expected_hash) in &manifest.files {
            let mut zip_file = archive
                .by_name(file_path)
                .map_err(|e| format!("failed to read ZIP entry '{file_path}': {e}"))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("failed to read file content '{file_path}': {e}"))?;

            let actual_hash = BackupEngine::compute_sha256(&content);
            if &actual_hash != expected_hash {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Execute restore operation.
    /// port_overrides: map of service_name -> new_port for conflict resolution.
    pub async fn restore(
        zip_path: &str,
        project_root: &Path,
        app_handle: Option<&tauri::AppHandle>,
        path_overrides: &[SitePathOverride],
    ) -> Result<RestoreResult, String> {
        let mut restored_files: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("failed to open backup file: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("failed to parse ZIP file: {e}"))?;

        // Step 0: 安全校验——拒绝包含路径遍历条目的备份包，在任何写盘操作之前拦截
        validate_archive_entry_names(&mut archive)?;

        // Step 1: Read manifest
        Self::emit_progress(app_handle, "restore.progress.steps.parsing", 5);
        let mut manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;
        let env_text = read_entry_string(&mut archive, ".env");
        fill_missing_sites(&mut manifest, env_text.as_deref());
        ensure_absolute_paths_mapped(&manifest.sites, path_overrides)?;

        // Step 2: Extract .env
        Self::emit_progress(app_handle, "restore.progress.steps.envConfig", 15);
        match Self::restore_env_file(&mut archive, project_root) {
            Ok(()) => {
                restored_files.push(".env".to_string());
                if let Err(e) = apply_path_overrides(project_root, path_overrides) {
                    errors.push(e);
                }
                errors.extend(warn_missing_absolute_dirs(
                    &manifest.sites,
                    project_root,
                    path_overrides,
                ));
            }
            Err(e) => errors.push(format!("failed to restore .env: {e}")),
        }

        // Step 3: Extract docker-compose.yml
        Self::emit_progress(app_handle, "restore.progress.steps.dockerConfig", 25);
        match Self::extract_file_to_path(
            &mut archive,
            "docker-compose.yml",
            &project_root.join("docker-compose.yml"),
        ) {
            Ok(()) => restored_files.push("docker-compose.yml".to_string()),
            Err(e) => errors.push(format!("failed to restore docker-compose.yml: {e}")),
        }

        // Step 4: Extract services/ directory contents
        Self::emit_progress(app_handle, "restore.progress.steps.serviceConfig", 40);
        match Self::extract_prefix(&mut archive, "services/", &project_root.join("services")) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("failed to restore services/: {e}")),
        }

        // Step 4.5: Restore user custom configuration files
        Self::emit_progress(app_handle, "restore.progress.steps.userConfig", 45);

        for file_name in [
            user_config::MIRROR_CONFIG,
            user_config::VERSION_OVERRIDES,
            user_config::SITES,
        ] {
            let zip_name = user_config::relative(file_name);
            if let Ok(()) = Self::extract_file_to_path(
                &mut archive,
                &zip_name,
                &user_config::path(project_root, file_name),
            ) {
                restored_files.push(zip_name);
            }
        }

        // Step 5: Extract vhosts/ to services/nginx/conf.d/
        Self::emit_progress(app_handle, "restore.progress.steps.vhost", 55);
        match Self::extract_prefix(
            &mut archive,
            "vhosts/",
            &project_root.join("services/nginx/conf.d"),
        ) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("failed to restore vhosts/: {e}")),
        }

        // Step 6: 工作区外站点解到用户新选的目录；其余 projects/ 仍解到工作区。
        Self::emit_progress(app_handle, "restore.progress.steps.projectFiles", 70);
        let pack_projects = manifest.options.include_projects
            && (!manifest.options.site_ids.is_empty()
                || !manifest.options.project_patterns.is_empty()
                || manifest.options.pack_full_tree);
        if pack_projects {
            match Self::restore_external_site_files(
                &mut archive,
                project_root,
                &manifest.sites,
                path_overrides,
            ) {
                Ok(files) => restored_files.extend(files),
                Err(e) => errors.push(e),
            }
            match Self::extract_prefix_excluding(
                &mut archive,
                "projects/",
                project_root,
                &["projects/sites/"],
            ) {
                Ok(files) => restored_files.extend(files),
                Err(e) => errors.push(format!("failed to restore project files: {e}")),
            }
        }

        // Step 7: Extract database/ SQL files
        Self::emit_progress(app_handle, "restore.progress.steps.database", 85);
        match Self::extract_prefix(&mut archive, "database/", &project_root.join("database")) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("failed to restore database files: {e}")),
        }

        // Step 8: Done
        Self::emit_progress(app_handle, "restore.progress.steps.done", 100);

        let _ = manifest; // manifest was used for reading

        Ok(RestoreResult {
            success: errors.is_empty(),
            restored_files,
            errors,
            // 由调用方（commands::execute_restore）在创建回滚包后填入
            rollback_path: None,
        })
    }

    /// Read manifest.json from a ZIP archive.
    fn read_manifest_from_archive<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
    ) -> Result<BackupManifest, String> {
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|e| format!("failed to read manifest.json: {e}"))?;

        let mut manifest_json = String::new();
        manifest_file
            .read_to_string(&mut manifest_json)
            .map_err(|e| format!("failed to read manifest.json content: {e}"))?;

        BackupManifest::deserialize(&manifest_json)
    }

    /// Extract a single file from ZIP to a target path.
    fn extract_file_to_path<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        zip_entry: &str,
        target_path: &Path,
    ) -> Result<(), String> {
        let mut zip_file = archive
            .by_name(zip_entry)
            .map_err(|e| format!("failed to read ZIP entry '{zip_entry}': {e}"))?;

        let mut content = Vec::new();
        zip_file
            .read_to_end(&mut content)
            .map_err(|e| format!("failed to read file content '{zip_entry}': {e}"))?;

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory: {e}"))?;
        }

        std::fs::write(target_path, &content)
            .map_err(|e| format!("failed to write file '{}': {}", target_path.display(), e))?;

        Ok(())
    }

    /// Extract all files with a given prefix from ZIP to a target directory.
    /// Returns the list of extracted file paths (relative to the prefix).
    fn extract_prefix<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        prefix: &str,
        target_dir: &Path,
    ) -> Result<Vec<String>, String> {
        let mut extracted = Vec::new();

        // Collect matching file names first to avoid borrow issues
        let matching_entries: Vec<(usize, String)> = (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                let name = file.name().to_string();
                if name.starts_with(prefix) && name.len() > prefix.len() && !name.ends_with('/') {
                    Some((i, name))
                } else {
                    None
                }
            })
            .collect();

        for (idx, name) in matching_entries {
            let relative = &name[prefix.len()..];
            let target_path = target_dir.join(relative);

            let mut zip_file = archive
                .by_index(idx)
                .map_err(|e| format!("failed to read ZIP entry '{name}': {e}"))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("failed to read file content '{name}': {e}"))?;

            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("failed to create directory '{}': {}", parent.display(), e)
                })?;
            }

            std::fs::write(&target_path, &content)
                .map_err(|e| format!("failed to write file '{}': {}", target_path.display(), e))?;

            extracted.push(name);
        }

        Ok(extracted)
    }

    /// 与 [`extract_prefix`] 相同，但跳过指定前缀（用于把 `projects/sites/` 留给站点重映射）。
    fn extract_prefix_excluding<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        prefix: &str,
        target_dir: &Path,
        exclude_prefixes: &[&str],
    ) -> Result<Vec<String>, String> {
        let mut extracted = Vec::new();
        let matching_entries: Vec<(usize, String)> = (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                let name = file.name().to_string();
                if !name.starts_with(prefix) || name.len() <= prefix.len() || name.ends_with('/') {
                    return None;
                }
                if exclude_prefixes.iter().any(|skip| name.starts_with(skip)) {
                    return None;
                }
                Some((i, name))
            })
            .collect();

        for (idx, name) in matching_entries {
            let relative = &name[prefix.len()..];
            let target_path = target_dir.join(relative);
            let mut zip_file = archive
                .by_index(idx)
                .map_err(|e| format!("failed to read ZIP entry '{name}': {e}"))?;
            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("failed to read file content '{name}': {e}"))?;
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("failed to create directory '{}': {}", parent.display(), e)
                })?;
            }
            std::fs::write(&target_path, &content)
                .map_err(|e| format!("failed to write file '{}': {}", target_path.display(), e))?;
            extracted.push(name);
        }
        Ok(extracted)
    }

    fn restore_external_site_files<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        project_root: &Path,
        sites: &[ManifestSite],
        overrides: &[SitePathOverride],
    ) -> Result<Vec<String>, String> {
        let mut restored = Vec::new();
        for site in sites
            .iter()
            .filter(|site| site.kind == site_manager::KIND_ABSOLUTE)
        {
            let Some(item) = overrides.iter().find(|item| item.env_key == site.env_key) else {
                continue;
            };
            if item.skipped || item.host_path.trim().is_empty() {
                continue;
            }
            let dest = site_manager::resolve_host_path(project_root, &item.host_path);
            if let Err(e) = std::fs::create_dir_all(&dest) {
                return Err(format!(
                    "failed to create site {} directory {}: {e}",
                    site.id,
                    dest.display()
                ));
            }
            let prefix = format!("{}/", site_manager::site_zip_prefix(site));
            let files = Self::extract_prefix(archive, &prefix, &dest)?;
            restored.extend(files);
        }
        Ok(restored)
    }

    /// Restore .env file from backup.
    fn restore_env_file<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        project_root: &Path,
    ) -> Result<(), String> {
        let mut zip_file = archive
            .by_name(".env")
            .map_err(|e| format!("failed to read .env: {e}"))?;

        let mut content = String::new();
        zip_file
            .read_to_string(&mut content)
            .map_err(|e| format!("failed to read .env content: {e}"))?;

        let env_path = project_root.join(".env");
        std::fs::write(&env_path, content).map_err(|e| format!("failed to write .env: {e}"))?;

        Ok(())
    }

    /// Helper: emit progress event via Tauri.
    ///
    /// `step` 传的是 i18n key（如 `restore.progress.steps.parsing`），
    /// 由前端 `t()` 翻译后再展示，不要在这里拼自然语言。
    fn emit_progress(app_handle: Option<&tauri::AppHandle>, step: &str, percentage: u8) {
        if let Some(handle) = app_handle {
            use tauri::Emitter;
            let _ = handle.emit(
                "restore-progress",
                RestoreProgress {
                    step: step.to_string(),
                    percentage,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::backup_engine::BackupEngine;
    use crate::engine::backup_manifest::{BackupManifest, BackupOptions, ManifestService};
    use crate::engine::user_config;
    use std::collections::HashMap;
    use std::fs;
    use std::io::Write;
    use zip::write::FileOptions;

    /// Feature: restore-security, Property: 恶意条目名必须被拒绝
    #[test]
    fn test_is_safe_entry_name_rejects_traversal() {
        assert!(!is_safe_entry_name("../evil.txt"));
        assert!(!is_safe_entry_name("a/../../evil.txt"));
        assert!(!is_safe_entry_name("/etc/passwd"));
        assert!(!is_safe_entry_name("C:\\Windows\\evil.txt"));
        assert!(!is_safe_entry_name("\\\\server\\share\\evil.txt"));
    }

    #[test]
    fn test_is_safe_entry_name_accepts_normal_paths() {
        assert!(is_safe_entry_name(".env"));
        assert!(is_safe_entry_name("docker-compose.yml"));
        assert!(is_safe_entry_name("services/php82/php.ini"));
        assert!(is_safe_entry_name("projects/www/test/index.php"));
        assert!(is_safe_entry_name(
            "services/php82/sub/dir/conf.d/default.conf"
        ));
    }

    /// Feature: restore-security, Property: 判定必须与解压平台无关
    ///
    /// 回归：原实现依赖 `std::path` 的平台语义 —— `C:\Windows\evil.txt` 在
    /// Windows 上是绝对路径（被拒），在 Linux 上却只是普通文件名（被放行），
    /// 于是同一份备份包的安全边界随机器漂移，CI（ubuntu）上断言失败。
    /// 备份包跨平台流转，防护必须在所有平台上同样生效。
    #[test]
    fn test_is_safe_entry_name_is_platform_independent() {
        // Windows 绝对路径 / UNC：无论在哪台机器上解压都要拒绝
        assert!(!is_safe_entry_name("C:/Windows/evil.txt"));
        assert!(!is_safe_entry_name("C:\\Windows\\evil.txt"));
        assert!(!is_safe_entry_name("\\\\server\\share\\evil.txt"));
        assert!(!is_safe_entry_name("//server/share/evil.txt"));
        // 盘符：即使不含反斜杠也不能放过
        assert!(!is_safe_entry_name("C:evil.txt"));
        // 空条目名
        assert!(!is_safe_entry_name(""));
    }

    /// 拒绝要严，但不能误伤规范允许的写法 —— 备份侧确实会写出这些条目
    #[test]
    fn test_is_safe_entry_name_accepts_zip_conventional_names() {
        // 目录条目带结尾斜杠；重复分隔符来自路径拼接，无害
        assert!(is_safe_entry_name("services/"));
        assert!(is_safe_entry_name("services//php82/php.ini"));
        assert!(is_safe_entry_name("./.env"));
        // ".." 只有在作为独立路径段时才构成遍历
        assert!(is_safe_entry_name("..hidden"));
        assert!(is_safe_entry_name("a..b/config.ini"));
        // Unix 文件名允许含冒号，不能因为防盘符而误拒
        assert!(is_safe_entry_name("report:v2.txt"));
        assert!(is_safe_entry_name("services/php82/notes 2026-09:final.md"));
    }

    /// Feature: restore-security, Property: 含路径遍历条目的 ZIP 必须整体拒绝且不写盘
    #[test]
    fn test_restore_rejects_zip_slip() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let restore_dir = tmp_dir.path().join("restored");
        fs::create_dir_all(&restore_dir).expect("创建恢复目录失败");

        // 构造含 ../ 逃逸条目的恶意 ZIP
        let malicious_zip = tmp_dir.path().join("malicious.zip");
        let file = fs::File::create(&malicious_zip).expect("创建恶意 ZIP 失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("../../escaped.txt", zip_options).unwrap();
        zip.write_all(b"pwned").unwrap();
        zip.finish().unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(RestoreEngine::restore(
            malicious_zip.to_str().unwrap(),
            &restore_dir,
            None,
            &[],
        ));

        assert!(result.is_err(), "含路径遍历条目的备份包必须被拒绝");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("../../escaped.txt"),
            "错误信息应指出非法条目，实际: {err_msg}"
        );

        // 逃逸目标（restore_dir 的上级）不应出现被写入的文件
        let escaped = tmp_dir.path().join("escaped.txt");
        assert!(!escaped.exists(), "逃逸文件不应被写入磁盘");
    }

    /// Feature: restore-security, Property: 预览阶段也必须拒绝 zip-slip（不必等到执行恢复）
    #[test]
    fn test_preview_rejects_zip_slip() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let malicious_zip = tmp_dir.path().join("malicious-preview.zip");
        let file = fs::File::create(&malicious_zip).expect("创建恶意 ZIP 失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(br#"{"version":"1.0.0","timestamp":"t","services":[]}"#)
            .unwrap();
        zip.start_file("../../escaped.txt", zip_options).unwrap();
        zip.write_all(b"pwned").unwrap();
        zip.finish().unwrap();

        let err = RestoreEngine::preview(malicious_zip.to_str().unwrap())
            .expect_err("预览必须拒绝含路径遍历条目的包");
        assert!(
            err.contains("../../escaped.txt"),
            "预览错误应指出非法条目，实际: {err}"
        );
    }

    /// Feature: restore-security, Property: 校验阶段也必须拒绝 zip-slip
    #[test]
    fn test_verify_rejects_zip_slip() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let malicious_zip = tmp_dir.path().join("malicious-verify.zip");
        let file = fs::File::create(&malicious_zip).expect("创建恶意 ZIP 失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("../../escaped.txt", zip_options).unwrap();
        zip.write_all(b"pwned").unwrap();
        zip.finish().unwrap();

        let err = RestoreEngine::verify_integrity(malicious_zip.to_str().unwrap())
            .expect_err("校验必须拒绝含路径遍历条目的包");
        assert!(
            err.contains("../../escaped.txt"),
            "校验错误应指出非法条目，实际: {err}"
        );
    }

    /// Feature: restore-compat, Property: 预览拒绝格式版本过新的备份包
    #[test]
    fn test_preview_rejects_unsupported_manifest_version() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = tmp_dir.path().join("future.zip");
        let file = fs::File::create(&zip_path).expect("创建 ZIP 失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        let mut manifest = BackupManifest::new();
        manifest.version = "2.0.0".to_string();
        let json = manifest.serialize().expect("序列化失败");
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap();

        let err = RestoreEngine::preview(zip_path.to_str().unwrap())
            .expect_err("格式版本过新应在预览阶段拒绝");
        assert!(err.contains("too new"), "实际: {err}");
        assert!(err.contains("2.0.0"), "实际: {err}");
    }

    /// Helper: create a test backup ZIP with manifest and some files.
    fn create_test_backup(dir: &Path) -> String {
        let backup_path = dir.join("test_backup.zip");
        let file = fs::File::create(&backup_path).expect("创建备份文件失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        // Add .env
        let env_content = b"PHP82_VERSION=8.2.27\nMYSQL_HOST_PORT=3306\nSOURCE_DIR=./www\n";
        zip.start_file(".env", zip_options).unwrap();
        zip.write_all(env_content).unwrap();
        let env_hash = BackupEngine::compute_sha256(env_content);

        // Add docker-compose.yml
        let compose_content = b"version: '3'\nservices:\n  php:\n    image: php:8.2\n";
        zip.start_file("docker-compose.yml", zip_options).unwrap();
        zip.write_all(compose_content).unwrap();
        let compose_hash = BackupEngine::compute_sha256(compose_content);

        // Add a services/ config file
        let php_ini_content = b"memory_limit=256M\n";
        zip.start_file("services/php82/php.ini", zip_options)
            .unwrap();
        zip.write_all(php_ini_content).unwrap();
        let php_ini_hash = BackupEngine::compute_sha256(php_ini_content);

        // Add user custom configuration files
        let user_mirror_config_content =
            b"{\"apt\":{\"source\":\"http://mirrors.aliyun.com/debian/\",\"enabled\":true}}";
        let mirror_entry = user_config::relative(user_config::MIRROR_CONFIG);
        zip.start_file(&mirror_entry, zip_options).unwrap();
        zip.write_all(user_mirror_config_content).unwrap();
        let user_mirror_hash = BackupEngine::compute_sha256(user_mirror_config_content);

        let overrides_entry = user_config::relative(user_config::VERSION_OVERRIDES);
        let user_version_overrides_content = b"{\"php\":{\"8.2\":{\"tag\":\"8.2-custom\"}}}";
        zip.start_file(&overrides_entry, zip_options).unwrap();
        zip.write_all(user_version_overrides_content).unwrap();
        let user_version_hash = BackupEngine::compute_sha256(user_version_overrides_content);

        // Build manifest
        let mut files = HashMap::new();
        files.insert(".env".to_string(), env_hash);
        files.insert("docker-compose.yml".to_string(), compose_hash);
        files.insert("services/php82/php.ini".to_string(), php_ini_hash);
        files.insert(mirror_entry, user_mirror_hash);
        files.insert(overrides_entry, user_version_hash);

        let mut ports = HashMap::new();
        ports.insert(3306, 3306);

        let manifest = BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "2025-01-15T10:30:00+08:00".to_string(),
            app_version: "0.1.0".to_string(),
            os_info: "linux".to_string(),
            services: vec![ManifestService {
                name: "mysql".to_string(),
                image: "mysql:8.0".to_string(),
                version: "8.0".to_string(),
                ports,
            }],
            options: BackupOptions {
                include_projects: false,
                project_patterns: Vec::new(),
                include_logs: false,
                site_ids: Vec::new(),
                pack_full_tree: false,
            },
            files,
            errors: Vec::new(),
            sites: Vec::new(),
        };

        let manifest_json = manifest.serialize().expect("序列化 manifest 失败");
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();

        zip.finish().unwrap();
        backup_path.to_str().unwrap().to_string()
    }

    /// Helper: create a test backup ZIP with project files in www/ directory.
    fn create_test_backup_with_projects(dir: &Path) -> String {
        let backup_path = dir.join("test_backup_with_projects.zip");
        let file = fs::File::create(&backup_path).expect("创建备份文件失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        // Add .env
        let env_content = b"PHP82_VERSION=8.2.27\nMYSQL_HOST_PORT=3306\n";
        zip.start_file(".env", zip_options).unwrap();
        zip.write_all(env_content).unwrap();
        let env_hash = BackupEngine::compute_sha256(env_content);

        // Add docker-compose.yml
        let compose_content = b"version: '3'\nservices:\n  php:\n    image: php:8.2\n";
        zip.start_file("docker-compose.yml", zip_options).unwrap();
        zip.write_all(compose_content).unwrap();
        let compose_hash = BackupEngine::compute_sha256(compose_content);

        // Add project files under projects/www/
        let test_php_content = b"<?php echo 'Hello World'; ?>\n";
        zip.start_file("projects/www/test/index.php", zip_options)
            .unwrap();
        zip.write_all(test_php_content).unwrap();
        let test_php_hash = BackupEngine::compute_sha256(test_php_content);

        let readme_content = b"# My Project\n";
        zip.start_file("projects/www/readme.md", zip_options)
            .unwrap();
        zip.write_all(readme_content).unwrap();
        let readme_hash = BackupEngine::compute_sha256(readme_content);

        // Build manifest with project patterns
        let mut files = HashMap::new();
        files.insert(".env".to_string(), env_hash);
        files.insert("docker-compose.yml".to_string(), compose_hash);
        files.insert("projects/www/test/index.php".to_string(), test_php_hash);
        files.insert("projects/www/readme.md".to_string(), readme_hash);

        let manifest = BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "2025-01-15T10:30:00+08:00".to_string(),
            app_version: "0.1.0".to_string(),
            os_info: "linux".to_string(),
            services: Vec::new(),
            options: BackupOptions {
                include_projects: true,
                project_patterns: vec!["www/test/**".to_string(), "www/readme.md".to_string()],
                include_logs: false,
                site_ids: Vec::new(),
                pack_full_tree: false,
            },
            files,
            errors: Vec::new(),
            sites: Vec::new(),
        };

        let manifest_json = manifest.serialize().expect("序列化 manifest 失败");
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();

        zip.finish().unwrap();
        backup_path.to_str().unwrap().to_string()
    }

    #[test]
    fn test_preview_backup() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup(tmp_dir.path());

        let preview = RestoreEngine::preview(&zip_path).expect("预览备份失败");

        // Verify manifest was parsed correctly
        assert_eq!(preview.manifest.version, "1.0.0");
        assert_eq!(preview.manifest.services.len(), 1);
        assert_eq!(preview.manifest.services[0].name, "mysql");

        // Verify file count (5 files: .env, docker-compose.yml, services/php82/php.ini, mirror_config.json, version_overrides.json)
        assert_eq!(
            preview.file_count, 5,
            "Should have 5 files (excluding manifest.json), got {}",
            preview.file_count
        );
    }

    /// Feature: restore-port-conflict, Property: 占用端口应检出并给出互不冲突的建议端口
    #[test]
    fn test_detect_port_conflicts_suggests_free_ports() {
        let mut ports_mysql = HashMap::new();
        ports_mysql.insert(3306, 3306);
        let mut ports_redis = HashMap::new();
        ports_redis.insert(6379, 6379);

        let manifest = BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "t".to_string(),
            app_version: "0.3.1".to_string(),
            os_info: "test".to_string(),
            services: vec![
                ManifestService {
                    name: "mysql".to_string(),
                    image: "mysql:8".to_string(),
                    version: "8".to_string(),
                    ports: ports_mysql,
                },
                ManifestService {
                    name: "redis".to_string(),
                    image: "redis:7".to_string(),
                    version: "7".to_string(),
                    ports: ports_redis,
                },
            ],
            options: BackupOptions {
                include_projects: false,
                project_patterns: Vec::new(),
                include_logs: false,
                site_ids: Vec::new(),
                pack_full_tree: false,
            },
            files: HashMap::new(),
            errors: Vec::new(),
            sites: Vec::new(),
        };

        // 3306/6379/3307 视为占用；建议应跳过这些并互不重复
        let occupied: HashSet<u16> = [3306u16, 3307, 6379].into_iter().collect();
        let conflicts = detect_port_conflicts_with(&manifest, |p| !occupied.contains(&p));

        assert_eq!(conflicts.len(), 2, "两个占用端口都应检出: {conflicts:?}");
        let mysql = conflicts.iter().find(|c| c.service == "mysql").unwrap();
        let redis = conflicts.iter().find(|c| c.service == "redis").unwrap();
        assert_eq!(mysql.port, 3306);
        assert_eq!(mysql.suggested_port, 3308, "应跳过已占用的 3307");
        assert_eq!(redis.port, 6379);
        assert_eq!(redis.suggested_port, 6380);
        assert_ne!(
            mysql.suggested_port, redis.suggested_port,
            "两条建议端口不能撞车"
        );

        // 全部空闲时无冲突
        let none = detect_port_conflicts_with(&manifest, |_| true);
        assert!(none.is_empty());
    }

    #[test]
    fn test_verify_integrity_valid() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup(tmp_dir.path());

        let result = RestoreEngine::verify_integrity(&zip_path).expect("验证完整性失败");
        assert!(result, "Valid backup should pass integrity check");
    }

    #[test]
    fn test_verify_integrity_tampered() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");

        // Create a backup ZIP where the manifest SHA256 doesn't match the actual file content
        let backup_path = tmp_dir.path().join("tampered_backup.zip");
        let file = fs::File::create(&backup_path).expect("创建备份文件失败");
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        // Add .env with one content
        let env_content = b"TAMPERED=true\n";
        zip.start_file(".env", zip_options).unwrap();
        zip.write_all(env_content).unwrap();

        // But record a different SHA256 in manifest (hash of different content)
        let wrong_hash = BackupEngine::compute_sha256(b"original content");
        let mut files = HashMap::new();
        files.insert(".env".to_string(), wrong_hash);

        let manifest = BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "2025-01-15T10:30:00+08:00".to_string(),
            app_version: "0.1.0".to_string(),
            os_info: "linux".to_string(),
            services: Vec::new(),
            options: BackupOptions {
                include_projects: false,
                project_patterns: Vec::new(),
                include_logs: false,
                site_ids: Vec::new(),
                pack_full_tree: false,
            },
            files,
            errors: Vec::new(),
            sites: Vec::new(),
        };

        let manifest_json = manifest.serialize().expect("序列化 manifest 失败");
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();
        zip.finish().unwrap();

        let result = RestoreEngine::verify_integrity(backup_path.to_str().unwrap())
            .expect("验证完整性调用失败");
        assert!(!result, "Tampered backup should fail integrity check");
    }

    #[test]
    fn test_restore_basic() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup(tmp_dir.path());

        // Create a separate restore target directory
        let restore_dir = tmp_dir.path().join("restored");
        fs::create_dir_all(&restore_dir).expect("创建恢复目录失败");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(RestoreEngine::restore(&zip_path, &restore_dir, None, &[]));

        let restore_result = result.expect("恢复操作失败");
        assert!(
            restore_result.success,
            "Restore should succeed, errors: {:?}",
            restore_result.errors
        );

        // Verify .env was restored
        let env_path = restore_dir.join(".env");
        assert!(env_path.exists(), ".env should be restored");
        let env_content = fs::read_to_string(&env_path).expect("读取 .env 失败");
        assert!(
            env_content.contains("PHP82_VERSION=8.2.27"),
            ".env should contain PHP82_VERSION"
        );

        // Verify docker-compose.yml was restored
        let compose_path = restore_dir.join("docker-compose.yml");
        assert!(
            compose_path.exists(),
            "docker-compose.yml should be restored"
        );

        // Verify services/php82/php.ini was restored
        let php_ini_path = restore_dir.join("services/php82/php.ini");
        assert!(
            php_ini_path.exists(),
            "services/php82/php.ini should be restored"
        );
        let php_ini_content = fs::read_to_string(&php_ini_path).expect("读取 php.ini 失败");
        assert!(
            php_ini_content.contains("memory_limit=256M"),
            "php.ini should contain memory_limit"
        );

        let user_mirror_path = user_config::path(&restore_dir, user_config::MIRROR_CONFIG);
        assert!(
            user_mirror_path.exists(),
            ".user-config/mirror_config.json should be restored"
        );
        let user_mirror_content =
            fs::read_to_string(&user_mirror_path).expect("读取 mirror_config.json 失败");
        assert!(
            user_mirror_content.contains("mirrors.aliyun.com"),
            "mirror_config.json should contain mirror source"
        );

        let user_version_path = user_config::path(&restore_dir, user_config::VERSION_OVERRIDES);
        assert!(
            user_version_path.exists(),
            ".user-config/version_overrides.json should be restored"
        );
        let user_version_content =
            fs::read_to_string(&user_version_path).expect("读取 version_overrides.json 失败");
        assert!(
            user_version_content.contains("8.2-custom"),
            "version_overrides.json should contain custom version tag"
        );

        // Verify restored_files list
        assert!(
            restore_result.restored_files.contains(&".env".to_string()),
            "restored_files should contain .env"
        );
        assert!(
            restore_result
                .restored_files
                .contains(&"docker-compose.yml".to_string()),
            "restored_files should contain docker-compose.yml"
        );
    }

    #[test]
    fn test_restore_projects_to_correct_location() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup_with_projects(tmp_dir.path());

        // Create a separate restore target directory
        let restore_dir = tmp_dir.path().join("restored");
        fs::create_dir_all(&restore_dir).expect("创建恢复目录失败");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(RestoreEngine::restore(&zip_path, &restore_dir, None, &[]));

        let restore_result = result.expect("恢复操作失败");
        assert!(
            restore_result.success,
            "Restore should succeed, errors: {:?}",
            restore_result.errors
        );

        // Verify that project files are restored to correct location relative to project_root
        // Backup patterns were ["www/test/**", "www/readme.md"]
        // ZIP contains: projects/www/test/index.php, projects/www/readme.md
        // Should restore to: restore_dir/www/test/index.php, restore_dir/www/readme.md
        let index_php_path = restore_dir.join("www/test/index.php");
        assert!(
            index_php_path.exists(),
            "Project file should be restored to www/test/index.php"
        );

        let readme_path = restore_dir.join("www/readme.md");
        assert!(
            readme_path.exists(),
            "Project file should be restored to www/readme.md"
        );

        // Verify content
        let index_content = fs::read_to_string(&index_php_path).expect("读取 index.php 失败");
        assert!(
            index_content.contains("Hello World"),
            "index.php should contain correct content"
        );
    }

    #[test]
    fn absolute_site_requires_remap_and_extracts_outside_workspace() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let backup_path = tmp_dir.path().join("sites.zip");
        let file = fs::File::create(&backup_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        let env_content = b"SOURCE_DIR=./www\nSITE_SHOP=D:/old/shop\n";
        zip.start_file(".env", zip_options).unwrap();
        zip.write_all(env_content).unwrap();
        let env_hash = BackupEngine::compute_sha256(env_content);

        zip.start_file("docker-compose.yml", zip_options).unwrap();
        zip.write_all(b"name: php-stack\nservices: {}\n").unwrap();
        let compose_hash = BackupEngine::compute_sha256(b"name: php-stack\nservices: {}\n");

        let index = b"<?php echo 'shop';\n";
        zip.start_file("projects/sites/shop/index.php", zip_options)
            .unwrap();
        zip.write_all(index).unwrap();
        let index_hash = BackupEngine::compute_sha256(index);

        let mut files = HashMap::new();
        files.insert(".env".to_string(), env_hash);
        files.insert("docker-compose.yml".to_string(), compose_hash);
        files.insert("projects/sites/shop/index.php".to_string(), index_hash);

        let manifest = BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "2025-01-15T10:30:00+08:00".to_string(),
            app_version: "0.1.0".to_string(),
            os_info: "windows".to_string(),
            services: Vec::new(),
            options: BackupOptions {
                include_projects: true,
                project_patterns: Vec::new(),
                include_logs: false,
                site_ids: vec!["shop".to_string()],
                pack_full_tree: true,
            },
            files,
            errors: Vec::new(),
            sites: vec![crate::engine::site_manager::ManifestSite {
                id: "shop".to_string(),
                env_key: "SITE_SHOP".to_string(),
                container_path: "/sites/shop".to_string(),
                host_path: "D:/old/shop".to_string(),
                kind: "absolute".to_string(),
                server_name: "shop.test".to_string(),
                public_dir: String::new(),
            }],
        };
        let manifest_json = manifest.serialize().unwrap();
        zip.start_file("manifest.json", zip_options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();
        zip.finish().unwrap();

        let zip_path = backup_path.to_str().unwrap().to_string();
        let restore_dir = tmp_dir.path().join("workspace");
        fs::create_dir_all(&restore_dir).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();

        let blocked = rt.block_on(RestoreEngine::restore(&zip_path, &restore_dir, None, &[]));
        let blocked = blocked.expect_err("未映射绝对路径时必须拒绝恢复");
        assert!(
            blocked.contains("absolute site paths"),
            "实际错误: {blocked}"
        );
        assert!(!restore_dir.join(".env").exists(), "拒绝恢复时不应写入文件");

        let new_root = tmp_dir.path().join("new-shop");
        fs::create_dir_all(&new_root).unwrap();
        let overrides = vec![SitePathOverride {
            env_key: "SITE_SHOP".to_string(),
            host_path: new_root.to_string_lossy().replace('\\', "/"),
            skipped: false,
        }];
        let restored = rt
            .block_on(RestoreEngine::restore(
                &zip_path,
                &restore_dir,
                None,
                &overrides,
            ))
            .expect("映射后应恢复成功");
        assert!(restored.success, "errors: {:?}", restored.errors);
        assert!(new_root.join("index.php").exists());
        assert!(!restore_dir.join("sites").exists());
        let env = fs::read_to_string(restore_dir.join(".env")).unwrap();
        assert!(env.contains("SITE_SHOP="));
        assert!(!env.contains("D:/old/shop"));
    }
}
