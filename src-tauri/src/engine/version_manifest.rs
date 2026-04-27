use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 版本条目 — 每条记录自描述，包含所有下游需要的信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    /// 前端显示名称（如 "PHP 8.2"）
    pub display_name: String,
    /// 完整 Docker 镜像名（如 "php:8.2-fpm"），可直接 docker pull
    pub image_tag: String,
    /// 配置目录名（如 "php82"），与 services/ 下的子目录一致
    pub service_dir: String,
    /// 默认端口
    pub default_port: u16,
    /// 是否在 UI 中显示端口配置
    pub show_port: bool,
    /// 是否已停止维护 (End of Life)
    #[serde(default)]
    pub eol: bool,
    /// 版本描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
    /// 所有服务的版本映射，key 为 ID（如 "php82"）
    versions: HashMap<ServiceType, HashMap<String, VersionEntry>>,
}

impl VersionManifest {
    /// 从嵌入的 JSON 数据加载版本清单
    pub fn new() -> Self {
        let json_data = include_str!("../../services/version_manifest.json");
        let raw: HashMap<String, HashMap<String, VersionEntry>> =
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

    // ─── 新 API ───────────────────────────────────────────────

    /// 按 ID 查询版本条目
    pub fn get_entry(&self, service_type: &ServiceType, id: &str) -> Option<&VersionEntry> {
        self.versions
            .get(service_type)
            .and_then(|entries| entries.get(id))
    }

    /// 按 env 变量前缀反查版本条目
    /// 将 prefix 转小写后匹配 service_dir（如 "PHP82" → "php82"）
    pub fn find_entry_by_env_prefix(
        &self,
        service_type: &ServiceType,
        prefix: &str,
    ) -> Option<(&String, &VersionEntry)> {
        let prefix_lower = prefix.to_lowercase();
        self.versions
            .get(service_type)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|(_, entry)| entry.service_dir == prefix_lower)
            })
    }

    /// 获取指定服务的所有可用版本条目，按版本号降序排列
    /// 返回 Vec<(&String, &VersionEntry)>，其中 String 为 ID
    pub fn get_available_entries(
        &self,
        service_type: &ServiceType,
    ) -> Vec<(&String, &VersionEntry)> {
        let mut entries: Vec<(&String, &VersionEntry)> = self
            .versions
            .get(service_type)
            .map(|e| e.iter().collect())
            .unwrap_or_default();

        // 按版本号降序排列 — 从 ID 中提取数字部分进行比较
        entries.sort_by(|(id_a, _), (id_b, _)| {
            let ver_a = extract_version_numbers(id_a);
            let ver_b = extract_version_numbers(id_b);
            ver_b.cmp(&ver_a)
        });

        entries
    }

    /// 获取推荐版本（非 EOL 的最新版本）
    pub fn get_recommended_entry(
        &self,
        service_type: &ServiceType,
    ) -> Option<(&String, &VersionEntry)> {
        self.get_available_entries(service_type)
            .into_iter()
            .find(|(_, entry)| !entry.eol)
    }

    /// 检查 ID 是否存在
    pub fn is_id_valid(&self, service_type: &ServiceType, id: &str) -> bool {
        self.get_entry(service_type, id).is_some()
    }

    /// 获取版本警告信息（如果是 EOL 版本）
    pub fn get_entry_warning(&self, service_type: &ServiceType, id: &str) -> Option<String> {
        self.get_entry(service_type, id)
            .filter(|entry| entry.eol)
            .and_then(|entry| {
                entry.description.as_ref().map(|desc| {
                    format!("⚠️ {desc} - 建议使用更新版本")
                })
            })
    }

}

