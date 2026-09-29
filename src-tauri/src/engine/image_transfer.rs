//! Docker 镜像 tar 导入导出
//!
//! 收集当前工作区实际可用镜像（image 型 tag + PHP/Nginx 构建产物 + 基础镜像），
//! 经 `docker save` / `docker load` 跨机迁移。旁路 `{stem}.manifest.json` 记录角色与指纹。

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config_extractor::{ConfigExtractor, ImageStatus};
use super::config_generator::{
    resolve_service_image_tag, ConfigGenerator, EnvConfig,
};
use super::image_fingerprint::{
    self, built_image_ref, compose_default_image_name, nginx_fingerprint, php_fingerprint,
    NginxFingerprintInput, PhpFingerprintInput, BUILT_IMAGE_PREFIX,
};
use super::service_catalog::{normalize_service_kind, GeneratorKind, ServiceCatalog};
use super::user_override_manager::UserOverrideManager;
use super::version_manifest::VersionManifest;
use crate::app_log;

/// 镜像在包中的角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageRole {
    /// catalog image 型服务直接使用的 tag
    Image,
    /// PHP/Nginx FROM 基础镜像
    Base,
    /// PHP/Nginx 构建产物（带指纹 tag）
    Built,
}

/// 工作区镜像条目（列表 / 导出用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceImageEntry {
    /// 导出时使用的 docker 引用（目标 tag）
    pub ref_name: String,
    pub role: ImageRole,
    pub service_dir: String,
    pub service_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// 本地是否已有可导出源（目标 tag 或可 retag 的候选）
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// 若目标 tag 不存在但找到候选源，记录源引用（导出前会 docker tag）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// UI 提示（如仅有 base、目标机需 build）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 旁路清单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageTransferManifest {
    pub version: u32,
    pub created_at: String,
    pub images: Vec<ImageTransferManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageTransferManifestEntry {
    pub ref_name: String,
    pub role: ImageRole,
    pub service_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// 进度事件
#[derive(Debug, Clone, Serialize)]
pub struct ImageTransferProgress {
    pub step: String,
    pub percentage: u8,
}

/// 导入结果
#[derive(Debug, Clone, Serialize)]
pub struct ImageImportResult {
    pub loaded_refs: Vec<String>,
    pub message: String,
}

pub struct ImageTransferEngine;

impl ImageTransferEngine {
    /// 列出当前工作区相关镜像（含存在性）。`config` 由调用方从 `.env` 解析。
    pub fn list_workspace_images(
        project_root: &Path,
        config: &EnvConfig,
    ) -> Result<Vec<WorkspaceImageEntry>, String> {
        Self::collect_entries(project_root, config, true)
    }

    /// 纯收集（可跳过 docker 探测，单测用）
    pub fn collect_entries(
        project_root: &Path,
        config: &EnvConfig,
        probe_docker: bool,
    ) -> Result<Vec<WorkspaceImageEntry>, String> {
        let catalog = ServiceCatalog::merged(project_root);
        let override_manager = UserOverrideManager::new(project_root);
        let manifest = VersionManifest::new();
        let (puid, pgid) = detect_uid_gid();
        let (apt_mirror, composer_mirror, github_proxy) =
            ConfigGenerator::effective_mirror_values(project_root);

        let container_images = if probe_docker {
            Self::ps_container_image_map()
        } else {
            BTreeMap::new()
        };

        let mut by_ref: BTreeMap<String, WorkspaceImageEntry> = BTreeMap::new();

        for service in &config.services {
            let kind = normalize_service_kind(&service.service_type);
            let Some(desc) = catalog.get(&kind) else {
                continue;
            };
            let entry =
                ConfigGenerator::lookup_version_entry(&override_manager, &manifest, &kind, &service.version);
            let service_dir = entry.service_dir.clone();
            let base_image = resolve_service_image_tag(
                &override_manager,
                &manifest,
                &kind,
                &service.version,
            );

            match desc.generator {
                GeneratorKind::Image => {
                    let present_info = if probe_docker {
                        ConfigExtractor::check_image_present(&base_image)
                    } else {
                        ImageStatus::Missing {
                            tag: base_image.clone(),
                        }
                    };
                    let (present, size) = match present_info {
                        ImageStatus::Present { size, .. } => (true, size),
                        ImageStatus::Missing { .. } => (false, None),
                    };
                    Self::upsert(
                        &mut by_ref,
                        WorkspaceImageEntry {
                            ref_name: base_image,
                            role: ImageRole::Image,
                            service_dir,
                            service_kind: kind,
                            fingerprint: None,
                            present,
                            size,
                            source_ref: None,
                            note: if present {
                                None
                            } else {
                                Some("missing_local".into())
                            },
                        },
                    );
                }
                GeneratorKind::Php => {
                    let dockerfile_bytes = image_fingerprint::read_dockerfile_bytes(
                        project_root,
                        &service_dir,
                        |rel| ConfigGenerator::locate_template_file(rel).ok(),
                    );
                    let empty: Vec<String> = Vec::new();
                    let extensions = service.extensions.as_ref().unwrap_or(&empty);
                    let fp = php_fingerprint(&PhpFingerprintInput {
                        service_dir: &service_dir,
                        base_image: &base_image,
                        extensions,
                        puid,
                        pgid,
                        apt_mirror: &apt_mirror,
                        composer_mirror: &composer_mirror,
                        github_proxy: &github_proxy,
                        dockerfile_bytes: &dockerfile_bytes,
                    });
                    let built_ref = built_image_ref(&service_dir, &fp);
                    Self::add_base_entry(
                        &mut by_ref,
                        &base_image,
                        &service_dir,
                        &kind,
                        probe_docker,
                    );
                    Self::add_built_entry(
                        &mut by_ref,
                        &built_ref,
                        &service_dir,
                        &kind,
                        Some(fp),
                        &container_images,
                        probe_docker,
                    );
                }
                GeneratorKind::Nginx => {
                    let dockerfile_bytes = image_fingerprint::read_dockerfile_bytes(
                        project_root,
                        &service_dir,
                        |rel| ConfigGenerator::locate_template_file(rel).ok(),
                    );
                    let fp = nginx_fingerprint(&NginxFingerprintInput {
                        service_dir: &service_dir,
                        base_image: &base_image,
                        puid,
                        pgid,
                        dockerfile_bytes: &dockerfile_bytes,
                    });
                    let built_ref = built_image_ref(&service_dir, &fp);
                    Self::add_base_entry(
                        &mut by_ref,
                        &base_image,
                        &service_dir,
                        &kind,
                        probe_docker,
                    );
                    Self::add_built_entry(
                        &mut by_ref,
                        &built_ref,
                        &service_dir,
                        &kind,
                        Some(fp),
                        &container_images,
                        probe_docker,
                    );
                }
            }
        }

        Ok(by_ref.into_values().collect())
    }

    fn add_base_entry(
        by_ref: &mut BTreeMap<String, WorkspaceImageEntry>,
        base_image: &str,
        service_dir: &str,
        kind: &str,
        probe_docker: bool,
    ) {
        let present_info = if probe_docker {
            ConfigExtractor::check_image_present(base_image)
        } else {
            ImageStatus::Missing {
                tag: base_image.to_string(),
            }
        };
        let (present, size) = match present_info {
            ImageStatus::Present { size, .. } => (true, size),
            ImageStatus::Missing { .. } => (false, None),
        };
        Self::upsert(
            by_ref,
            WorkspaceImageEntry {
                ref_name: base_image.to_string(),
                role: ImageRole::Base,
                service_dir: service_dir.to_string(),
                service_kind: kind.to_string(),
                fingerprint: None,
                present,
                size,
                source_ref: None,
                note: if present {
                    None
                } else {
                    Some("base_missing".into())
                },
            },
        );
    }

    fn add_built_entry(
        by_ref: &mut BTreeMap<String, WorkspaceImageEntry>,
        built_ref: &str,
        service_dir: &str,
        kind: &str,
        fingerprint: Option<String>,
        container_images: &BTreeMap<String, String>,
        probe_docker: bool,
    ) {
        let mut present = false;
        let mut size = None;
        let mut source_ref = None;
        let mut note = None;

        if probe_docker {
            match ConfigExtractor::check_image_present(built_ref) {
                ImageStatus::Present { size: s, .. } => {
                    present = true;
                    size = s;
                }
                ImageStatus::Missing { .. } => {
                    // 回退：compose 历史名 / 运行中容器镜像
                    let candidates = [
                        compose_default_image_name(service_dir),
                        container_images
                            .get(&format!("ps-{service_dir}"))
                            .cloned()
                            .unwrap_or_default(),
                    ];
                    for cand in candidates {
                        if cand.is_empty() {
                            continue;
                        }
                        match ConfigExtractor::check_image_present(&cand) {
                            ImageStatus::Present { size: s, .. } => {
                                present = true;
                                size = s;
                                source_ref = Some(cand);
                                note = Some("retag_from_legacy".into());
                                break;
                            }
                            ImageStatus::Missing { .. } => {}
                        }
                    }
                    if !present {
                        note = Some("built_missing_will_build_on_target".into());
                    }
                }
            }
        }

        Self::upsert(
            by_ref,
            WorkspaceImageEntry {
                ref_name: built_ref.to_string(),
                role: ImageRole::Built,
                service_dir: service_dir.to_string(),
                service_kind: kind.to_string(),
                fingerprint,
                present,
                size,
                source_ref,
                note,
            },
        );
    }

    fn upsert(map: &mut BTreeMap<String, WorkspaceImageEntry>, entry: WorkspaceImageEntry) {
        match map.get(&entry.ref_name) {
            Some(existing) if existing.present && !entry.present => {}
            Some(existing) if existing.role == ImageRole::Built && entry.role != ImageRole::Built => {
            }
            _ => {
                map.insert(entry.ref_name.clone(), entry);
            }
        }
    }

    fn ps_container_image_map() -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        // 用 CLI 避免在已有 tokio runtime 内 block_on bollard
        let output = Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                "name=ps-",
                "--format",
                "{{.Names}} {{.Image}}",
            ])
            .output();
        let Ok(o) = output else {
            return map;
        };
        if !o.status.success() {
            return map;
        }
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let line = line.trim();
            if let Some((name, image)) = line.split_once(' ') {
                let name = name.trim().trim_start_matches('/').to_string();
                let image = image.trim().to_string();
                if name.starts_with("ps-") && !image.is_empty() {
                    map.insert(name, image);
                }
            }
        }
        map
    }

    /// 导出选中的镜像到 .tar（先写临时文件再 rename）
    pub fn export_images(
        project_root: &Path,
        config: &EnvConfig,
        save_path: &str,
        selected_refs: Option<Vec<String>>,
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<(), String> {
        use tauri::Emitter;

        Self::emit_progress(app_handle, "images.progress.collect", 5);
        let entries = Self::list_workspace_images(project_root, config)?;
        if entries.is_empty() {
            return Err("no workspace images to export; apply config first".into());
        }

        let selected: BTreeSet<String> = match selected_refs {
            Some(refs) if !refs.is_empty() => refs.into_iter().collect(),
            _ => entries.iter().map(|e| e.ref_name.clone()).collect(),
        };

        let mut to_export: Vec<WorkspaceImageEntry> = entries
            .into_iter()
            .filter(|e| selected.contains(&e.ref_name))
            .collect();

        if to_export.is_empty() {
            return Err("no images selected for export".into());
        }

        // 仅导出本地存在的；built 缺失时若有 base 仍导出 base
        let missing: Vec<_> = to_export
            .iter()
            .filter(|e| !e.present)
            .map(|e| e.ref_name.clone())
            .collect();
        if !missing.is_empty() {
            // 允许只导出 present 的子集，但若全部缺失则失败
            to_export.retain(|e| e.present);
            if to_export.is_empty() {
                return Err(format!(
                    "selected images are not present locally: {}",
                    missing.join(", ")
                ));
            }
            app_log!(
                warn,
                "engine::image_transfer",
                "Skipping missing images: {}",
                missing.join(", ")
            );
        }

        Self::emit_progress(app_handle, "images.progress.retag", 15);
        for entry in &to_export {
            if let Some(src) = &entry.source_ref {
                if src != &entry.ref_name {
                    Self::docker_tag(src, &entry.ref_name)?;
                }
            }
        }

        let refs: Vec<String> = to_export.iter().map(|e| e.ref_name.clone()).collect();
        let save_path = PathBuf::from(save_path);
        let parent = save_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stem = save_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("php-stack-images");
        let tmp_path = parent.join(format!("{stem}.tmp.tar"));
        let manifest_path = parent.join(format!("{stem}.manifest.json"));

        Self::emit_progress(app_handle, "images.progress.save", 30);
        app_log!(
            info,
            "engine::image_transfer",
            "docker save {} images -> {}",
            refs.len(),
            tmp_path.display()
        );

        let mut cmd = Command::new("docker");
        cmd.arg("save").arg("-o").arg(&tmp_path);
        for r in &refs {
            cmd.arg(r);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000200);
        }
        let status = cmd
            .status()
            .map_err(|e| format!("failed to start docker save: {e}"))?;
        if !status.success() {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "docker save failed (exit {:?})",
                status.code()
            ));
        }

        Self::emit_progress(app_handle, "images.progress.manifest", 85);
        let manifest = ImageTransferManifest {
            version: 1,
            created_at: chrono::Local::now().to_rfc3339(),
            images: to_export
                .iter()
                .map(|e| ImageTransferManifestEntry {
                    ref_name: e.ref_name.clone(),
                    role: e.role.clone(),
                    service_dir: e.service_dir.clone(),
                    fingerprint: e.fingerprint.clone(),
                })
                .collect(),
        };
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("failed to serialize image manifest: {e}"))?;
        fs::write(&manifest_path, manifest_json)
            .map_err(|e| format!("failed to write image manifest: {e}"))?;

        // rename 覆盖目标
        if save_path.exists() {
            fs::remove_file(&save_path)
                .map_err(|e| format!("failed to replace existing tar: {e}"))?;
        }
        fs::rename(&tmp_path, &save_path)
            .map_err(|e| format!("failed to finalize tar ({}): {e}", tmp_path.display()))?;

        Self::emit_progress(app_handle, "images.progress.done", 100);
        if let Some(handle) = app_handle {
            let _ = handle.emit(
                "image-transfer-progress",
                ImageTransferProgress {
                    step: "images.progress.done".into(),
                    percentage: 100,
                },
            );
        }
        Ok(())
    }

    /// 从 .tar 导入镜像
    pub fn import_images(
        tar_path: &str,
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<ImageImportResult, String> {
        use tauri::Emitter;

        let path = Path::new(tar_path);
        if !path.is_file() {
            return Err(format!("tar file not found: {tar_path}"));
        }

        Self::emit_progress(app_handle, "images.progress.load", 20);
        app_log!(
            info,
            "engine::image_transfer",
            "docker load -i {}",
            tar_path
        );

        let mut cmd = Command::new("docker");
        cmd.args(["load", "-i", tar_path]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000200);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("failed to start docker load: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker load failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut loaded_refs = Vec::new();
        for line in stdout.lines() {
            // "Loaded image: repo:tag" or "Loaded image ID: sha256:..."
            if let Some(rest) = line.strip_prefix("Loaded image: ") {
                loaded_refs.push(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("Loaded image ID: ") {
                loaded_refs.push(rest.trim().to_string());
            }
        }

        // 旁路 manifest（若存在）补充 ref 列表
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let manifest_path = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!("{stem}.manifest.json"));
            if manifest_path.is_file() {
                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Ok(m) = serde_json::from_str::<ImageTransferManifest>(&content) {
                        for e in m.images {
                            if !loaded_refs.contains(&e.ref_name) {
                                loaded_refs.push(e.ref_name);
                            }
                        }
                    }
                }
            }
        }

        Self::emit_progress(app_handle, "images.progress.done", 100);
        if let Some(handle) = app_handle {
            let _ = handle.emit(
                "image-transfer-progress",
                ImageTransferProgress {
                    step: "images.progress.done".into(),
                    percentage: 100,
                },
            );
        }

        Ok(ImageImportResult {
            message: format!("Loaded {} image ref(s)", loaded_refs.len()),
            loaded_refs,
        })
    }

    /// 启动前：解析构建型目标镜像是否缺失，并收集 --cache-from 候选。
    /// 若仅有历史 compose 名 / 容器镜像，会先 `docker tag` 到指纹名。
    pub fn build_plan_for_start(
        project_root: &Path,
        config: &EnvConfig,
    ) -> Result<StartBuildPlan, String> {
        let entries = Self::collect_entries(project_root, config, true)?;
        let mut missing_services: Vec<String> = Vec::new();
        let mut cache_from: BTreeSet<String> = BTreeSet::new();

        for e in &entries {
            match e.role {
                ImageRole::Built => {
                    let target_present = matches!(
                        ConfigExtractor::check_image_present(&e.ref_name),
                        ImageStatus::Present { .. }
                    );
                    if !target_present {
                        if let Some(src) = &e.source_ref {
                            match Self::docker_tag(src, &e.ref_name) {
                                Ok(()) => {
                                    // retag 成功则无需 build
                                }
                                Err(err) => {
                                    app_log!(
                                        warn,
                                        "engine::image_transfer",
                                        "retag before start failed: {err}"
                                    );
                                    missing_services.push(e.service_dir.clone());
                                }
                            }
                        } else {
                            missing_services.push(e.service_dir.clone());
                        }
                    }
                    for tag in Self::list_local_tags_for_repo(&format!(
                        "{BUILT_IMAGE_PREFIX}/{}",
                        e.service_dir
                    )) {
                        cache_from.insert(tag);
                    }
                    // 旧 compose 默认名也可作 cache（如 php-stack-php82:latest）
                    let legacy = compose_default_image_name(&e.service_dir);
                    if matches!(
                        ConfigExtractor::check_image_present(&legacy),
                        ImageStatus::Present { .. }
                    ) {
                        cache_from.insert(format!("{legacy}:latest"));
                    }
                }
                ImageRole::Base => {
                    if e.present {
                        cache_from.insert(e.ref_name.clone());
                    }
                }
                ImageRole::Image => {}
            }
        }

        missing_services.sort();
        missing_services.dedup();

        Ok(StartBuildPlan {
            services_needing_build: missing_services,
            cache_from: cache_from.into_iter().collect(),
        })
    }

    /// 写入临时 compose override，用 `build.cache_from`（CLI 无 `--cache-from` 标志）。
    /// 返回 override 路径；调用方负责在 build 结束后删除。
    pub fn write_build_cache_override(
        project_root: &Path,
        plan: &StartBuildPlan,
    ) -> Result<Option<PathBuf>, String> {
        if plan.services_needing_build.is_empty() || plan.cache_from.is_empty() {
            return Ok(None);
        }
        let mut lines: Vec<String> = vec!["services:".into()];
        for svc in &plan.services_needing_build {
            lines.push(format!("  {svc}:"));
            lines.push("    build:".into());
            lines.push("      cache_from:".into());
            for img in &plan.cache_from {
                lines.push(format!("        - {img}"));
            }
        }
        let path = project_root.join(".php-stack-build-cache.yml");
        fs::write(&path, lines.join("\n") + "\n")
            .map_err(|e| format!("failed to write build cache override: {e}"))?;
        Ok(Some(path))
    }

    fn list_local_tags_for_repo(repo: &str) -> Vec<String> {
        let output = Command::new("docker")
            .args([
                "images",
                "--format",
                "{{.Repository}}:{{.Tag}}",
                repo,
            ])
            .output();
        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.ends_with(":<none>"))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn docker_tag(source: &str, target: &str) -> Result<(), String> {
        app_log!(
            info,
            "engine::image_transfer",
            "docker tag {} -> {}",
            source,
            target
        );
        let status = Command::new("docker")
            .args(["tag", source, target])
            .status()
            .map_err(|e| format!("failed to start docker tag: {e}"))?;
        if !status.success() {
            return Err(format!(
                "docker tag {source} {target} failed (exit {:?})",
                status.code()
            ));
        }
        Ok(())
    }

    fn emit_progress(app_handle: Option<&tauri::AppHandle>, step: &str, percentage: u8) {
        use tauri::Emitter;
        if let Some(handle) = app_handle {
            let _ = handle.emit(
                "image-transfer-progress",
                ImageTransferProgress {
                    step: step.to_string(),
                    percentage,
                },
            );
        }
    }
}

