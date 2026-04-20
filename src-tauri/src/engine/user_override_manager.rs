use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::version_manifest::{ImageInfo, ServiceType, VersionManifest};

/// 用户自定义的版本覆盖配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserVersionOverride {
    /// Docker 镜像标签
    pub tag: String,
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
    pub fn new(project_root: &PathBuf) -> Self {
        let default_manifest = VersionManifest::new();
        let user_overrides = Self::load_user_overrides(project_root);
        
        Self {
            default_manifest,
            user_overrides,
        }
    }

    /// 从文件加载用户覆盖配置
    fn load_user_overrides(
        project_root: &PathBuf,
    ) -> HashMap<ServiceType, HashMap<String, UserVersionOverride>> {
        // 使用 project_root 作为配置文件存放位置（与 .env 同级）
        let overrides_path = project_root.join(".user_version_overrides.json");
        
        if !overrides_path.exists() {
            return HashMap::new();
        }

        match std::fs::read_to_string(&overrides_path) {
            Ok(content) => {
                match serde_json::from_str::<HashMap<String, HashMap<String, UserVersionOverride>>>(&content) {
                    Ok(raw) => {
                        let mut result = HashMap::new();
                        for (service_key, versions) in raw {
                            let service_type = match service_key.as_str() {
                                "php" => ServiceType::Php,
                                "mysql" => ServiceType::Mysql,
                                "redis" => ServiceType::Redis,
                                "nginx" => ServiceType::Nginx,
                                _ => continue,
                            };
                            result.insert(service_type, versions);
                        }
                        result
                    }
                    Err(e) => {
                        eprintln!("警告: 无法解析用户覆盖配置文件: {}", e);
                        HashMap::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("警告: 无法读取用户覆盖配置文件: {}", e);
                HashMap::new()
            }
        }
    }

    /// 获取合并后的镜像信息（用户覆盖 > 默认配置）
    pub fn get_merged_image_info(
        &self,
        service_type: &ServiceType,
        version: &str,
    ) -> Option<ImageInfo> {
        // 1. 检查用户是否有覆盖配置
        if let Some(user_override) = self
            .user_overrides
            .get(service_type)
            .and_then(|versions| versions.get(version))
        {
            // 2. 获取默认配置作为基础
            if let Some(default_info) = self.default_manifest.get_image_info(service_type, version) {
                // 3. 返回合并后的配置（用户覆盖优先）
                return Some(ImageInfo {
                    image: default_info.image.clone(),
                    tag: user_override.tag.clone(), // 使用用户的标签
                    eol: default_info.eol,
                    description: user_override.description.clone().or_else(|| default_info.description.clone()),
                });
            }
        }

        // 4. 没有用户覆盖，返回默认配置
        self.default_manifest.get_image_info(service_type, version).cloned()
    }

    /// 保存用户覆盖配置到文件
    pub fn save_user_override(
        &mut self,
        project_root: &PathBuf,
        service_type: ServiceType,
        version: String,
        override_config: UserVersionOverride,
    ) -> Result<(), String> {
        // 更新内存中的配置
        self.user_overrides
            .entry(service_type)
            .or_insert_with(HashMap::new)
            .insert(version, override_config);

        // 序列化并保存到文件（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        let json = serde_json::to_string_pretty(&self.user_overrides)
            .map_err(|e| format!("序列化失败: {}", e))?;

        std::fs::write(&overrides_path, json)
            .map_err(|e| format!("写入文件失败: {}", e))?;

        Ok(())
    }

    /// 删除用户覆盖配置
    pub fn remove_user_override(
        &mut self,
        project_root: &PathBuf,
        service_type: &ServiceType,
        version: &str,
    ) -> Result<(), String> {
        if let Some(versions) = self.user_overrides.get_mut(service_type) {
            versions.remove(version);
        }

        // 重新保存（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        let json = serde_json::to_string_pretty(&self.user_overrides)
            .map_err(|e| format!("序列化失败: {}", e))?;

        std::fs::write(&overrides_path, json)
            .map_err(|e| format!("写入文件失败: {}", e))?;

        Ok(())
    }

    /// 重置所有用户覆盖（恢复到默认配置）
    pub fn reset_all_overrides(&mut self, project_root: &PathBuf) -> Result<(), String> {
        self.user_overrides.clear();
        
        // 删除配置文件（与 .env 同级目录）
        let overrides_path = project_root.join(".user_version_overrides.json");
        if overrides_path.exists() {
            std::fs::remove_file(&overrides_path)
                .map_err(|e| format!("删除文件失败: {}", e))?;
        }

        Ok(())
    }

    /// 检查指定版本是否有用户覆盖配置
    pub fn has_user_override(&self, service_type: &ServiceType, version: &str) -> bool {
        self.user_overrides
            .get(service_type)
            .and_then(|versions| versions.get(version))
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
        
        let info = manager.get_merged_image_info(&ServiceType::Mysql, "8.0");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.tag, "8.0");
    }
}
