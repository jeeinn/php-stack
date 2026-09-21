use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;

use super::backup_engine::BackupEngine;
use super::backup_manifest::{check_manifest_version, BackupManifest};

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

/// 恢复结果
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
/// 拒绝三类条目：
/// - 绝对路径（`/etc/passwd`、Windows 盘符 `C:\...` 或 UNC `\\...`）
/// - 包含父目录引用（`../`）
/// - 根目录组件
fn is_safe_entry_name(name: &str) -> bool {
    let path = std::path::Path::new(name);
    if path.is_absolute() {
        return false;
    }
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return false,
            _ => {}
        }
    }
    true
}

/// 扫描 ZIP 全部条目名；任一非法则整体拒绝（写盘前 / 预览前均可调用）。
fn validate_archive_entry_names<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<(), String> {
    for i in 0..archive.len() {
        let name = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {e}"))?
            .name()
            .to_string();
        if !is_safe_entry_name(&name) {
            return Err(format!("备份包包含非法路径条目，已拒绝恢复: {name}"));
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

impl RestoreEngine {
    /// Parse backup ZIP and return preview info.
    /// Reads manifest.json from ZIP, detects port conflicts, counts files.
    pub fn preview(zip_path: &str) -> Result<RestorePreview, String> {
        let file = std::fs::File::open(zip_path).map_err(|e| format!("打开备份文件失败: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("解析 ZIP 文件失败: {e}"))?;

        // 预览阶段即做 zip-slip 扫描，恶意包不必等到执行恢复才暴露
        validate_archive_entry_names(&mut archive)?;

        // Read manifest.json
        let manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;

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
        let file = std::fs::File::open(zip_path).map_err(|e| format!("打开备份文件失败: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("解析 ZIP 文件失败: {e}"))?;

        validate_archive_entry_names(&mut archive)?;

        let manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;

        for (file_path, expected_hash) in &manifest.files {
            let mut zip_file = archive
                .by_name(file_path)
                .map_err(|e| format!("读取 ZIP 条目 '{file_path}' 失败: {e}"))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("读取文件内容 '{file_path}' 失败: {e}"))?;

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
    ) -> Result<RestoreResult, String> {
        let mut restored_files: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        let file = std::fs::File::open(zip_path).map_err(|e| format!("打开备份文件失败: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("解析 ZIP 文件失败: {e}"))?;

        // Step 0: 安全校验——拒绝包含路径遍历条目的备份包，在任何写盘操作之前拦截
        validate_archive_entry_names(&mut archive)?;

        // Step 1: Read manifest
        Self::emit_progress(app_handle, "restore.progress.steps.parsing", 5);
        let manifest = Self::read_manifest_from_archive(&mut archive)?;
        check_manifest_version(&manifest.version)?;

        // Step 2: Extract .env
        Self::emit_progress(app_handle, "restore.progress.steps.envConfig", 15);
        match Self::restore_env_file(&mut archive, project_root) {
            Ok(()) => restored_files.push(".env".to_string()),
            Err(e) => errors.push(format!("恢复 .env 失败: {e}")),
        }

        // Step 3: Extract docker-compose.yml
        Self::emit_progress(app_handle, "restore.progress.steps.dockerConfig", 25);
        match Self::extract_file_to_path(
            &mut archive,
            "docker-compose.yml",
            &project_root.join("docker-compose.yml"),
        ) {
            Ok(()) => restored_files.push("docker-compose.yml".to_string()),
            Err(e) => errors.push(format!("恢复 docker-compose.yml 失败: {e}")),
        }

        // Step 4: Extract services/ directory contents
        Self::emit_progress(app_handle, "restore.progress.steps.serviceConfig", 40);
        match Self::extract_prefix(&mut archive, "services/", &project_root.join("services")) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("恢复 services/ 失败: {e}")),
        }

        // Step 4.5: Restore user custom configuration files
        Self::emit_progress(app_handle, "restore.progress.steps.userConfig", 45);

        // .user_mirror_config.json - User mirror source configuration
        match Self::extract_file_to_path(
            &mut archive,
            ".user_mirror_config.json",
            &project_root.join(".user_mirror_config.json"),
        ) {
            Ok(()) => restored_files.push(".user_mirror_config.json".to_string()),
            Err(_) => {
                // Not a critical error, file may not exist in backup
            }
        }

        // .user_version_overrides.json - User version override configuration
        match Self::extract_file_to_path(
            &mut archive,
            ".user_version_overrides.json",
            &project_root.join(".user_version_overrides.json"),
        ) {
            Ok(()) => restored_files.push(".user_version_overrides.json".to_string()),
            Err(_) => {
                // Not a critical error, file may not exist in backup
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
            Err(e) => errors.push(format!("恢复 vhosts/ 失败: {e}")),
        }

        // Step 6: Extract projects/ to project_root (paths are already relative to project_root)
        Self::emit_progress(app_handle, "restore.progress.steps.projectFiles", 70);
        if !manifest.options.project_patterns.is_empty() {
            // 备份时已经将文件路径存储为相对于 project_root 的路径
            // 例如："www/test/index.php" → ZIP 中为 "projects/www/test/index.php"
            // 恢复时直接提取到 project_root 即可
            match Self::extract_prefix(&mut archive, "projects/", project_root) {
                Ok(files) => restored_files.extend(files),
                Err(e) => errors.push(format!("恢复项目文件失败: {e}")),
            }
        }

        // Step 7: Extract database/ SQL files
        Self::emit_progress(app_handle, "restore.progress.steps.database", 85);
        match Self::extract_prefix(&mut archive, "database/", &project_root.join("database")) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("恢复数据库文件失败: {e}")),
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
            .map_err(|e| format!("读取 manifest.json 失败: {e}"))?;

        let mut manifest_json = String::new();
        manifest_file
            .read_to_string(&mut manifest_json)
            .map_err(|e| format!("读取 manifest.json 内容失败: {e}"))?;

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
            .map_err(|e| format!("读取 ZIP 条目 '{zip_entry}' 失败: {e}"))?;

        let mut content = Vec::new();
        zip_file
            .read_to_end(&mut content)
            .map_err(|e| format!("读取文件内容 '{zip_entry}' 失败: {e}"))?;

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }

        std::fs::write(target_path, &content)
            .map_err(|e| format!("写入文件 '{}' 失败: {}", target_path.display(), e))?;

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
                .map_err(|e| format!("读取 ZIP 条目 '{name}' 失败: {e}"))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("读取文件内容 '{name}' 失败: {e}"))?;

            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建目录 '{}' 失败: {}", parent.display(), e))?;
            }

            std::fs::write(&target_path, &content)
                .map_err(|e| format!("写入文件 '{}' 失败: {}", target_path.display(), e))?;

            extracted.push(name);
        }

        Ok(extracted)
    }

    /// Restore .env file from backup.
    fn restore_env_file<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        project_root: &Path,
    ) -> Result<(), String> {
        let mut zip_file = archive
            .by_name(".env")
            .map_err(|e| format!("读取 .env 失败: {e}"))?;

        let mut content = String::new();
        zip_file
            .read_to_string(&mut content)
            .map_err(|e| format!("读取 .env 内容失败: {e}"))?;

        let env_path = project_root.join(".env");
        std::fs::write(&env_path, content).map_err(|e| format!("写入 .env 失败: {e}"))?;

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
        assert!(err.contains("过新"), "实际: {err}");
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
        let env_content = b"PHP82_VERSION=8.2.27\nMYSQL_HOST_PORT=3306\nSOURCE_DIR=/projects\n";
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
        zip.start_file(".user_mirror_config.json", zip_options)
            .unwrap();
        zip.write_all(user_mirror_config_content).unwrap();
        let user_mirror_hash = BackupEngine::compute_sha256(user_mirror_config_content);

        let user_version_overrides_content = b"{\"php\":{\"8.2\":{\"tag\":\"8.2-custom\"}}}";
        zip.start_file(".user_version_overrides.json", zip_options)
            .unwrap();
        zip.write_all(user_version_overrides_content).unwrap();
        let user_version_hash = BackupEngine::compute_sha256(user_version_overrides_content);

        // Build manifest
        let mut files = HashMap::new();
        files.insert(".env".to_string(), env_hash);
        files.insert("docker-compose.yml".to_string(), compose_hash);
        files.insert("services/php82/php.ini".to_string(), php_ini_hash);
        files.insert(".user_mirror_config.json".to_string(), user_mirror_hash);
        files.insert(
            ".user_version_overrides.json".to_string(),
            user_version_hash,
        );

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
            },
            files,
            errors: Vec::new(),
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
            },
            files,
            errors: Vec::new(),
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

        // Verify file count (5 files: .env, docker-compose.yml, services/php82/php.ini, .user_mirror_config.json, .user_version_overrides.json)
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
            },
            files: HashMap::new(),
            errors: Vec::new(),
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
            },
            files,
            errors: Vec::new(),
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
        let result = rt.block_on(RestoreEngine::restore(&zip_path, &restore_dir, None));

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

        // Verify .user_mirror_config.json was restored
        let user_mirror_path = restore_dir.join(".user_mirror_config.json");
        assert!(
            user_mirror_path.exists(),
            ".user_mirror_config.json should be restored"
        );
        let user_mirror_content =
            fs::read_to_string(&user_mirror_path).expect("读取 .user_mirror_config.json 失败");
        assert!(
            user_mirror_content.contains("mirrors.aliyun.com"),
            ".user_mirror_config.json should contain mirror source"
        );

        // Verify .user_version_overrides.json was restored
        let user_version_path = restore_dir.join(".user_version_overrides.json");
        assert!(
            user_version_path.exists(),
            ".user_version_overrides.json should be restored"
        );
        let user_version_content =
            fs::read_to_string(&user_version_path).expect("读取 .user_version_overrides.json 失败");
        assert!(
            user_version_content.contains("8.2-custom"),
            ".user_version_overrides.json should contain custom version tag"
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
        let result = rt.block_on(RestoreEngine::restore(&zip_path, &restore_dir, None));

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
}
