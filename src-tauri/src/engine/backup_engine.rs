use glob::glob;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::Path;
use zip::write::FileOptions;

use super::backup_manifest::{BackupManifest, BackupOptions};
use crate::app_log;

/// 备份进度事件
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupProgress {
    pub step: String,
    pub percentage: u8,
}

pub struct BackupEngine;

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

        // .user_mirror_config.json - User mirror source configuration
        let user_mirror_config_path = project_root.join(".user_mirror_config.json");
        if user_mirror_config_path.exists() {
            Self::add_file_to_zip(
                &mut zip,
                ".user_mirror_config.json",
                &user_mirror_config_path,
                &mut manifest,
            )?;
        }

        // .user_version_overrides.json - User version override configuration
        let user_version_overrides_path = project_root.join(".user_version_overrides.json");
        if user_version_overrides_path.exists() {
            Self::add_file_to_zip(
                &mut zip,
                ".user_version_overrides.json",
                &user_version_overrides_path,
                &mut manifest,
            )?;
        }

        // Step 4: Optional — Project files (50%)
        if options.include_projects && !options.project_patterns.is_empty() {
            Self::emit_progress(app_handle, "backup.progress.steps.projectFiles", 60);
            for pattern in &options.project_patterns {
                // 将相对路径模式转换为绝对路径模式
                let mut normalized_pattern = pattern.clone();

                // 如果模式以 /** 结尾，添加 /* 以匹配文件
                // 例如："www/AAA/**" -> "www/AAA/**/*"
                if normalized_pattern.ends_with("/**") {
                    normalized_pattern.push_str("/*");
                }

                let abs_pattern = if std::path::Path::new(&normalized_pattern).is_absolute() {
                    normalized_pattern
                } else {
                    project_root
                        .join(&normalized_pattern)
                        .to_string_lossy()
                        .replace('\\', "/")
                };

                // 记录尝试的模式（用于调试）
                app_log!(
                    debug,
                    "engine::backup",
                    "Trying glob: {} -> {}",
                    pattern,
                    abs_pattern
                );

                match glob(&abs_pattern) {
                    Ok(entries) => {
                        let mut matched_count = 0;
                        for entry in entries {
                            match entry {
                                Ok(path) if path.is_file() => {
                                    matched_count += 1;
                                    // 计算相对于项目根目录的路径
                                    let relative_path = pathdiff::diff_paths(&path, project_root)
                                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                                        .unwrap_or_else(|| path.display().to_string());
                                    let zip_path = format!("projects/{relative_path}");
                                    app_log!(debug, "engine::backup", "Adding file: {}", zip_path);
                                    // 流式写入：单个大文件不再整体进内存
                                    if let Err(e) = Self::add_file_to_zip(
                                        &mut zip,
                                        &zip_path,
                                        &path,
                                        &mut manifest,
                                    ) {
                                        manifest.errors.push(format!(
                                            "failed to pack project file {}: {}",
                                            path.display(),
                                            e
                                        ));
                                    }
                                }
                                Ok(path) => {
                                    // 跳过目录
                                    app_log!(debug, "engine::backup", "Skipping dir: {:?}", path);
                                }
                                Err(e) => {
                                    manifest.errors.push(format!("Glob match error: {e}"));
                                    app_log!(warn, "engine::backup", "Glob match error: {}", e);
                                }
                            }
                        }
                        app_log!(
                            info,
                            "engine::backup",
                            "Pattern '{}' matched {} file(s)",
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
        let big_file = project_root.join("big.bin");
        fs::write(&big_file, &content).expect("写入大文件失败");

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
            manifest.files.get("projects/big.bin"),
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
            let mut entry = archive.by_name("projects/big.bin").unwrap();
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

        // Create user custom configuration files
        fs::write(
            project_root.join(".user_mirror_config.json"),
            "{\"apt\":{\"source\":\"http://mirrors.aliyun.com/debian/\",\"enabled\":true}}",
        )
        .expect("写入 .user_mirror_config.json 失败");
        fs::write(
            project_root.join(".user_version_overrides.json"),
            "{\"php\":{\"8.2\":{\"tag\":\"8.2-custom\"}}}",
        )
        .expect("写入 .user_version_overrides.json 失败");

        let backup_path = project_root.join("backup.zip");
        let options = BackupOptions {
            include_projects: false,
            project_patterns: Vec::new(),
            include_logs: false,
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
            file_names.contains(&".user_mirror_config.json".to_string()),
            "ZIP 应包含 .user_mirror_config.json，实际: {file_names:?}"
        );
        assert!(
            file_names.contains(&".user_version_overrides.json".to_string()),
            "ZIP 应包含 .user_version_overrides.json，实际: {file_names:?}"
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
            manifest.files.contains_key(".user_mirror_config.json"),
            "manifest 应包含 .user_mirror_config.json 的 SHA256"
        );
        assert!(
            manifest.files.contains_key(".user_version_overrides.json"),
            "manifest 应包含 .user_version_overrides.json 的 SHA256"
        );
    }
}
