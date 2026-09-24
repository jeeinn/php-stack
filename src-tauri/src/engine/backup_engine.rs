use glob::glob;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::Path;
use zip::write::FileOptions;

use super::backup_manifest::{BackupManifest, BackupOptions};
use super::user_config;
use crate::app_log;

/// 备份进度事件
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupProgress {
    pub step: String,
    pub percentage: u8,
}

pub struct BackupEngine;

/// Pattern 必须相对站点根：禁止 `..`、绝对路径与盘符。
fn is_safe_site_relative_pattern(pattern: &str) -> bool {
    let p = pattern.trim().replace('\\', "/");
    if p.is_empty() || p.starts_with('/') {
        return false;
    }
    let bytes = p.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !p.split('/').any(|seg| seg == "..")
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    path.starts_with(&root)
}

/// 计算相对站点根的路径（保留子目录）。失败则返回 Err，禁止扁平成文件名。
fn site_relative_path(path: &Path, site_root: &Path) -> Result<String, String> {
    let canon_path = path
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", path.display()))?;
    let canon_root = site_root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", site_root.display()))?;
    let rel = canon_path.strip_prefix(&canon_root).map_err(|_| {
        format!(
            "path {} is not under site root {}",
            path.display(),
            site_root.display()
        )
    })?;
    let normalized = rel.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() || normalized == "." {
        return Err(format!("empty relative path for {}", path.display()));
    }
    Ok(normalized)
}

/// 无路径分隔符的 pattern（如 `*.local.php`、`.env`）自动加 `**/`，以便匹配子目录。
fn expand_site_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    if normalized.ends_with("/**") {
        normalized.push_str("/*");
    }
    if !normalized.contains('/') {
        normalized = format!("**/{normalized}");
    }
    normalized
}