/// 启动时可选的 compose build 计划
#[derive(Debug, Clone)]
pub struct StartBuildPlan {
    pub services_needing_build: Vec<String>,
    pub cache_from: Vec<String>,
}

fn detect_uid_gid() -> (u32, u32) {
    #[cfg(target_os = "linux")]
    {
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(1000);
        let gid = std::process::Command::new("id")
            .arg("-g")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(1000);
        (uid, gid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        (1000, 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config_generator::ServiceEntry;

    fn php_mysql_config() -> EnvConfig {
        EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9000,
                    extensions: Some(vec!["gd".into(), "redis".into()]),
                },
                ServiceEntry {
                    service_type: "mysql".to_string(),
                    version: "mysql80".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        }
    }

    #[test]
    fn test_collect_entries_lists_built_and_base_for_php() {
        let tmp = tempfile::tempdir().unwrap();
        let config = php_mysql_config();
        let entries =
            ImageTransferEngine::collect_entries(tmp.path(), &config, false).expect("collect");

        let roles: BTreeSet<_> = entries.iter().map(|e| format!("{:?}", e.role)).collect();
        assert!(
            entries.iter().any(|e| e.role == ImageRole::Built
                && e.service_dir == "php82"
                && e.ref_name.starts_with("php-stack/php82:")),
            "应包含 php82 构建产物，实际: {:?}",
            entries
                .iter()
                .map(|e| (&e.ref_name, &e.role))
                .collect::<Vec<_>>()
        );
        assert!(
            entries
                .iter()
                .any(|e| e.role == ImageRole::Base && e.service_dir == "php82"),
            "应包含 php base"
        );
        assert!(
            entries
                .iter()
                .any(|e| e.role == ImageRole::Image && e.service_kind == "mysql"),
            "应包含 mysql image tag"
        );
        assert!(roles.contains("Built") && roles.contains("Base") && roles.contains("Image"));
    }

    #[test]
    fn test_collect_entries_extension_order_same_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let mut a = php_mysql_config();
        a.services[0].extensions = Some(vec!["gd".into(), "redis".into()]);
        let mut b = php_mysql_config();
        b.services[0].extensions = Some(vec!["redis".into(), "gd".into()]);

        let ea = ImageTransferEngine::collect_entries(tmp.path(), &a, false).unwrap();
        let eb = ImageTransferEngine::collect_entries(tmp.path(), &b, false).unwrap();
        let fa = ea
            .iter()
            .find(|e| e.role == ImageRole::Built)
            .unwrap()
            .ref_name
            .clone();
        let fb = eb
            .iter()
            .find(|e| e.role == ImageRole::Built)
            .unwrap()
            .ref_name
            .clone();
        assert_eq!(fa, fb);
    }
}
