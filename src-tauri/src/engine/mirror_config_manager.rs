/// 用户自定义镜像源配置管理器
///
/// 类似于 UserOverrideManager，管理用户对镜像源的自定义配置。
/// 配置文件：.user_mirror_config.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 用户自定义的单个镜像源类别配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMirrorCategory {
    /// 自定义的镜像源地址
    pub source: String,
    /// 是否启用此自定义配置
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 备注说明
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_enabled() -> bool {
    true
}

/// 用户镜像源配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserMirrorConfig {
    /// 用户自定义的各个类别配置
    #[serde(default)]
    pub categories: HashMap<String, UserMirrorCategory>,
}

impl UserMirrorConfig {
    /// 从文件加载用户配置
    pub fn load(project_root: &Path) -> Result<Self, String> {
        let config_path = project_root.join(".user_mirror_config.json");
        
        if !config_path.exists() {
            return Ok(Self::default());
        }
        
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("读取用户镜像配置文件失败: {}", e))?;
        
        serde_json::from_str(&content)
            .map_err(|e| format!("解析用户镜像配置文件失败: {}", e))
    }
    
    /// 保存用户配置到文件
    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let config_path = project_root.join(".user_mirror_config.json");
        
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化用户镜像配置失败: {}", e))?;
        
        std::fs::write(&config_path, content)
            .map_err(|e| format!("写入用户镜像配置文件失败: {}", e))
    }
    
    /// 检查某个类别是否有用户自定义配置
    pub fn has_user_override(&self, category_id: &str) -> bool {
        self.categories.get(category_id)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }
    
    /// 获取某个类别的自定义配置
    pub fn get_category(&self, category_id: &str) -> Option<&UserMirrorCategory> {
        self.categories.get(category_id)
    }
    
    /// 设置某个类别的自定义配置
    pub fn set_category(&mut self, category_id: String, category: UserMirrorCategory) {
        self.categories.insert(category_id, category);
    }
    
    /// 删除某个类别的自定义配置
    pub fn remove_category(&mut self, category_id: &str) -> Option<UserMirrorCategory> {
        self.categories.remove(category_id)
    }
    
    /// 清空所有自定义配置
    pub fn clear_all(&mut self) {
        self.categories.clear();
    }
}

/// 单个镜像源选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorSourceOption {
    pub id: String,
    pub name: String,
    pub value: String,
    pub description: String,
}

/// 合并后的镜像源类别信息（默认配置 + 用户自定义）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedMirrorCategory {
    pub category_id: String,
    pub options: Vec<MirrorSourceOption>,
    pub selected_id: String,        // 当前选中的选项 ID
    pub current_value: String,      // 当前值
    pub has_user_override: bool,    // 是否有用户自定义
}

pub struct MirrorConfigManager;

impl MirrorConfigManager {
    /// 从嵌入的 JSON 数据加载默认镜像配置
    pub fn load_default_config() -> Result<serde_json::Value, String> {
        let json_data = include_str!("../../services/mirror_config.json");
        serde_json::from_str(json_data)
            .map_err(|e| format!("解析 mirror_config.json 失败: {}", e))
    }
    
    /// 获取所有类别 ID 列表
    pub fn get_category_ids() -> Vec<String> {
        vec![
            "docker_registry".to_string(),
            "apt".to_string(),
            "composer".to_string(),
            "npm".to_string(),
            "github_proxy".to_string(),
        ]
    }
    
    /// 获取合并后的镜像源列表（默认配置 + 用户自定义）
    pub fn get_merged_mirror_list(project_root: &Path) -> Result<Vec<MergedMirrorCategory>, String> {
        let default_config = Self::load_default_config()?;
        let user_config = UserMirrorConfig::load(project_root)?;
        
        let category_ids = Self::get_category_ids();
        let mut merged_list = Vec::new();
        
        for category_id in category_ids {
            // 从默认配置中获取该类别的选项列表
            let options_value = &default_config[&category_id];
            let options_array = options_value
                .as_array()
                .ok_or(format!("mirror_config.json 中缺少类别: {}", category_id))?;
            
            // 解析选项列表
            let options: Vec<MirrorSourceOption> = options_array
                .iter()
                .filter_map(|opt| {
                    serde_json::from_value(opt.clone()).ok()
                })
                .collect();
            
            if options.is_empty() {
                continue;
            }
            
            // 确定当前选中的选项
            let has_user_override = user_config.has_user_override(&category_id);
            let (selected_id, current_value, final_options) = if has_user_override {
                // 用户有自定义配置
                if let Some(user_cat) = user_config.get_category(&category_id) {
                    // 查找匹配的选项 ID
                    let matched_option = options.iter()
                        .find(|opt| opt.value == user_cat.source);
                    
                    if let Some(opt) = matched_option {
                        // 匹配到预设选项
                        (opt.id.clone(), user_cat.source.clone(), options.clone())
                    } else {
                        // 自定义地址，不匹配任何预设，添加 custom 选项
                        let mut opts_with_custom = options.clone();
                        opts_with_custom.push(MirrorSourceOption {
                            id: "custom".to_string(),
                            name: "自定义".to_string(),
                            value: user_cat.source.clone(),
                            description: user_cat.description.clone().unwrap_or_default(),
                        });
                        
                        ("custom".to_string(), user_cat.source.clone(), opts_with_custom)
                    }
                } else {
                    // 回退到第一个选项
                    (options[0].id.clone(), options[0].value.clone(), options.clone())
                }
            } else {
                // 使用默认配置的第一个选项
                (options[0].id.clone(), options[0].value.clone(), options.clone())
            };
            
            merged_list.push(MergedMirrorCategory {
                category_id,
                options: final_options,
                selected_id,
                current_value,
                has_user_override,
            });
        }
        
        Ok(merged_list)
    }
    