impl BackupEngine {
    /// Execute complete backup flow.
    /// `app_handle` is `Option` to allow testing without Tauri runtime.
    pub async fn create_backup(
        save_path: &str,
        options: BackupOptions,
        project_root: &Path,
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<(), String> {
        let file = fs::File::create(save_path)
            .map_err(|e| format!("failed to create backup file: {e}"))?;
        let mut zip = zip::ZipWriter::new(file);
        let mut manifest = BackupManifest::new();
        manifest.options = options.clone();

        // Step 1: Pack .env (10%)
        Self::emit_progress(app_handle, "backup.progress.steps.envConfig", 10);
        let env_path = project_root.join(".env");
        if env_path.exists() {
            Self::add_file_to_zip(&mut zip, ".env", &env_path, &mut manifest)?;
        }

        // Step 2: Pack docker-compose.yml (20%)
        Self::emit_progress(app_handle, "backup.progress.steps.dockerConfig", 20);
        let compose_path = project_root.join("docker-compose.yml");
        if compose_path.exists() {
            Self::add_file_to_zip(&mut zip, "docker-compose.yml", &compose_path, &mut manifest)?;
        }

        // Step 3: Pack services/ configs (30%)
        Self::emit_progress(app_handle, "backup.progress.steps.serviceConfig", 30);
        let services_dir = project_root.join("services");
        if services_dir.exists() {
            Self::add_dir_to_zip(&mut zip, &services_dir, "services", &mut manifest)?;
        }

        // Step 3.5: Pack user custom configuration files (35%)
        Self::emit_progress(app_handle, "backup.progress.steps.userConfig", 35);

        for file_name in [
            user_config::MIRROR_CONFIG,
            user_config::VERSION_OVERRIDES,
            user_config::SITES,
        ] {
            let disk_path = user_config::path(project_root, file_name);
            if disk_path.exists() {
                Self::add_file_to_zip(
                    &mut zip,
                    &user_config::relative(file_name),
                    &disk_path,
                    &mut manifest,
                )?;
            }
        }

        manifest.sites = crate::engine::site_manager::collect_manifest_sites(project_root);

        // Step 4: 按所选站点打包本地文件（patterns 相对站点根，或整树）
        if options.include_projects && !options.site_ids.is_empty() {
            Self::emit_progress(app_handle, "backup.progress.steps.projectFiles", 60);
            Self::pack_selected_site_files(
                &mut zip,
                &options,
                project_root,
                &manifest.sites.clone(),
                &mut manifest,
            )?;
        }

        // Step 5: Optional — Recent logs (70%)
        if options.include_logs {
            Self::emit_progress(app_handle, "backup.progress.steps.logs", 85);
            let logs_dir = project_root.join("logs");
            if logs_dir.exists() {
                // MVP: pack all logs (7-day filter can be added later)
                Self::add_dir_to_zip(&mut zip, &logs_dir, "logs", &mut manifest)?;
            }
        }

        // Step 8: Write manifest.json (95%)
        Self::emit_progress(app_handle, "backup.progress.steps.manifest", 95);
        let manifest_json = manifest.serialize()?;
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", zip_options)
            .map_err(|e| format!("failed to create manifest entry: {e}"))?;
        zip.write_all(manifest_json.as_bytes())
            .map_err(|e| format!("failed to write manifest: {e}"))?;

        // Finish ZIP
        zip.finish()
            .map_err(|e| format!("failed to finalize ZIP file: {e}"))?;

        Self::emit_progress(app_handle, "backup.progress.steps.done", 100);
        Ok(())
    }

    /// 在各已选站点的 `host_path` 下按 pattern（或整树）收集文件。
    fn pack_selected_site_files(
        zip: &mut zip::ZipWriter<fs::File>,
        options: &BackupOptions,
        project_root: &Path,
        sites: &[crate::engine::site_manager::ManifestSite],
        manifest: &mut BackupManifest,
    ) -> Result<(), String> {
        for site in sites {
            if !options.site_ids.iter().any(|id| id == &site.id) {
                continue;
            }
            let host =
                crate::engine::site_manager::resolve_host_path(project_root, &site.host_path);
            if !host.is_dir() {
                manifest.errors.push(format!(
                    "site {} source directory does not exist: {}",
                    site.id,
                    host.display()
                ));
                continue;
            }
            let prefix = crate::engine::site_manager::site_zip_prefix(site);

            if options.pack_full_tree {
                if let Err(e) = Self::add_dir_to_zip(zip, &host, &prefix, manifest) {
                    manifest
                        .errors
                        .push(format!("failed to pack site {} files: {e}", site.id));
                }
                continue;
            }

            for pattern in &options.project_patterns {
                if !is_safe_site_relative_pattern(pattern) {
                    manifest.errors.push(format!(
                        "site {} rejected unsafe pattern (path escape): {pattern}",
                        site.id
                    ));
                    continue;
                }
                let normalized = expand_site_glob_pattern(pattern);
                let abs_pattern = host.join(&normalized).to_string_lossy().replace('\\', "/");

                app_log!(
                    debug,
                    "engine::backup",
                    "Site {} glob: {} -> {}",
                    site.id,
                    pattern,
                    abs_pattern
                );

                match glob(&abs_pattern) {
                    Ok(entries) => {
                        let mut matched_count = 0;
                        for entry in entries {
                            match entry {
                                Ok(path) if path.is_file() => {
                                    // 拒绝 glob 解析后仍逃出站点根的路径
                                    if !path_is_under(&path, &host) {
                                        manifest.errors.push(format!(
                                            "site {} matched path escaped site root: {}",
                                            site.id,
                                            path.display()
                                        ));
                                        continue;
                                    }
                                    let relative = match site_relative_path(&path, &host) {
                                        Ok(rel) => rel,
                                        Err(e) => {
                                            manifest.errors.push(format!(
                                                "site {} failed to keep relative path: {e}",
                                                site.id
                                            ));
                                            continue;
                                        }
                                    };
                                    matched_count += 1;
                                    // 必须保留站点内目录结构，例如 cmp/APP/Config/Config.local.php
                                    let zip_path = format!("{prefix}/{relative}");
                                    if let Err(e) =
                                        Self::add_file_to_zip(zip, &zip_path, &path, manifest)
                                    {
                                        manifest.errors.push(format!(
                                            "failed to pack site {} file {}: {e}",
                                            site.id,
                                            path.display()
                                        ));
                                    }
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    manifest.errors.push(format!("Glob match error: {e}"));
                                }
                            }
                        }
                        app_log!(
                            info,
                            "engine::backup",
                            "Site {} pattern '{}' matched {} file(s)",
                            site.id,
                            pattern,
                            matched_count
                        );
                    }
                    Err(e) => {
                        let error_msg = format!("Invalid glob '{pattern}': {e}");
                        manifest.errors.push(error_msg.clone());
                        app_log!(error, "engine::backup", "{}", error_msg);
                    }
                }
            }
        }
        Ok(())
    }

    /// Compute SHA256 hash of byte content.
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Helper: emit progress event via Tauri.
    ///
    /// `step` 传的是 i18n key（如 `backup.progress.steps.envConfig`），
    /// 由前端 `t()` 翻译后再展示，不要在这里拼自然语言。
    fn emit_progress(app_handle: Option<&tauri::AppHandle>, step: &str, percentage: u8) {
        if let Some(handle) = app_handle {
            use tauri::Emitter;
            let _ = handle.emit(
                "backup-progress",
                BackupProgress {
                    step: step.to_string(),
                    percentage,
                },
            );
        }
    }

    /// Helper: add a file to ZIP and record its SHA256 in the manifest.
    ///
    /// 以 64KB 分块流式读取：同一趟既喂给 SHA256 也写进 ZIP，文件不再整体进内存。
    /// 勾选"包含项目文件"时可能命中数据库 dump、视频素材等大文件，
    /// 旧实现 `fs::read` 全量读入会让内存随文件大小线性飙升。
    fn add_file_to_zip<W: Write + Seek>(
        zip: &mut zip::ZipWriter<W>,
        zip_path: &str,
        source: &Path,
        manifest: &mut BackupManifest,
    ) -> Result<(), String> {
        let zip_options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(zip_path, zip_options)
            .map_err(|e| format!("failed to create ZIP entry: {e}"))?;

        let mut file = fs::File::open(source)
            .map_err(|e| format!("failed to open file {}: {}", source.display(), e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|e| format!("failed to read file {}: {}", source.display(), e))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            zip.write_all(&buffer[..read])
                .map_err(|e| format!("failed to write ZIP content: {e}"))?;
        }

        let sha256 = format!("{:x}", hasher.finalize());
        manifest.files.insert(zip_path.to_string(), sha256);
        Ok(())
    }

    /// Helper: add directory contents to ZIP recursively.
    fn add_dir_to_zip<W: Write + Seek>(
        zip: &mut zip::ZipWriter<W>,
        src_dir: &Path,
        zip_prefix: &str,
        manifest: &mut BackupManifest,
    ) -> Result<(), String> {
        if !src_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(src_dir).map_err(|e| format!("failed to read directory: {e}"))? {
            let entry = entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
            let path = entry.path();
            // 无文件名（盘符根等）时跳过而非 panic
            let Some(name) = path.file_name() else {
                app_log!(
                    warn,
                    "engine::backup",
                    "Skipping path with no file name: {:?}",
                    path
                );
                continue;
            };
            let zip_path = format!("{zip_prefix}/{}", name.to_string_lossy());

            if path.is_dir() {
                Self::add_dir_to_zip(zip, &path, &zip_path, manifest)?;
            } else {
                Self::add_file_to_zip(zip, &zip_path, &path, manifest)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::user_config;

    /// 流式写入必须跨分块边界算出与全量读取一致的 SHA256。
    ///
    /// 缓冲区是 64KB，这里构造远超一块的内容（含 5 个跨块点），
    /// 一旦分块拼接或 hasher 更新有错，哈希立刻对不上。
    #[test]
    fn test_streamed_sha256_matches_full_read() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = tmp_dir.path();

        // 约 300KB，跨越 5 个 64KB 分块
        let content: Vec<u8> = (0..300 * 1024).map(|i| (i % 251) as u8).collect();
        let www = project_root.join("www");
        fs::create_dir_all(&www).expect("创建 www 失败");
        fs::write(www.join("big.bin"), &content).expect("写入大文件失败");
        fs::write(project_root.join(".env"), "SOURCE_DIR=./www\n").expect("写入 .env 失败");

        let services_dir = project_root.join("services");
        fs::create_dir_all(&services_dir).expect("创建 services 失败");
        fs::write(services_dir.join("big.bin"), &content).expect("写入失败");

        let backup_path = project_root.join("backup.zip");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(BackupEngine::create_backup(
            backup_path.to_str().unwrap(),
            BackupOptions {
                include_projects: true,
                project_patterns: vec!["big.bin".to_string()],
                include_logs: false,
                site_ids: vec!["main".to_string()],
                pack_full_tree: false,
            },
            project_root,
            None,
        ))
        .expect("备份失败");

        let file = fs::File::open(&backup_path).expect("打开备份文件失败");
        let mut archive = zip::ZipArchive::new(file).expect("解析 ZIP 失败");

        // manifest 里的哈希必须等于全量读取算出的哈希
        use std::io::Read;
        let manifest: BackupManifest = {
            let mut manifest_file = archive.by_name("manifest.json").unwrap();
            let mut manifest_json = String::new();
            manifest_file.read_to_string(&mut manifest_json).unwrap();
            serde_json::from_str(&manifest_json).expect("解析 manifest 失败")
        };

        let expected = BackupEngine::compute_sha256(&content);
        assert_eq!(
            manifest.files.get("projects/www/big.bin"),
            Some(&expected),
            "流式写入的哈希与全量读取不一致"
        );
        assert_eq!(
            manifest.files.get("services/big.bin"),
            Some(&expected),
            "目录递归同样应走流式写入"
        );

        // 内容本身也必须完整（哈希对但内容丢字节的情况要排除）
        let mut zipped = Vec::new();
        {
            let mut entry = archive.by_name("projects/www/big.bin").unwrap();
            entry.read_to_end(&mut zipped).unwrap();
        }
        assert_eq!(zipped.len(), content.len(), "ZIP 内文件大小不符");
    }

    #[test]
    fn test_add_file_to_zip_missing_file_reports_error() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let backup_path = tmp_dir.path().join("out.zip");
        let file = fs::File::create(&backup_path).expect("创建备份文件失败");
        let mut zip = zip::ZipWriter::new(file);
        let mut manifest = BackupManifest::new();

        let missing = tmp_dir.path().join("nope.txt");
        let result = BackupEngine::add_file_to_zip(&mut zip, "nope.txt", &missing, &mut manifest);
        assert!(result.is_err(), "缺失文件应返回错误而不是 panic");
    }

    #[test]
    fn test_compute_sha256() {
        // Known SHA256 for "hello world"
        let data = b"hello world";
        let hash = BackupEngine::compute_sha256(data);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_sha256_empty() {
        let data = b"";
        let hash = BackupEngine::compute_sha256(data);
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_create_backup_basic() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let project_root = tmp_dir.path();

        // Create .env and docker-compose.yml in the temp project root
        fs::write(project_root.join(".env"), "PHP82_VERSION=8.2.27\n").expect("写入 .env 失败");
        fs::write(
            project_root.join("docker-compose.yml"),
            "version: '3'\nservices:\n  php:\n    image: php:8.2\n",
        )
        .expect("写入 docker-compose.yml 失败");

        // Create a services/ directory with a config file
        let services_dir = project_root.join("services/php82");
        fs::create_dir_all(&services_dir).expect("创建 services 目录失败");
        fs::write(services_dir.join("php.ini"), "memory_limit=256M\n").expect("写入 php.ini 失败");

        user_config::ensure_dir(project_root).expect("创建 .user-config 失败");
        fs::write(
            user_config::path(project_root, user_config::MIRROR_CONFIG),
            "{\"apt\":{\"source\":\"http://mirrors.aliyun.com/debian/\",\"enabled\":true}}",
        )
        .expect("写入 mirror_config.json 失败");
        fs::write(
            user_config::path(project_root, user_config::VERSION_OVERRIDES),
            "{\"php\":{\"8.2\":{\"tag\":\"8.2-custom\"}}}",
        )
        .expect("写入 version_overrides.json 失败");

        let backup_path = project_root.join("backup.zip");
        let options = BackupOptions {
            include_projects: false,
            project_patterns: Vec::new(),
            include_logs: false,
            site_ids: Vec::new(),
            pack_full_tree: false,
        };

        // Run backup synchronously (no Tauri runtime needed)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(BackupEngine::create_backup(
            backup_path.to_str().unwrap(),
            options,
            project_root,
            None,
        ));
        assert!(result.is_ok(), "备份失败: {:?}", result.err());

        // Verify ZIP contents
        let file = fs::File::open(&backup_path).expect("打开备份文件失败");
        let mut archive = zip::ZipArchive::new(file).expect("解析 ZIP 失败");

        let mut file_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        file_names.sort();

        assert!(
            file_names.contains(&".env".to_string()),
            "ZIP 应包含 .env，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&"docker-compose.yml".to_string()),
            "ZIP 应包含 docker-compose.yml，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&"services/php82/php.ini".to_string()),
            "ZIP 应包含 services/php82/php.ini，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&user_config::relative(user_config::MIRROR_CONFIG)),
            "ZIP 应包含 .user-config/mirror_config.json，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&user_config::relative(user_config::VERSION_OVERRIDES)),
            "ZIP 应包含 .user-config/version_overrides.json，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&"manifest.json".to_string()),
            "ZIP 应包含 manifest.json，实际: {file_names:?}"
        );

        // Verify manifest.json content
        use std::io::Read;
        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut manifest_json = String::new();
        manifest_file.read_to_string(&mut manifest_json).unwrap();

        let manifest: BackupManifest =
            serde_json::from_str(&manifest_json).expect("解析 manifest 失败");
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.errors.is_empty(), "不应有错误");
        assert!(
            manifest.files.contains_key(".env"),
            "manifest 应包含 .env 的 SHA256"
        );
        assert!(
            manifest.files.contains_key("docker-compose.yml"),
            "manifest 应包含 docker-compose.yml 的 SHA256"
        );
        assert!(
            manifest.files.contains_key("services/php82/php.ini"),
            "manifest 应包含 services/php82/php.ini 的 SHA256"
        );
        assert!(
            manifest
                .files
                .contains_key(&user_config::relative(user_config::MIRROR_CONFIG)),
            "manifest 应包含 .user-config/mirror_config.json 的 SHA256"
        );
        assert!(
            manifest
                .files
                .contains_key(&user_config::relative(user_config::VERSION_OVERRIDES)),
            "manifest 应包含 .user-config/version_overrides.json 的 SHA256"
        );
    }

