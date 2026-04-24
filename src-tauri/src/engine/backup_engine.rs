use std::fs;
use std::io::{Write, Seek};
use std::path::Path;
use zip::write::FileOptions;
use sha2::{Sha256, Digest};
use glob::glob;

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
            .map_err(|e| format!("创建备份文件失败: {e}"))?;
        let mut zip = zip::ZipWriter::new(file);
        let mut manifest = BackupManifest::new();
        manifest.options = options.clone();

        // Step 1: Pack .env (10%)
        Self::emit_progress(app_handle, "打包环境配置...", 10);
        let env_path = project_root.join(".env");
        if env_path.exists() {
            let content = fs::read(&env_path)
                .map_err(|e| format!("读取 .env 失败: {e}"))?;
            Self::add_file_to_zip(&mut zip, ".env", &content, &mut manifest)?;
        }

        // Step 2: Pack docker-compose.yml (20%)
        Self::emit_progress(app_handle, "打包 Docker 配置...", 20);
        let compose_path = project_root.join("docker-compose.yml");
        if compose_path.exists() {
            let content = fs::read(&compose_path)
                .map_err(|e| format!("读取 docker-compose.yml 失败: {e}"))?;
            Self::add_file_to_zip(
                &mut zip,
                "docker-compose.yml",
                &content,
                &mut manifest,
            )?;
        }

        // Step 3: Pack services/ configs (30%)
        Self::emit_progress(app_handle, "打包服务配置...", 30);
        let services_dir = project_root.join("services");
        if services_dir.exists() {
            Self::add_dir_to_zip(&mut zip, &services_dir, "services", &mut manifest)?;
        }

        // Step 3.5: Pack user custom configuration files (35%)
        Self::emit_progress(app_handle, "打包用户自定义配置...", 35);
        
        // .user_mirror_config.json - User mirror source configuration
        let user_mirror_config_path = project_root.join(".user_mirror_config.json");
        if user_mirror_config_path.exists() {
            let content = fs::read(&user_mirror_config_path)
                .map_err(|e| format!("读取 .user_mirror_config.json 失败: {e}"))?;
            Self::add_file_to_zip(
                &mut zip,
                ".user_mirror_config.json",
                &content,
                &mut manifest,
            )?;
        }
        
        // .user_version_overrides.json - User version override configuration
        let user_version_overrides_path = project_root.join(".user_version_overrides.json");
        if user_version_overrides_path.exists() {
            let content = fs::read(&user_version_overrides_path)
                .map_err(|e| format!("读取 .user_version_overrides.json 失败: {e}"))?;
            Self::add_file_to_zip(
                &mut zip,
                ".user_version_overrides.json",
                &content,
                &mut manifest,
            )?;
        }

        // Step 4: Optional — Project files (50%)
        if options.include_projects && !options.project_patterns.is_empty() {
            Self::emit_progress(app_handle, "打包项目文件...", 60);
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
                    project_root.join(&normalized_pattern).to_string_lossy().replace('\\', "/")
                };
                
                // 记录尝试的模式（用于调试）
                app_log!(debug, "engine::backup", "尝试匹配模式: {} -> {}", pattern, abs_pattern);
                
                match glob(&abs_pattern) {
                    Ok(entries) => {
                        let mut matched_count = 0;
                        for entry in entries {
                            match entry {
                                Ok(path) if path.is_file() => {
                                    matched_count += 1;
                                    match fs::read(&path) {
                                        Ok(content) => {
                                            // 计算相对于项目根目录的路径
                                            let relative_path = pathdiff::diff_paths(&path, project_root)
                                                .map(|p| p.to_string_lossy().replace('\\', "/"))
                                                .unwrap_or_else(|| path.display().to_string());
                                            let zip_path = format!("projects/{relative_path}");
                                            app_log!(debug, "engine::backup", "添加文件: {}", zip_path);
                                            Self::add_file_to_zip(
                                                &mut zip,
                                                &zip_path,
                                                &content,
                                                &mut manifest,
                                            )?;
                                        }
                                        Err(e) => {
                                            manifest.errors.push(format!(
                                                "读取项目文件失败 {}: {}",
                                                path.display(),
                                                e
                                            ));
                                        }
                                    }
                                }
                                Ok(path) => {
                                    // 跳过目录
                                    app_log!(debug, "engine::backup", "跳过目录: {:?}", path);
                                }
                                Err(e) => {
                                    manifest.errors.push(format!("Glob 匹配错误: {e}"));
                                    app_log!(warn, "engine::backup", "Glob 匹配错误: {}", e);
                                }
                            }
                        }
                        app_log!(info, "engine::backup", "模式 '{}' 匹配到 {} 个文件", pattern, matched_count);
                    }
                    Err(e) => {
                        let error_msg = format!("Glob 模式错误 '{pattern}': {e}");
                        manifest.errors.push(error_msg.clone());
                        app_log!(error, "engine::backup", "{}", error_msg);
                    }
                }
            }
        }

        // Step 5: Optional — Recent logs (70%)
        if options.include_logs {
            Self::emit_progress(app_handle, "打包日志文件...", 85);
            let logs_dir = project_root.join("logs");
            if logs_dir.exists() {
                // MVP: pack all logs (7-day filter can be added later)
                Self::add_dir_to_zip(&mut zip, &logs_dir, "logs", &mut manifest)?;
            }
        }

        // Step 8: Write manifest.json (95%)
        Self::emit_progress(app_handle, "生成备份清单...", 95);
        let manifest_json = manifest.serialize()?;
        let zip_options = FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", zip_options)
            .map_err(|e| format!("创建 manifest 条目失败: {e}"))?;
        zip.write_all(manifest_json.as_bytes())
            .map_err(|e| format!("写入 manifest 失败: {e}"))?;

        // Finish ZIP
        zip.finish().map_err(|e| format!("完成 ZIP 文件失败: {e}"))?;

        Self::emit_progress(app_handle, "备份完成", 100);
        Ok(())
    }

    /// Compute SHA256 hash of byte content.
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Helper: emit progress event via Tauri.
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

    /// Helper: add a file to ZIP and record in manifest with SHA256.
    fn add_file_to_zip<W: Write + Seek>(
        zip: &mut zip::ZipWriter<W>,
        zip_path: &str,
        content: &[u8],
        manifest: &mut BackupManifest,
    ) -> Result<(), String> {
        let zip_options = FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(zip_path, zip_options)
            .map_err(|e| format!("创建 ZIP 条目失败: {e}"))?;
        zip.write_all(content)
            .map_err(|e| format!("写入 ZIP 内容失败: {e}"))?;

        let sha256 = Self::compute_sha256(content);
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
        for entry in
            fs::read_dir(src_dir).map_err(|e| format!("读取目录失败: {e}"))?
        {
            let entry = entry.map_err(|e| format!("读取目录条目失败: {e}"))?;
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();
            let zip_path = format!("{zip_prefix}/{name}");

            if path.is_dir() {
                Self::add_dir_to_zip(zip, &path, &zip_path, manifest)?;
            } else {
                let content = fs::read(&path)
                    .map_err(|e| format!("读取文件失败 {}: {}", path.display(), e))?;
                Self::add_file_to_zip(zip, &zip_path, &content, manifest)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        fs::write(project_root.join(".env"), "PHP82_VERSION=8.2.27\n")
            .expect("写入 .env 失败");
        fs::write(
            project_root.join("docker-compose.yml"),
            "version: '3'\nservices:\n  php:\n    image: php:8.2\n",
        )
        .expect("写入 docker-compose.yml 失败");

        // Create a services/ directory with a config file
        let services_dir = project_root.join("services/php82");
        fs::create_dir_all(&services_dir).expect("创建 services 目录失败");
        fs::write(services_dir.join("php.ini"), "memory_limit=256M\n")
            .expect("写入 php.ini 失败");

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