    /// 保存用户选择的镜像源选项
    pub fn save_selected_option(
        project_root: &Path,
        category_id: &str,
        option_id: &str,
    ) -> Result<(), String> {
        let default_config = Self::load_default_config()?;
        let options_value = &default_config[category_id];
        let options_array = options_value
            .as_array()
            .ok_or(format!("类别 {} 不存在", category_id))?;
        
        // 查找对应的选项值
        let selected_value = options_array.iter()
            .find(|opt| opt.get("id").and_then(|v| v.as_str()) == Some(option_id))
            .and_then(|opt| opt.get("value").and_then(|v| v.as_str()))
            .ok_or(format!("选项 {} 不存在于类别 {} 中", option_id, category_id))?;
        
        // 保存为用户自定义配置
        Self::save_user_category(project_root, category_id, selected_value, None)
    }
    
    /// 保存用户自定义的单个类别配置
    pub fn save_user_category(
        project_root: &Path,
        category_id: &str,
        source: &str,
        description: Option<String>,
    ) -> Result<(), String> {
        let mut user_config = UserMirrorConfig::load(project_root)?;
        
        let category = UserMirrorCategory {
            source: source.to_string(),
            enabled: true,
            description,
        };
        
        user_config.set_category(category_id.to_string(), category);
        user_config.save(project_root)
    }
    
    /// 删除用户自定义的类别配置
    pub fn remove_user_category(project_root: &Path, category_id: &str) -> Result<(), String> {
        let mut user_config = UserMirrorConfig::load(project_root)?;
        user_config.remove_category(category_id);
        user_config.save(project_root)
    }
    
    /// 重置所有用户自定义配置
    pub fn reset_all_overrides(project_root: &Path) -> Result<(), String> {
        let mut user_config = UserMirrorConfig::load(project_root)?;
        user_config.clear_all();
        user_config.save(project_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    
    fn create_test_project() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("创建临时目录失败");
        let path = dir.path().to_path_buf();
        (dir, path)
    }
    
    #[test]
    fn test_load_default_config() {
        let config = MirrorConfigManager::load_default_config();
        assert!(config.is_ok());
        
        let config = config.unwrap();
        assert!(config.get("docker_registry").is_some());
        assert!(config.get("apt").is_some());
        assert!(config.get("composer").is_some());
        assert!(config.get("npm").is_some());
        assert!(config.get("github_proxy").is_some());
    }
    
    #[test]
    fn test_user_config_save_and_load() {
        let (_dir, path) = create_test_project();
        
        let mut user_config = UserMirrorConfig::default();
        
        let category = UserMirrorCategory {
            source: "https://custom.mirror.com".to_string(),
            enabled: true,
            description: Some("Custom mirror".to_string()),
        };
        user_config.set_category("npm".to_string(), category);
        
        user_config.save(&path).expect("保存配置失败");
        
        let loaded = UserMirrorConfig::load(&path).expect("加载配置失败");
        assert!(loaded.has_user_override("npm"));
        assert_eq!(
            loaded.get_category("npm").unwrap().source,
            "https://custom.mirror.com"
        );
    }
    
    #[test]
    fn test_get_merged_mirror_list() {
        let (_dir, path) = create_test_project();
        
        let merged = MirrorConfigManager::get_merged_mirror_list(&path);
        assert!(merged.is_ok());
        
        let merged = merged.unwrap();
        assert!(!merged.is_empty());
        
        // 检查是否包含所有类别
        let category_ids: Vec<&String> = merged.iter().map(|m| &m.category_id).collect();
        assert!(category_ids.contains(&&"docker_registry".to_string()));
        assert!(category_ids.contains(&&"apt".to_string()));
        assert!(category_ids.contains(&&"composer".to_string()));
        assert!(category_ids.contains(&&"npm".to_string()));
        assert!(category_ids.contains(&&"github_proxy".to_string()));
    }
}
