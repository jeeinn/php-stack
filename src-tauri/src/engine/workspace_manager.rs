use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 工作目录配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace_path: String,
    pub last_updated: Option<String>,
}

pub struct WorkspaceManager;

impl WorkspaceManager {
    /// 获取 workspace.json 的路径（位于可执行文件同级）
    fn get_config_path() -> Result<PathBuf, String> {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {}", e))?;
        
        let config_dir = if cfg!(debug_assertions) {
            // 开发模式：workspace.json 放在项目根目录 (src-tauri 的父目录)
            exe_path.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .ok_or("无法获取项目根目录")?
                .to_path_buf()
        } else {
            // 生产模式：放在 exe 同级目录
            exe_path.parent()
                .ok_or("无法获取程序所在目录")?
                .to_path_buf()
        };

        Ok(config_dir.join("workspace.json"))
    }

    /// 读取工作目录配置
    pub fn load_workspace() -> Result<Option<WorkspaceConfig>, String> {
        let config_path = Self::get_config_path()?;
        if !config_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("读取 workspace.json 失败: {}", e))?;
        
        let config: WorkspaceConfig = serde_json::from_str(&content)
            .map_err(|e| format!("解析 workspace.json 失败: {}", e))?;

        Ok(Some(config))
    }

    /// 保存工作目录配置
    pub fn save_workspace(path: &str) -> Result<(), String> {
        let config_path = Self::get_config_path()?;
        let config = WorkspaceConfig {
            workspace_path: path.to_string(),
            last_updated: Some(chrono::Local::now().to_rfc3339()),
        };

        let json = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;

        fs::write(&config_path, json)
            .map_err(|e| format!("写入 workspace.json 失败: {}", e))?;

        Ok(())
    }

    /// 检查工作目录是否有效（包含 .env 或 docker-compose.yml）
    pub fn is_workspace_valid(workspace_path: &str) -> bool {
        let path = PathBuf::from(workspace_path);
        path.join(".env").exists() || path.join("docker-compose.yml").exists()
    }
}
