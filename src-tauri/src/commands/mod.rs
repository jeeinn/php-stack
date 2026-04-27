mod docker;
mod env_config;
mod mirror;
mod backup;
mod workspace;

// Re-export all commands for lib.rs invoke_handler registration
pub use docker::*;
pub use env_config::*;
pub use mirror::*;
pub use backup::*;
pub use workspace::*;

use crate::engine::workspace_manager::WorkspaceManager;

/// 获取项目根目录（优先读取 workspace.json）
pub(crate) fn get_project_root() -> Result<std::path::PathBuf, String> {
    // 1. 尝试从 workspace.json 读取配置
    if let Some(workspace) = WorkspaceManager::load_workspace()? {
        let path = std::path::PathBuf::from(&workspace.workspace_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. 如果未配置或路径无效，返回 exe 同级目录作为默认值
    if cfg!(debug_assertions) {
        // 开发模式：使用项目根目录（src-tauri 的父目录）
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent() // target/debug/
            .and_then(|p| p.parent()) // target/
            .and_then(|p| p.parent()) // src-tauri/
            .and_then(|p| p.parent()) // 项目根目录
            .ok_or("无法获取项目根目录")?
            .to_path_buf())
    } else {
        // 生产模式：使用可执行文件所在目录
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent()
            .ok_or("无法获取程序所在目录")?
            .to_path_buf())
    }
}