    #[test]
    fn packs_external_site_without_drive_letter_in_zip_name() {
        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("index.php"), b"<?php echo 1;\n").unwrap();
        let host = external.path().to_string_lossy().replace('\\', "/");
        fs::write(workspace.path().join(".env"), format!("SITE_SHOP={host}\n")).unwrap();
        user_config::ensure_dir(workspace.path()).unwrap();
        fs::write(
            user_config::path(workspace.path(), user_config::SITES),
            r#"{"sites":[{"id":"shop","server_name":"shop.test","env_key":"SITE_SHOP","container_path":"/sites/shop","nginx_service":"nginx125","php_service":"php82"}]}"#,
        )
        .unwrap();

        let backup_path = workspace.path().join("backup.zip");
        let options = BackupOptions {
            include_projects: true,
            project_patterns: Vec::new(),
            include_logs: false,
            site_ids: vec!["shop".to_string()],
            pack_full_tree: true,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(BackupEngine::create_backup(
            backup_path.to_str().unwrap(),
            options,
            workspace.path(),
            None,
        ))
        .expect("备份应成功");

        let file = fs::File::open(&backup_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(
            names
                .iter()
                .any(|name| name == "projects/sites/shop/index.php"),
            "外部站点应打进 projects/sites/shop，实际: {names:?}"
        );
        assert!(
            names.iter().all(|name| !name.contains(':')),
            "ZIP 条目名不应包含盘符: {names:?}"
        );
        assert!(names
            .iter()
            .any(|name| name == &user_config::relative(user_config::SITES)));

        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut json = String::new();
        use std::io::Read;
        manifest_file.read_to_string(&mut json).unwrap();
        let manifest: BackupManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.sites[0].kind, "absolute");
        assert_eq!(manifest.sites[0].id, "shop");
    }

