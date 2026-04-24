use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

use super::backup_engine::BackupEngine;
use super::backup_manifest::BackupManifest;

/// 恢复预览信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub manifest: BackupManifest,
    pub file_count: usize,
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
}

pub struct RestoreEngine;

impl RestoreEngine {
    /// Parse backup ZIP and return preview info.
    /// Reads manifest.json from ZIP, detects port conflicts, counts files.
    pub fn preview(zip_path: &str) -> Result<RestorePreview, String> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("打开备份文件失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("解析 ZIP 文件失败: {}", e))?;

        // Read manifest.json
        let manifest = Self::read_manifest_from_archive(&mut archive)?;

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
            manifest,
            file_count,
        })
    }

    /// Verify backup integrity by checking SHA256 checksums.
    /// For each file in manifest.files, read from ZIP and compute SHA256,
    /// compare with recorded hash.
    pub fn verify_integrity(zip_path: &str) -> Result<bool, String> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("打开备份文件失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("解析 ZIP 文件失败: {}", e))?;

        let manifest = Self::read_manifest_from_archive(&mut archive)?;

        for (file_path, expected_hash) in &manifest.files {
            let mut zip_file = archive
                .by_name(file_path)
                .map_err(|e| format!("读取 ZIP 条目 '{}' 失败: {}", file_path, e))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("读取文件内容 '{}' 失败: {}", file_path, e))?;

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

        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("打开备份文件失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("解析 ZIP 文件失败: {}", e))?;

        // Step 1: Read manifest
        Self::emit_progress(app_handle, "解析备份包...", 5);
        let manifest = Self::read_manifest_from_archive(&mut archive)?;

        // Step 2: Extract .env
        Self::emit_progress(app_handle, "恢复环境配置...", 15);
        match Self::restore_env_file(&mut archive, project_root) {
            Ok(()) => restored_files.push(".env".to_string()),
            Err(e) => errors.push(format!("恢复 .env 失败: {}", e)),
        }

        // Step 3: Extract docker-compose.yml
        Self::emit_progress(app_handle, "恢复 Docker 配置...", 25);
        match Self::extract_file_to_path(&mut archive, "docker-compose.yml", &project_root.join("docker-compose.yml")) {
            Ok(()) => restored_files.push("docker-compose.yml".to_string()),
            Err(e) => errors.push(format!("恢复 docker-compose.yml 失败: {}", e)),
        }

        // Step 4: Extract services/ directory contents
        Self::emit_progress(app_handle, "恢复服务配置...", 40);
        match Self::extract_prefix(&mut archive, "services/", &project_root.join("services")) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("恢复 services/ 失败: {}", e)),
        }

        // Step 4.5: Restore user custom configuration files
        Self::emit_progress(app_handle, "恢复用户自定义配置...", 45);
        
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
        Self::emit_progress(app_handle, "恢复虚拟主机配置...", 55);
        match Self::extract_prefix(
            &mut archive,
            "vhosts/",
            &project_root.join("services/nginx/conf.d"),
        ) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("恢复 vhosts/ 失败: {}", e)),
        }

        // Step 6: Extract projects/ to project_root (paths are already relative to project_root)
        Self::emit_progress(app_handle, "恢复项目文件...", 70);
        if !manifest.options.project_patterns.is_empty() {
            // 备份时已经将文件路径存储为相对于 project_root 的路径
            // 例如："www/test/index.php" → ZIP 中为 "projects/www/test/index.php"
            // 恢复时直接提取到 project_root 即可
            match Self::extract_prefix(&mut archive, "projects/", project_root) {
                Ok(files) => restored_files.extend(files),
                Err(e) => errors.push(format!("恢复项目文件失败: {}", e)),
            }
        }

        // Step 7: Extract database/ SQL files
        Self::emit_progress(app_handle, "恢复数据库文件...", 85);
        match Self::extract_prefix(
            &mut archive,
            "database/",
            &project_root.join("database"),
        ) {
            Ok(files) => restored_files.extend(files),
            Err(e) => errors.push(format!("恢复数据库文件失败: {}", e)),
        }

        // Step 8: Done
        Self::emit_progress(app_handle, "恢复完成", 100);

        let _ = manifest; // manifest was used for reading

        Ok(RestoreResult {
            success: errors.is_empty(),
            restored_files,
            errors,
        })
    }


    /// Read manifest.json from a ZIP archive.
    fn read_manifest_from_archive<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
    ) -> Result<BackupManifest, String> {
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|e| format!("读取 manifest.json 失败: {}", e))?;

        let mut manifest_json = String::new();
        manifest_file
            .read_to_string(&mut manifest_json)
            .map_err(|e| format!("读取 manifest.json 内容失败: {}", e))?;

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
            .map_err(|e| format!("读取 ZIP 条目 '{}' 失败: {}", zip_entry, e))?;

        let mut content = Vec::new();
        zip_file
            .read_to_end(&mut content)
            .map_err(|e| format!("读取文件内容 '{}' 失败: {}", zip_entry, e))?;

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
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
                .map_err(|e| format!("读取 ZIP 条目 '{}' 失败: {}", name, e))?;

            let mut content = Vec::new();
            zip_file
                .read_to_end(&mut content)
                .map_err(|e| format!("读取文件内容 '{}' 失败: {}", name, e))?;

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

    /// Read SOURCE_DIR from the restored .env file.
    /// Note: Currently unused as we restore projects directly to project_root.
    /// Kept for potential future use or fallback scenarios.
    #[allow(dead_code)]
    fn read_source_dir_from_env(project_root: &Path) -> Option<String> {
        let env_path = project_root.join(".env");
        let content = std::fs::read_to_string(&env_path).ok()?;
        use super::env_parser::EnvFile;
        let env_file = EnvFile::parse(&content).ok()?;
        env_file.get("SOURCE_DIR").map(|s| s.to_string())
    }

    /// Restore .env file from backup.
    fn restore_env_file<R: Read + std::io::Seek>(
        archive: &mut zip::ZipArchive<R>,
        project_root: &Path,
    ) -> Result<(), String> {
        let mut zip_file = archive
            .by_name(".env")
            .map_err(|e| format!("读取 .env 失败: {}", e))?;

        let mut content = String::new();
        zip_file
            .read_to_string(&mut content)
            .map_err(|e| format!("读取 .env 内容失败: {}", e))?;

        let env_path = project_root.join(".env");
        std::fs::write(&env_path, content)
            .map_err(|e| format!("写入 .env 失败: {}", e))?;

        Ok(())
    }

    /// Helper: emit progress event via Tauri.
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
        let user_mirror_config_content = b"{\"apt\":{\"source\":\"http://mirrors.aliyun.com/debian/\",\"enabled\":true}}";
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
        files.insert(".user_version_overrides.json".to_string(), user_version_hash);

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
                project_patterns: vec![
                    "www/test/**".to_string(),
                    "www/readme.md".to_string(),
                ],
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

    #[test]
    fn test_verify_integrity_valid() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup(tmp_dir.path());

        let result =
            RestoreEngine::verify_integrity(&zip_path).expect("验证完整性失败");
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
        assert!(
            !result,
            "Tampered backup should fail integrity check"
        );
    }

    #[test]
    fn test_restore_basic() {
        let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
        let zip_path = create_test_backup(tmp_dir.path());

        // Create a separate restore target directory
        let restore_dir = tmp_dir.path().join("restored");
        fs::create_dir_all(&restore_dir).expect("创建恢复目录失败");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(RestoreEngine::restore(
            &zip_path,
            &restore_dir,
            None,
        ));

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
        let php_ini_content =
            fs::read_to_string(&php_ini_path).expect("读取 php.ini 失败");
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
        let result = rt.block_on(RestoreEngine::restore(
            &zip_path,
            &restore_dir,
            None,
        ));

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
