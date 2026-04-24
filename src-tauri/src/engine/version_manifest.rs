use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Docker 镜像信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Docker 镜像名称（如 "mysql", "php"）
    pub image: String,
    /// Docker 镜像标签（如 "8.0", "8.4-lts"）
    pub tag: String,
    /// 是否已停止维护 (End of Life)
    #[serde(default)]
    pub eol: bool,
    /// 版本描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ImageInfo {
    /// 获取完整的镜像名称（image:tag）
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.image, self.tag)
    }
}

/// 服务类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Php,
    Mysql,
    Redis,
    Nginx,
}

/// 版本清单管理器
pub struct VersionManifest {
    /// 所有服务的版本映射
    versions: HashMap<ServiceType, HashMap<String, ImageInfo>>,
}

impl VersionManifest {
    /// 从嵌入的 JSON 数据加载版本清单
    pub fn new() -> Self {
        let json_data = include_str!("../../services/version_manifest.json");
        let raw: HashMap<String, HashMap<String, ImageInfo>> =
            serde_json::from_str(json_data).expect("Failed to parse version_manifest.json");

        let mut versions = HashMap::new();

        // 转换键为 ServiceType 枚举
        for (service_key, service_versions) in raw {
            let service_type = match service_key.as_str() {
                "php" => ServiceType::Php,
                "mysql" => ServiceType::Mysql,
                "redis" => ServiceType::Redis,
                "nginx" => ServiceType::Nginx,
                _ => continue,
            };
            versions.insert(service_type, service_versions);
        }

        Self { versions }
    }

    /// 获取指定服务和版本的镜像信息
    pub fn get_image_info(
        &self,
        service_type: &ServiceType,
        version: &str,
    ) -> Option<&ImageInfo> {
        self.versions
            .get(service_type)
            .and_then(|versions| versions.get(version))
    }

    /// 获取完整的镜像名称（image:tag）
    pub fn get_full_image_name(&self, service_type: &ServiceType, version: &str) -> Option<String> {
        self.get_image_info(service_type, version)
            .map(|info| info.full_name())
    }

    /// 检查版本是否存在
    pub fn is_version_valid(&self, service_type: &ServiceType, version: &str) -> bool {
        self.get_image_info(service_type, version).is_some()
    }

    /// 获取指定服务的所有可用版本
    pub fn get_available_versions(&self, service_type: &ServiceType) -> Vec<&String> {
        self.versions
            .get(service_type)
            .map(|versions| versions.keys().collect())
            .unwrap_or_default()
    }

    /// 获取推荐版本（非 EOL 的最新版本）
    pub fn get_recommended_version(&self, service_type: &ServiceType) -> Option<&String> {
        self.versions
            .get(service_type)
            .and_then(|versions| {
                versions
                    .iter()
                    .filter(|(_, info)| !info.eol)
                    .max_by_key(|(version, _)| parse_version_for_comparison(version))
                    .map(|(version, _)| version)
            })
    }

    /// 验证并规范化版本号
    /// 如果用户输入的版本不存在，返回最接近的可用版本
    pub fn normalize_version(&self, service_type: &ServiceType, version: &str) -> Option<String> {
        // 首先尝试精确匹配
        if self.is_version_valid(service_type, version) {
            return Some(version.to_string());
        }

        // 尝试模糊匹配（例如 "8" -> "8.0"）
        let available = self.get_available_versions(service_type);
        let best_match = available
            .iter()
            .find(|v| v.starts_with(&format!("{version}.")) || version.starts_with(&format!("{v}.")));

        best_match.map(|v| (*v).to_string())
    }

    /// 获取版本警告信息（如果是 EOL 版本）
    pub fn get_version_warning(&self, service_type: &ServiceType, version: &str) -> Option<String> {
        self.get_image_info(service_type, version)
            .filter(|info| info.eol)
            .and_then(|info| {
                info.description.as_ref().map(|desc| {
                    format!("⚠️ {desc} - 建议使用更新版本")
                })
            })
    }
}

/// 简单的版本比较辅助函数
fn parse_version_for_comparison(version: &str) -> (u32, u32, u32) {
    let parts: Vec<u32> = version
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    match parts.len() {
        0 => (0, 0, 0),
        1 => (parts[0], 0, 0),
        2 => (parts[0], parts[1], 0),
        _ => (parts[0], parts[1], parts[2]),
    }
}

impl Default for VersionManifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_manifest() {
        let manifest = VersionManifest::new();
        assert!(!manifest.versions.is_empty());
    }

    #[test]
    fn test_get_image_info() {
        let manifest = VersionManifest::new();
        
        // 测试 MySQL 8.4
        let info = manifest.get_image_info(&ServiceType::Mysql, "8.4");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.tag, "8.4");
        assert_eq!(info.full_name(), "mysql:8.4");
    }

    #[test]
    fn test_version_validation() {
        let manifest = VersionManifest::new();
        
        assert!(manifest.is_version_valid(&ServiceType::Mysql, "8.0"));
        assert!(manifest.is_version_valid(&ServiceType::Mysql, "8.4"));
        assert!(!manifest.is_version_valid(&ServiceType::Mysql, "9.0"));
    }

    #[test]
    fn test_eol_detection() {
        let manifest = VersionManifest::new();
        
        // MySQL 5.7 应该是 EOL
        let warning = manifest.get_version_warning(&ServiceType::Mysql, "5.7");
        assert!(warning.is_some());
        
        // MySQL 8.0 应该不是 EOL
        let warning = manifest.get_version_warning(&ServiceType::Mysql, "8.0");
        assert!(warning.is_none());
    }

    #[test]
    fn test_version_normalization() {
        let manifest = VersionManifest::new();
        
        // 精确匹配
        let normalized = manifest.normalize_version(&ServiceType::Mysql, "8.0");
        assert_eq!(normalized, Some("8.0".to_string()));
        
        // 无效版本
        let normalized = manifest.normalize_version(&ServiceType::Mysql, "99.0");
        assert!(normalized.is_none());
    }

    #[test]
    fn test_recommended_version() {
        let manifest = VersionManifest::new();
        
        let recommended = manifest.get_recommended_version(&ServiceType::Mysql);
        assert!(recommended.is_some());
        
        // 应该是 8.4（最新的非 EOL 版本）
        let rec = recommended.unwrap();
        assert_eq!(rec, "8.4");
    }

    #[test]
    fn test_available_versions() {
        let manifest = VersionManifest::new();
        
        let versions = manifest.get_available_versions(&ServiceType::Php);
        assert!(!versions.is_empty());
        assert!(versions.contains(&&"8.2".to_string()));
    }
}
