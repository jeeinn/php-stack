//! 备份页选项持久化到 `.user-config/backup.json`（不打进环境 ZIP）。

use std::path::Path;

use super::backup_manifest::{default_local_config_patterns, BackupOptions};
use super::user_config;

/// 文件不存在时的默认选项（含本地配置默认 patterns）。
pub fn default_options() -> BackupOptions {
    BackupOptions {
        include_projects: false,
        project_patterns: default_local_config_patterns(),
        include_logs: false,
        site_ids: Vec::new(),
        pack_full_tree: false,
    }
}

pub fn load(project_root: &Path) -> Result<BackupOptions, String> {
    let path = user_config::path(project_root, user_config::BACKUP);
    if !path.exists() {
        return Ok(default_options());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", user_config::BACKUP))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", user_config::BACKUP))
}

pub fn save(project_root: &Path, options: &BackupOptions) -> Result<(), String> {
    user_config::ensure_dir(project_root)?;
    let path = user_config::path(project_root, user_config::BACKUP);
    let content = serde_json::to_string_pretty(options)
        .map_err(|e| format!("failed to serialize {}: {e}", user_config::BACKUP))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("failed to write {}: {e}", user_config::BACKUP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_default_local_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load(tmp.path()).expect("load");
        assert!(!loaded.include_projects);
        assert!(!loaded.pack_full_tree);
        assert_eq!(loaded.project_patterns, default_local_config_patterns());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let options = BackupOptions {
            include_projects: true,
            project_patterns: vec![".env".into(), "local.config.php".into()],
            include_logs: true,
            site_ids: vec!["main".into(), "shop".into()],
            pack_full_tree: false,
        };
        save(tmp.path(), &options).expect("save");
        let loaded = load(tmp.path()).expect("load");
        assert_eq!(loaded, options);
        assert!(user_config::path(tmp.path(), user_config::BACKUP).is_file());
    }

    #[test]
    fn corrupt_json_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        user_config::ensure_dir(tmp.path()).unwrap();
        std::fs::write(
            user_config::path(tmp.path(), user_config::BACKUP),
            "{not json",
        )
        .unwrap();
        assert!(load(tmp.path()).is_err());
    }
}
