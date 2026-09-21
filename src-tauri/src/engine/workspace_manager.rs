use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::commands::paths;

/// 工作目录配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace_path: String,
    pub last_updated: Option<String>,
}

pub struct WorkspaceManager;

impl WorkspaceManager {
    /// 获取 workspace.json 的路径。
    ///
    /// 位于应用数据目录（`%APPDATA%\<identifier>` / `~/Library/Application Support/<identifier>`）。
    /// 此前放在 exe 同级目录，用户装进 `Program Files` 后无写权限。
    fn get_config_path() -> Result<PathBuf, String> {
        Ok(paths::app_data_dir()?.join("workspace.json"))
    }

    /// 读取工作目录配置
    pub fn load_workspace() -> Result<Option<WorkspaceConfig>, String> {
        let config_path = Self::get_config_path()?;
        if !config_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("读取 workspace.json 失败: {e}"))?;

        let config: WorkspaceConfig =
            serde_json::from_str(&content).map_err(|e| format!("解析 workspace.json 失败: {e}"))?;

        Ok(Some(config))
    }

    /// 保存工作目录配置
    pub fn save_workspace(path: &str) -> Result<(), String> {
        let config_path = Self::get_config_path()?;
        let config = WorkspaceConfig {
            workspace_path: path.to_string(),
            last_updated: Some(chrono::Local::now().to_rfc3339()),
        };

        let json =
            serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {e}"))?;

        fs::write(&config_path, json).map_err(|e| format!("写入 workspace.json 失败: {e}"))?;

        Ok(())
    }

    /// 检查工作目录是否有效（包含 .env 或 docker-compose.yml）
    pub fn is_workspace_valid(workspace_path: &str) -> bool {
        let path = PathBuf::from(workspace_path);
        path.join(".env").exists() || path.join("docker-compose.yml").exists()
    }
}