/// 从 ID 中提取版本数字用于排序（如 "php82" → (8, 2), "nginx128" → (1, 28)）
fn extract_version_numbers(id: &str) -> (u32, u32, u32) {
    // 去掉前缀字母，保留数字部分
    let digits: String = id.chars().skip_while(|c| c.is_alphabetic()).collect();

    // 尝试按常见模式解析
    // 对于两位数字如 "82" → (8, 2)，三位数字如 "128" → (1, 28)
    // 使用启发式：如果数字部分长度 <= 2，视为 (major, minor)
    // 如果长度 == 3，视为 (major=1位, minor=2位)
    // 如果长度 >= 4，视为 (major=2位, minor=2位)
    let num: u64 = digits.parse().unwrap_or(0);

    match digits.len() {
        0 => (0, 0, 0),
        1 => (num as u32, 0, 0),
        2 => ((num / 10) as u32, (num % 10) as u32, 0),
        3 => ((num / 100) as u32, (num % 100) as u32, 0),
        _ => ((num / 100) as u32, (num % 100) as u32, 0),
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
        // 应该有 4 种服务类型
        assert_eq!(manifest.versions.len(), 4);
    }

    #[test]
    fn test_get_entry() {
        let manifest = VersionManifest::new();

        // 测试 MySQL 8.4
        let entry = manifest.get_entry(&ServiceType::Mysql, "mysql84");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.display_name, "MySQL 8.4 LTS");
        assert_eq!(entry.image_tag, "mysql:8.4");
        assert_eq!(entry.service_dir, "mysql84");
        assert_eq!(entry.default_port, 3306);
        assert!(entry.show_port);
        assert!(!entry.eol);

        // 测试 PHP 8.2
        let entry = manifest.get_entry(&ServiceType::Php, "php82");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.display_name, "PHP 8.2");
        assert_eq!(entry.image_tag, "php:8.2-fpm");
        assert_eq!(entry.service_dir, "php82");
        assert!(!entry.show_port);
    }

    #[test]
    fn test_id_validation() {
        let manifest = VersionManifest::new();

        assert!(manifest.is_id_valid(&ServiceType::Mysql, "mysql80"));
        assert!(manifest.is_id_valid(&ServiceType::Mysql, "mysql84"));
        assert!(!manifest.is_id_valid(&ServiceType::Mysql, "mysql90"));
    }

    #[test]
    fn test_eol_detection() {
        let manifest = VersionManifest::new();

        // MySQL 5.7 应该是 EOL
        let warning = manifest.get_entry_warning(&ServiceType::Mysql, "mysql57");
        assert!(warning.is_some());

        // MySQL 8.0 应该不是 EOL
        let warning = manifest.get_entry_warning(&ServiceType::Mysql, "mysql80");
        assert!(warning.is_none());
    }

    #[test]
    fn test_find_entry_by_env_prefix() {
        let manifest = VersionManifest::new();

        // PHP82 → php82
        let result = manifest.find_entry_by_env_prefix(&ServiceType::Php, "PHP82");
        assert!(result.is_some());
        let (id, entry) = result.unwrap();
        assert_eq!(id, "php82");
        assert_eq!(entry.image_tag, "php:8.2-fpm");

        // MYSQL84 → mysql84
        let result = manifest.find_entry_by_env_prefix(&ServiceType::Mysql, "MYSQL84");
        assert!(result.is_some());
        let (id, _) = result.unwrap();
        assert_eq!(id, "mysql84");

        // 不存在的前缀
        let result = manifest.find_entry_by_env_prefix(&ServiceType::Php, "PHP99");
        assert!(result.is_none());
    }

    #[test]
    fn test_recommended_entry() {
        let manifest = VersionManifest::new();

        let recommended = manifest.get_recommended_entry(&ServiceType::Mysql);
        assert!(recommended.is_some());

        // 应该是 mysql84（最新的非 EOL 版本）
        let (id, entry) = recommended.unwrap();
        assert_eq!(id, "mysql84");
        assert!(!entry.eol);
    }

    #[test]
    fn test_available_entries_sorted() {
        let manifest = VersionManifest::new();

        let entries = manifest.get_available_entries(&ServiceType::Php);
        assert!(!entries.is_empty());

        // 验证按版本号降序排列
        let ids: Vec<&str> = entries.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids[0], "php85"); // 最新版本在前
        assert_eq!(*ids.last().unwrap(), "php56"); // 最旧版本在后

        // 验证包含所有 PHP 版本
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn test_available_entries_nginx_sorted() {
        let manifest = VersionManifest::new();

        let entries = manifest.get_available_entries(&ServiceType::Nginx);
        let ids: Vec<&str> = entries.iter().map(|(id, _)| id.as_str()).collect();

        // Nginx 版本应该按降序排列：128, 127, 126, 125, 124
        assert_eq!(ids[0], "nginx128");
        assert_eq!(*ids.last().unwrap(), "nginx124");
    }

    #[test]
    fn test_extract_version_numbers() {
        assert_eq!(extract_version_numbers("php82"), (8, 2, 0));
        assert_eq!(extract_version_numbers("php56"), (5, 6, 0));
        assert_eq!(extract_version_numbers("mysql84"), (8, 4, 0));
        assert_eq!(extract_version_numbers("nginx128"), (1, 28, 0));
        assert_eq!(extract_version_numbers("redis72"), (7, 2, 0));
    }
}
