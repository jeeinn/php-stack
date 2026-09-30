//! Docker 镜像 tar 导入导出
//!
//! 收集当前工作区实际可用镜像（image 型 tag + PHP/Nginx 构建产物 + 基础镜像），
//! 经 `docker save` / `docker load` 跨机迁移。旁路 `{stem}.manifest.json` 记录角色与指纹。

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config_extractor::{DockerImageIndex, ImageStatus};
use super::config_generator::{ConfigGenerator, EnvConfig};
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
    /// 本地是否已有指纹目标 tag（可导出）；legacy 仅作 cache 提示，不算 present
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// 指纹 tag 缺失时，本地可用的旧 compose 名 / 容器镜像（仅 cache_from，不 retag）
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

/// 导出结果（含跳过项，避免 UI 误报「全部成功」）
#[derive(Debug, Clone, Serialize)]
pub struct ImageExportResult {
    pub exported: Vec<String>,
    pub skipped: Vec<String>,
    pub tar_path: String,
    pub manifest_path: String,
}

/// `add_built_entry` 入参（避免过多位置参数）
struct BuiltEntryArgs<'a> {
    built_ref: &'a str,
    service_dir: &'a str,
    kind: &'a str,
    fingerprint: Option<String>,
    container_images: &'a BTreeMap<String, String>,
    index: &'a DockerImageIndex,
    probe_docker: bool,
}