    #[test]
    fn packs_site_relative_patterns_only() {
        let workspace = tempfile::tempdir().unwrap();
        let www = workspace.path().join("www");
        fs::create_dir_all(&www).unwrap();
        fs::write(www.join(".env"), b"APP=1\n").unwrap();
        fs::write(www.join("index.php"), b"<?php\n").unwrap();
        fs::write(www.join("local.config.php"), b"<?php return [];\n").unwrap();
        fs::write(workspace.path().join(".env"), "SOURCE_DIR=./www\n").unwrap();

        let backup_path = workspace.path().join("backup.zip");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(BackupEngine::create_backup(
            backup_path.to_str().unwrap(),
            BackupOptions {
                include_projects: true,
                project_patterns: vec![".env".into(), "local.config.php".into()],
                include_logs: false,
                site_ids: vec!["main".into()],
                pack_full_tree: false,
            },
            workspace.path(),
            None,
        ))
        .expect("备份应成功");

        let file = fs::File::open(&backup_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "projects/www/.env"));
        assert!(names.iter().any(|n| n == "projects/www/local.config.php"));
        assert!(
            !names.iter().any(|n| n == "projects/www/index.php"),
            "未匹配的源码不应打进包: {names:?}"
        );
    }

    #[test]
    fn packs_nested_local_config_preserving_directories() {
        let workspace = tempfile::tempdir().unwrap();
        let www = workspace.path().join("www");
        let nested = www.join("cmp/APP/Config");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Config.local.php"), b"<?php return [];\n").unwrap();
        fs::write(www.join("index.php"), b"<?php\n").unwrap();
        fs::write(workspace.path().join(".env"), "SOURCE_DIR=./www\n").unwrap();

        let backup_path = workspace.path().join("backup.zip");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(BackupEngine::create_backup(
            backup_path.to_str().unwrap(),
            BackupOptions {
                include_projects: true,
                // 无斜杠：应自动按 **/Config.local.php 递归匹配，且 ZIP 保留目录
                project_patterns: vec!["Config.local.php".into(), "*.local.php".into()],
                include_logs: false,
                site_ids: vec!["main".into()],
                pack_full_tree: false,
            },
            workspace.path(),
            None,
        ))
        .expect("备份应成功");

        let file = fs::File::open(&backup_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(
            names
                .iter()
                .any(|n| n == "projects/www/cmp/APP/Config/Config.local.php"),
            "应保留完整相对路径，实际: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n == "projects/www/Config.local.php"),
            "不应扁平成仅文件名: {names:?}"
        );
        assert!(!names.iter().any(|n| n == "projects/www/index.php"));
    }

    #[test]
    fn rejects_path_escape_patterns() {
        assert!(!is_safe_site_relative_pattern("../.env"));
        assert!(!is_safe_site_relative_pattern("/etc/passwd"));
        assert!(!is_safe_site_relative_pattern("C:/Windows/win.ini"));
        assert!(is_safe_site_relative_pattern(".env"));
        assert!(is_safe_site_relative_pattern("config/*.local.php"));
        assert_eq!(expand_site_glob_pattern("*.local.php"), "**/*.local.php");
        assert_eq!(
            expand_site_glob_pattern("cmp/APP/Config/Config.local.php"),
            "cmp/APP/Config/Config.local.php"
        );
        assert_eq!(
            expand_site_glob_pattern("Config.local.php"),
            "**/Config.local.php"
        );
    }
}
