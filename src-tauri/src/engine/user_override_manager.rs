use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::version_manifest::{VersionEntry, ServiceType, VersionManifest};
use crate::app_log;

/// 用户自定义的版本覆盖配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserVersionOverride {
    /// 覆盖的完整 Docker 镜像名（如 "php:8.2-fpm-alpine"）
    pub image_tag: String,
    /// 备注说明
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 用户版本覆盖管理器
pub struct UserOverrideManager {
    /// 默认版本清单
    default_manifest: VersionManifest,
    /// 用户覆盖配置
    user_overrides: HashMap<ServiceType, HashMap<String, UserVersionOverride>>,
}

impl UserOverrideManager {
    /// 创建新的覆盖管理器
    pub fn new(project_root: &Path) -> Self {
        let default_manifest = VersionManifest::new();
        let user_overrides = Self::load_user_overrides(project_root);
        
        Self {
            default_manifest,
            user_overrides,
        }
    }

    /// 从文件加载用户覆盖配置
    fn load_user_overrides(
        project_root: &Path,
    ) -> HashMap<ServiceType, HashMap<String, UserVersionOverride>> {
        // 使用 project_root 作为配置文件存放位置（与 .env 同级）
        let overrides_path = project_root.join(".user_version_overrides.json");
        
        if !overrides_path.exists() {
            app_log!(info, "engine::user_override", "未找到用户覆盖配置文件，使用默认配置");
            return HashMap::new();
        }

        app_log!(info, "engine::user_override", "加载用户覆盖配置: {:?}", overrides_path);

        match std::fs::read_to_string(&overrides_path) {
            Ok(content) => {
                match serde_json::from_str::<HashMap<String, HashMap<String, UserVersionOverride>>>(&content) {
                    Ok(raw) => {
                        let mut result = HashMap::new();
                        let mut override_count = 0;
                        
                        for (service_key, versions) in raw {
                            let service_type = match service_key.as_str() {
                                "php" => ServiceType::Php,
                                "mysql" => ServiceType::Mysql,
                                "redis" => ServiceType::Redis,
                                "nginx" => ServiceType::Nginx,
                                _ => continue,
                            };
                            override_count += versions.len();
                            app_log!(info, "engine::user_override", "{service_key}: {} 个版本覆盖", versions.len());
                            result.insert(service_type, versions);
                        }
                        
                        app_log!(info, "engine::user_override", "加载成功，共 {} 个服务类型，{override_count} 个版本覆盖", result.len());
                        result
                    }
                    Err(e) => {
                        app_log!(warn, "engine::user_override", "解析配置文件失败: {e}");
                        HashMap::new()
                    }
                }
            }
            Err(e) => {
                app_log!(error, "engine::user_override", "读取配置文件失败: {e}");
                HashMap::new()
            }
        }
    }

    /// 获取合并后的版本条目（用户覆盖 > 默认配置）
    /// 用户覆盖仅替换 image_tag（和可选的 description），其他字段保持 manifest 默认值
    pub fn get_merged_entry(
        &self,
        service_type: &ServiceType,
        id: &str,
    ) -> Option<VersionEntry> {
        // 1. 检查用户是否有覆盖配置
        if let Some(user_override) = self
            .user_overrides
            .get(service_type)
            .and_then(|entries| entries.get(id))
        {
            app_log!(info, "engine::user_override", "{} {} 使用自定义标签: {}",
                format!("{service_type:?}").to_lowercase(),
                id, 
                user_override.image_tag);
            
            // 2. 获取默认配置作为基础
            if let Some(default_info) = self.default_manifest.get_entry(service_type, id) {
                // 3. 返回合并后的配置（用户覆盖的 image_tag 优先）
                return Some(VersionEntry {
                    display_name: default_info.display_name.clone(),
                    image_tag: user_override.image_tag.clone(), // 使用用户的镜像名
                    service_dir: default_info.service_dir.clone(),
                    default_port: default_info.default_port,
                    show_port: default_info.show_port,
                    eol: default_info.eol,
                    description: user_override.description.clone().or_else(|| default_info.description.clone()),
                });
            }
        }

        // 4. 没有用户覆盖，返回默认配置
        self.default_manifest.get_entry(service_type, id).cloned()
    }

    /// 保存用户覆盖配置到文件
    pub fn save_user_override(
        &mut self,
        project_root: &Path,
        service_type: ServiceType,
        id: String,
        override_config: UserVersionOverride,
    ) -> Result<(), String> {
        // 更新内存中的配置
        self.user_overrides
            .entry(service_type)
            .or_default()
            .insert(id, override_config);

        // 序列化并保存到文件（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        let json = serde_json::to_string_pretty(&self.user_overrides)
            .map_err(|e| format!("序列化失败: {e}"))?;

        std::fs::write(&overrides_path, json)
            .map_err(|e| format!("写入文件失败: {e}"))?;

        Ok(())
    }

    /// 删除用户覆盖配置
    pub fn remove_user_override(
        &mut self,
        project_root: &Path,
        service_type: &ServiceType,
        id: &str,
    ) -> Result<(), String> {
        if let Some(entries) = self.user_overrides.get_mut(service_type) {
            entries.remove(id);
        }

        // 重新保存（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        let json = serde_json::to_string_pretty(&self.user_overrides)
            .map_err(|e| format!("序列化失败: {e}"))?;

        std::fs::write(&overrides_path, json)
            .map_err(|e| format!("写入文件失败: {e}"))?;

        Ok(())
    }

    /// 重置所有用户覆盖（恢复到默认配置）
    pub fn reset_all_overrides(&mut self, project_root: &Path) -> Result<(), String> {
        self.user_overrides.clear();
        
        // 删除配置文件（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        if overrides_path.exists() {
            std::fs::remove_file(&overrides_path)
                .map_err(|e| format!("删除文件失败: {e}"))?;
        }

        Ok(())
    }

    /// 检查指定 ID 是否有用户覆盖配置
    pub fn has_user_override(&self, service_type: &ServiceType, id: &str) -> bool {
        self.user_overrides
            .get(service_type)
            .and_then(|entries| entries.get(id))
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_load_nonexistent_overrides() {
        let temp_dir = env::temp_dir();
        let manager = UserOverrideManager::new(&temp_dir);
        assert!(manager.user_overrides.is_empty());
    }

    #[test]
    fn test_get_default_when_no_override() {
        let temp_dir = env::temp_dir();
        let manager = UserOverrideManager::new(&temp_dir);
        
        let info = manager.get_merged_entry(&ServiceType::Mysql, "mysql80");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.image_tag, "mysql:8.0");
    }
}
