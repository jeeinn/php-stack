mod backup;
mod docker;
mod env_config;
mod mirror;
pub mod paths;
mod workspace;

// Re-export all commands for lib.rs invoke_handler registration
pub use backup::*;
pub use docker::*;
pub use env_config::*;
pub use mirror::*;
pub use workspace::*;

use paths::project_root;

/// 获取项目根目录（优先读取 workspace.json）
///
/// 逻辑已迁至 `paths::project_root()`，此处保留同名薄封装，
/// 避免 20 余处调用点一起改动。
pub(crate) fn get_project_root() -> Result<std::path::PathBuf, String> {
    project_root()
}

/// 获取日志文件路径（位于应用数据目录）
pub(crate) fn get_log_file() -> Result<std::path::PathBuf, String> {
    paths::log_file()
}
