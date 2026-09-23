//! 工作区内用户侧配置的唯一路径约定。
//!
//! 全部落在工作区 `.user-config/` 下，文件名不再带前导点。

use std::fs;
use std::path::{Path, PathBuf};

pub const DIR_NAME: &str = ".user-config";
pub const MIRROR_CONFIG: &str = "mirror_config.json";
pub const VERSION_OVERRIDES: &str = "version_overrides.json";
pub const SITES: &str = "sites.json";

pub fn dir(project_root: &Path) -> PathBuf {
    project_root.join(DIR_NAME)
}

pub fn path(project_root: &Path, file_name: &str) -> PathBuf {
    dir(project_root).join(file_name)
}

/// ZIP 条目与清单键。始终使用 `/`，与操作系统无关。
pub fn relative(file_name: &str) -> String {
    format!("{DIR_NAME}/{file_name}")
}

pub fn ensure_dir(project_root: &Path) -> Result<(), String> {
    fs::create_dir_all(dir(project_root)).map_err(|e| format!("failed to create {DIR_NAME}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_relative_stay_under_user_config_dir() {
        let root = Path::new("workspace");
        let mirror = path(root, MIRROR_CONFIG);
        assert_eq!(
            mirror.file_name().and_then(|n| n.to_str()),
            Some(MIRROR_CONFIG)
        );
        assert_eq!(
            mirror
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some(DIR_NAME)
        );
        assert_eq!(mirror.parent().and_then(|p| p.parent()), Some(root));
        assert_eq!(
            relative(VERSION_OVERRIDES),
            ".user-config/version_overrides.json"
        );
        assert_eq!(relative(SITES), ".user-config/sites.json");
        assert!(!relative(MIRROR_CONFIG).contains('\\'));
    }

    #[test]
    fn ensure_dir_creates_the_folder() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let folder = dir(tmp.path());
        assert!(!folder.exists());
        ensure_dir(tmp.path()).expect("创建 .user-config 失败");
        assert!(folder.is_dir());
        ensure_dir(tmp.path()).expect("重复创建应成功");
    }
}