/// `docker ps` 可能给出 `sha256:…` / 短 ID，不能当作 cache_from 引用
fn is_image_id_ref(ref_name: &str) -> bool {
    let lower = ref_name.to_ascii_lowercase();
    if lower.starts_with("sha256:") {
        return true;
    }
    // 无仓库分隔符的纯 hex（短/长 image id）
    !ref_name.contains('/')
        && !ref_name.contains(':')
        && ref_name.len() >= 12
        && ref_name.chars().all(|c| c.is_ascii_hexdigit())
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
        // 与 compose build args / generate_compose 指纹同源：优先 .env 的 PUID/PGID
        let (puid, pgid) = ConfigGenerator::effective_uid_gid(project_root);
        let (apt_mirror, composer_mirror, github_proxy) =
            ConfigGenerator::effective_mirror_values(project_root);

        let index = if probe_docker {
            DockerImageIndex::load()
        } else {
            DockerImageIndex::empty()
        };
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
            // 与 generate_compose 同源：lookup_version_entry.image_tag（兜底为 id）
            let entry = ConfigGenerator::lookup_version_entry(
                &override_manager,
                &manifest,
                &kind,
                &service.version,
            );
            let service_dir = entry.service_dir.clone();
            let base_image = entry.image_tag.clone();

            match desc.generator {
                GeneratorKind::Image => {
                    let present_info = if probe_docker {
                        index.check(&base_image)
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
                        &index,
                        probe_docker,
                    );
                    Self::add_built_entry(
                        &mut by_ref,
                        BuiltEntryArgs {
                            built_ref: &built_ref,
                            service_dir: &service_dir,
                            kind: &kind,
                            fingerprint: Some(fp),
                            container_images: &container_images,
                            index: &index,
                            probe_docker,
                        },
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
                        &index,
                        probe_docker,
                    );
                    Self::add_built_entry(
                        &mut by_ref,
                        BuiltEntryArgs {
                            built_ref: &built_ref,
                            service_dir: &service_dir,
                            kind: &kind,
                            fingerprint: Some(fp),
                            container_images: &container_images,
                            index: &index,
                            probe_docker,
                        },
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
        index: &DockerImageIndex,
        probe_docker: bool,
    ) {
        let present_info = if probe_docker {
            index.check(base_image)
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

    /// 仅当指纹 tag 真实存在时标记 present。
    /// 旧 compose 名 / 运行中容器镜像只记入 note + source_ref 作 cache 提示，绝不 retag 到指纹 tag。
    fn add_built_entry(
        by_ref: &mut BTreeMap<String, WorkspaceImageEntry>,
        args: BuiltEntryArgs<'_>,
    ) {
        let mut present = false;
        let mut size = None;
        let mut source_ref = None;
        let mut note = None;

        if args.probe_docker {
            match args.index.check(args.built_ref) {
                ImageStatus::Present { size: s, .. } => {
                    present = true;
                    size = s;
                }
                ImageStatus::Missing { .. } => {
                    let candidates = [
                        compose_default_image_name(args.service_dir),
                        args.container_images
                            .get(&format!("ps-{}", args.service_dir))
                            .cloned()
                            .unwrap_or_default(),
                    ];
                    for cand in candidates {
                        if cand.is_empty() || is_image_id_ref(&cand) {
                            continue;
                        }
                        if matches!(args.index.check(&cand), ImageStatus::Present { .. }) {
                            // 仅作 cache 候选提示，不当作「指纹镜像已就绪」
                            source_ref = Some(cand);
                            note = Some("legacy_cache_available".into());
                            break;
                        }
                    }
                    if note.is_none() {
                        note = Some("built_missing_will_build_on_target".into());
                    }
                }
            }
        }

        Self::upsert(
            by_ref,
            WorkspaceImageEntry {
                ref_name: args.built_ref.to_string(),
                role: ImageRole::Built,
                service_dir: args.service_dir.to_string(),
                service_kind: args.kind.to_string(),
                fingerprint: args.fingerprint,
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
            Some(existing)
                if existing.role == ImageRole::Built && entry.role != ImageRole::Built => {}
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
    ) -> Result<ImageExportResult, String> {
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

        let skipped: Vec<String> = to_export
            .iter()
            .filter(|e| !e.present)
            .map(|e| e.ref_name.clone())
            .collect();
        to_export.retain(|e| e.present);
        if to_export.is_empty() {
            return Err(format!(
                "selected images are not present locally: {}",
                skipped.join(", ")
            ));
        }
        if !skipped.is_empty() {
            app_log!(
                warn,
                "engine::image_transfer",
                "Skipping missing images: {}",
                skipped.join(", ")
            );
        }

        let refs: Vec<String> = to_export.iter().map(|e| e.ref_name.clone()).collect();
        let save_path_buf = PathBuf::from(save_path);
        let parent = save_path_buf
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stem = save_path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("php-stack-images");
        let tmp_path = parent.join(format!("{stem}.tmp.tar"));
        let manifest_path = parent.join(format!("{stem}.manifest.json"));

        // 失败时尽量清掉可能很大的临时 tar
        let cleanup_tmp = |tmp: &Path| {
            let _ = fs::remove_file(tmp);
        };

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
        let status = cmd.status().map_err(|e| {
            cleanup_tmp(&tmp_path);
            format!("failed to start docker save: {e}")
        })?;
        if !status.success() {
            cleanup_tmp(&tmp_path);
            return Err(format!("docker save failed (exit {:?})", status.code()));
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
        let manifest_json = match serde_json::to_string_pretty(&manifest) {
            Ok(j) => j,
            Err(e) => {
                cleanup_tmp(&tmp_path);
                return Err(format!("failed to serialize image manifest: {e}"));
            }
        };
        if let Err(e) = fs::write(&manifest_path, manifest_json) {
            cleanup_tmp(&tmp_path);
            return Err(format!("failed to write image manifest: {e}"));
        }

        if save_path_buf.exists() {
            if let Err(e) = fs::remove_file(&save_path_buf) {
                cleanup_tmp(&tmp_path);
                return Err(format!("failed to replace existing tar: {e}"));
            }
        }
        if let Err(e) = fs::rename(&tmp_path, &save_path_buf) {
            cleanup_tmp(&tmp_path);
            return Err(format!(
                "failed to finalize tar ({}): {e}",
                tmp_path.display()
            ));
        }

        Self::emit_progress(app_handle, "images.progress.done", 100);
        Ok(ImageExportResult {
            exported: refs,
            skipped,
            tar_path: save_path_buf.to_string_lossy().to_string(),
            manifest_path: manifest_path.to_string_lossy().to_string(),
        })
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

    /// 启动前：解析构建型目标镜像是否缺失，并收集 cache_from 候选。
    /// **绝不**把旧镜像 retag 到指纹 tag（避免 tag 说谎）；旧镜像只进 cache_from。
    pub fn build_plan_for_start(
        project_root: &Path,
        config: &EnvConfig,
    ) -> Result<StartBuildPlan, String> {
        let entries = Self::collect_entries(project_root, config, true)?;
        let index = DockerImageIndex::load();
        Ok(Self::plan_from_entries(&entries, Some(&index)))
    }

    /// 从已收集条目计算启动构建计划。
    /// `index` 用于过滤 cache_from：只纳入本地确实存在的引用（`None` 时供单测，信任条目字段）。
    pub fn plan_from_entries(
        entries: &[WorkspaceImageEntry],
        index: Option<&DockerImageIndex>,
    ) -> StartBuildPlan {
        let mut missing_services: Vec<String> = Vec::new();
        let mut cache_from: BTreeSet<String> = BTreeSet::new();

        let push_cache = |set: &mut BTreeSet<String>, ref_name: &str| {
            if ref_name.is_empty() || is_image_id_ref(ref_name) {
                return;
            }
            let candidate = if ref_name.contains(':') {
                ref_name.to_string()
            } else {
                format!("{ref_name}:latest")
            };
            match index {
                Some(idx) => {
                    if matches!(idx.check(ref_name), ImageStatus::Present { .. })
                        || matches!(idx.check(&candidate), ImageStatus::Present { .. })
                    {
                        set.insert(candidate);
                    }
                }
                None => {
                    set.insert(candidate);
                }
            }
        };

        for e in entries {
            match e.role {
                ImageRole::Built => {
                    if !e.present {
                        missing_services.push(e.service_dir.clone());
                    }
                    let repo = format!("{BUILT_IMAGE_PREFIX}/{}", e.service_dir);
                    // 同服务指纹仓库下已有 tag：优先走 index（零额外 CLI）；无 index 时再 docker images
                    match index {
                        Some(idx) => {
                            for tag in idx.refs_for_repository(&repo) {
                                cache_from.insert(tag);
                            }
                        }
                        None => {
                            for tag in Self::list_local_tags_for_repo(&repo) {
                                cache_from.insert(tag);
                            }
                        }
                    }
                    // 旧 compose 默认名 / source_ref：仅在本地存在时纳入（index=None 时信任条目）
                    let legacy = compose_default_image_name(&e.service_dir);
                    push_cache(&mut cache_from, &legacy);
                    if let Some(src) = &e.source_ref {
                        push_cache(&mut cache_from, src);
                    }
                }
                ImageRole::Base => {
                    if e.present {
                        push_cache(&mut cache_from, &e.ref_name);
                    }
                }
                ImageRole::Image => {}
            }
        }

        missing_services.sort();
        missing_services.dedup();

        StartBuildPlan {
            services_needing_build: missing_services,
            cache_from: cache_from.into_iter().collect(),
        }
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
            .args(["images", "--format", "{{.Repository}}:{{.Tag}}", repo])
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

    #[test]
    fn test_plan_from_entries_needs_build_when_fingerprint_missing() {
        let entries = vec![
            WorkspaceImageEntry {
                ref_name: "php-stack/php82:abc".into(),
                role: ImageRole::Built,
                service_dir: "php82".into(),
                service_kind: "php".into(),
                fingerprint: Some("abc".into()),
                present: false,
                size: None,
                source_ref: Some("php-stack-php82".into()),
                note: Some("legacy_cache_available".into()),
            },
            WorkspaceImageEntry {
                ref_name: "php:8.2-fpm".into(),
                role: ImageRole::Base,
                service_dir: "php82".into(),
                service_kind: "php".into(),
                fingerprint: None,
                present: true,
                size: Some("691MB".into()),
                source_ref: None,
                note: None,
            },
        ];
        // index=None：单测信任条目字段，不探 docker
        let plan = ImageTransferEngine::plan_from_entries(&entries, None);
        assert_eq!(plan.services_needing_build, vec!["php82".to_string()]);
        // 旧镜像只进 cache，不会被当成「已就绪」而跳过 build
        assert!(
            plan.cache_from
                .iter()
                .any(|c| c.contains("php-stack-php82"))
                || plan.cache_from.iter().any(|c| c == "php:8.2-fpm"),
            "cache_from 应包含 base 或 legacy，实际: {:?}",
            plan.cache_from
        );
    }

    #[test]
    fn test_plan_from_entries_filters_cache_via_index() {
        let entries = vec![
            WorkspaceImageEntry {
                ref_name: "php-stack/php82:abc".into(),
                role: ImageRole::Built,
                service_dir: "php82".into(),
                service_kind: "php".into(),
                fingerprint: Some("abc".into()),
                present: false,
                size: None,
                source_ref: Some("sha256:deadbeefdeadbeefdeadbeefdeadbeef".into()),
                note: Some("legacy_cache_available".into()),
            },
            WorkspaceImageEntry {
                ref_name: "php:8.2-fpm".into(),
                role: ImageRole::Base,
                service_dir: "php82".into(),
                service_kind: "php".into(),
                fingerprint: None,
                present: true,
                size: None,
                source_ref: None,
                note: None,
            },
        ];
        // 空 index：legacy / sha256 / 未登记 base 均不得进入 cache_from
        let empty = DockerImageIndex::empty();
        let plan = ImageTransferEngine::plan_from_entries(&entries, Some(&empty));
        assert_eq!(plan.services_needing_build, vec!["php82".to_string()]);
        assert!(
            plan.cache_from.is_empty(),
            "空 index 时不应塞入不存在的 cache 源，实际: {:?}",
            plan.cache_from
        );

        // 仅 base + 同仓库旧指纹存在：应过滤掉 sha256 / 缺失的 legacy
        let idx = DockerImageIndex::from_refs(&["php:8.2-fpm", "php-stack/php82:oldfp"]);
        let plan2 = ImageTransferEngine::plan_from_entries(&entries, Some(&idx));
        assert!(plan2.cache_from.iter().any(|c| c == "php:8.2-fpm"));
        assert!(plan2
            .cache_from
            .iter()
            .any(|c| c == "php-stack/php82:oldfp"));
        assert!(!plan2.cache_from.iter().any(|c| c.contains("sha256")));
        assert!(!plan2
            .cache_from
            .iter()
            .any(|c| c.contains("php-stack-php82")));
    }

    #[test]
    fn test_is_image_id_ref() {
        assert!(is_image_id_ref("sha256:abc123"));
        assert!(is_image_id_ref("deadbeefdeadbeef"));
        assert!(!is_image_id_ref("php-stack-php82"));
        assert!(!is_image_id_ref("php:8.2-fpm"));
        assert!(!is_image_id_ref("php-stack/php82:abc"));
    }

    #[test]
    fn test_plan_from_entries_skips_build_when_fingerprint_present() {
        let entries = vec![WorkspaceImageEntry {
            ref_name: "php-stack/php82:abc".into(),
            role: ImageRole::Built,
            service_dir: "php82".into(),
            service_kind: "php".into(),
            fingerprint: Some("abc".into()),
            present: true,
            size: Some("1GB".into()),
            source_ref: None,
            note: None,
        }];
        let plan = ImageTransferEngine::plan_from_entries(&entries, None);
        assert!(plan.services_needing_build.is_empty());
    }

    #[test]
    fn test_write_build_cache_override_emits_cache_from() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = StartBuildPlan {
            services_needing_build: vec!["php82".into(), "nginx128".into()],
            cache_from: vec!["php:8.2-fpm".into(), "php-stack-php82:latest".into()],
        };
        let path = ImageTransferEngine::write_build_cache_override(tmp.path(), &plan)
            .unwrap()
            .expect("should write override");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("php82:"));
        assert!(content.contains("nginx128:"));
        assert!(content.contains("cache_from:"));
        assert!(content.contains("- php:8.2-fpm"));
        assert!(content.contains("- php-stack-php82:latest"));
    }

    #[test]
    fn test_write_build_cache_override_none_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = StartBuildPlan {
            services_needing_build: vec![],
            cache_from: vec!["php:8.2-fpm".into()],
        };
        assert!(
            ImageTransferEngine::write_build_cache_override(tmp.path(), &plan)
                .unwrap()
                .is_none()
        );
    }
}
