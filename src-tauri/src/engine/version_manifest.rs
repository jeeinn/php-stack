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

/// 清单溯源信息 —— 记录模板与版本数据的来源，便于后续核对与自动同步。
///
/// 这是给维护者看的元信息，不影响运行时行为：所有字段均为可选，
/// 缺失时解析照常成功。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestProvenance {
    /// 清单最后更新时间（YYYY-MM-DD）
    #[serde(default)]
    pub updated_at: Option<String>,
    /// 各类数据的上游来源，如 {"versions": "...", "eol": "..."}
    #[serde(default)]
    pub sources: Option<HashMap<String, String>>,
    /// 配置基线的提取方式说明
    #[serde(default)]
    pub config_baseline: Option<String>,
    /// 补充说明
    #[serde(default)]
    pub notes: Option<String>,
}

/// `version_manifest.json` 的整体结构。
///
/// 服务类型显式列出（而非直接反序列化为 `HashMap<String, HashMap<..>>`），
/// 这样 `_provenance` 这类元信息键不会因类型不匹配导致整份清单解析失败。
#[derive(Debug, Deserialize)]
struct ManifestFile {
    #[serde(default)]
    _provenance: Option<ManifestProvenance>,
    #[serde(default)]
    php: HashMap<String, VersionEntry>,
    #[serde(default)]
    mysql: HashMap<String, VersionEntry>,
    #[serde(default)]
    redis: HashMap<String, VersionEntry>,
    #[serde(default)]
    nginx: HashMap<String, VersionEntry>,
}

/// 版本清单管理器
pub struct VersionManifest {
    /// 所有服务的版本映射，key 为 ID（如 "php82"）
    versions: HashMap<ServiceType, HashMap<String, VersionEntry>>,
    /// 清单溯源信息（可选）
    provenance: Option<ManifestProvenance>,
}

impl VersionManifest {
    /// 加载版本清单：优先 `app_data_dir/services/version_manifest.json`，失败或缺失时用内置嵌入。
    ///
    /// 用户可在不重新发版的情况下覆盖清单（例如同步脚本生成新版本后拷贝到应用数据目录）。
    pub fn new() -> Self {
        if let Some(m) = Self::try_load_override() {
            return m;
        }
        Self::from_embedded()
    }

    /// 从嵌入的 JSON 数据加载版本清单
    fn from_embedded() -> Self {
        let json_data = include_str!("../../services/version_manifest.json");
        Self::from_json(json_data).unwrap_or_else(|e| {
            panic!("Failed to parse embedded version_manifest.json: {e}");
        })
    }

    /// 尝试从用户覆盖路径加载；解析失败时记日志并回退内置。
    fn try_load_override() -> Option<Self> {
        let path = crate::commands::paths::app_data_dir()
            .ok()?
            .join("services")
            .join("version_manifest.json");
        if !path.is_file() {
            return None;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match Self::from_json(&content) {
                Ok(m) => {
                    crate::app_log!(
                        info,
                        "engine::version_manifest",
                        "Loaded external version manifest: {}",
                        path.display()
                    );
                    Some(m)
                }
                Err(e) => {
                    crate::app_log!(
                        warn,
                        "engine::version_manifest",
                        "External manifest parse failed, using built-in: {} ({e})",
                        path.display()
                    );
                    None
                }
            },
            Err(e) => {
                crate::app_log!(
                    warn,
                    "engine::version_manifest",
                    "Failed to read external manifest, using built-in: {} ({e})",
                    path.display()
                );
                None
            }
        }
    }

    /// 从 JSON 字符串解析清单（供单测与外部覆盖共用）
    pub fn from_json(json_data: &str) -> Result<Self, String> {
        let file: ManifestFile = serde_json::from_str(json_data)
            .map_err(|e| format!("failed to parse version_manifest: {e}"))?;

        let mut versions = HashMap::new();
        versions.insert(ServiceType::Php, file.php);
        versions.insert(ServiceType::Mysql, file.mysql);
        versions.insert(ServiceType::Redis, file.redis);
        versions.insert(ServiceType::Nginx, file.nginx);

        Ok(Self {
            versions,
            provenance: file._provenance,
        })
    }

    /// 清单溯源信息（若清单中未声明则为 None）
    pub fn provenance(&self) -> Option<&ManifestProvenance> {
        self.provenance.as_ref()
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
        self.versions.get(service_type).and_then(|entries| {
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
                entry
                    .description
                    .as_ref()
                    .map(|desc| format!("⚠️ {desc} - 建议使用更新版本"))
            })
    }
}

/// 从 ID 中提取版本数字用于排序（如 "php82" → (8, 2), "nginx128" → (1, 28)）
pub(crate) fn extract_version_numbers(id: &str) -> (u32, u32, u32) {
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
    fn test_from_json_parses_minimal_manifest() {
        let json = r#"{
            "php": {
              "php99": {
                "display_name": "PHP 9.9",
                "image_tag": "php:9.9-fpm",
                "service_dir": "php99",
                "default_port": 9000,
                "show_port": false,
                "eol": false
              }
            },
            "mysql": {},
            "redis": {},
            "nginx": {}
        }"#;
        let m = VersionManifest::from_json(json).expect("minimal manifest should parse");
        assert!(m.is_id_valid(&ServiceType::Php, "php99"));
        assert_eq!(
            m.get_entry(&ServiceType::Php, "php99").unwrap().image_tag,
            "php:9.9-fpm"
        );
    }

    #[test]
    fn test_from_json_rejects_invalid() {
        assert!(
            VersionManifest::from_json("{not json").is_err(),
            "非法 JSON 应返回 Err"
        );
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

        // 推荐 = 按版本号降序后的第一个非 EOL（不硬编码 ID，避免 sync 增版后失败）
        let (id, entry) = recommended.unwrap();
        assert!(!entry.eol);
        let expected = manifest
            .get_available_entries(&ServiceType::Mysql)
            .into_iter()
            .find(|(_, e)| !e.eol)
            .map(|(i, _)| i.as_str());
        assert_eq!(Some(id.as_str()), expected);
    }

    #[test]
    fn test_available_entries_sorted() {
        let manifest = VersionManifest::new();

        let entries = manifest.get_available_entries(&ServiceType::Php);
        assert!(!entries.is_empty());

        // 验证按版本号降序排列（不硬编码 ID，避免 sync 增版后失败）
        let versions: Vec<_> = entries
            .iter()
            .map(|(id, _)| extract_version_numbers(id))
            .collect();
        for window in versions.windows(2) {
            assert!(window[0] >= window[1], "PHP entries must be sorted descending");
        }
        assert!(
            entries.iter().any(|(id, _)| id.as_str() == "php56"),
            "expected php56 to remain in the manifest"
        );
    }

    #[test]
    fn test_available_entries_nginx_sorted() {
        let manifest = VersionManifest::new();

        let entries = manifest.get_available_entries(&ServiceType::Nginx);
        assert!(!entries.is_empty());

        // Nginx 按 extract_version_numbers 降序（不硬编码最新 ID）
        let versions: Vec<_> = entries
            .iter()
            .map(|(id, _)| extract_version_numbers(id))
            .collect();
        for window in versions.windows(2) {
            assert!(
                window[0] >= window[1],
                "Nginx entries must be sorted descending"
            );
        }
        assert_eq!(
            *entries.last().unwrap().0,
            "nginx124",
            "oldest nginx in manifest should sort last"
        );
    }

    #[test]
    fn test_extract_version_numbers() {
        assert_eq!(extract_version_numbers("php82"), (8, 2, 0));
        assert_eq!(extract_version_numbers("php56"), (5, 6, 0));
        assert_eq!(extract_version_numbers("mysql84"), (8, 4, 0));
        assert_eq!(extract_version_numbers("nginx128"), (1, 28, 0));
        assert_eq!(extract_version_numbers("redis72"), (7, 2, 0));
    }

    // ─── _provenance 溯源信息 ────────────────────────────────────
    // 这些测试锁住「元信息可选、不影响主流程解析」的契约：
    // 缺少 _provenance 或只写部分字段时，清单仍必须能正常加载。

    #[test]
    fn test_provenance_parsed_from_embedded_manifest() {
        let manifest = VersionManifest::new();
        let provenance = manifest
            .provenance()
            .expect("内置清单应声明 _provenance，否则无法追溯数据来源");

        // 只校验 YYYY-MM-DD 格式，不锁具体日期（每次同步都会变）
        let updated_at = provenance
            .updated_at
            .as_deref()
            .expect("_provenance.updated_at 应有值");
        assert_eq!(updated_at.len(), 10, "updated_at 应为 YYYY-MM-DD 格式");
        assert_eq!(&updated_at[4..5], "-", "updated_at 第 5 位应为 -");
        assert_eq!(&updated_at[7..8], "-", "updated_at 第 8 位应为 -");

        let sources = provenance
            .sources
            .as_ref()
            .expect("_provenance.sources 应有值");
        assert!(!sources.is_empty(), "sources 不应为空");
        // 这两个来源是版本数据的权威入口，缺失即失去溯源意义
        assert!(sources.contains_key("versions"), "缺少 versions 来源");
        assert!(sources.contains_key("eol"), "缺少 eol 来源");

        assert!(provenance.config_baseline.is_some(), "缺少 config_baseline");
        assert!(provenance.notes.is_some(), "缺少 notes");
    }

    #[test]
    fn test_provenance_absent_when_not_declared() {
        let json = r#"{
            "php": {
                "php85": {
                    "display_name": "PHP 8.5",
                    "image_tag": "php:8.5-fpm",
                    "service_dir": "php85",
                    "default_port": 9000,
                    "show_port": false
                }
            }
        }"#;
        let file: ManifestFile =
            serde_json::from_str(json).expect("未声明 _provenance 的清单应正常解析");
        assert!(file._provenance.is_none(), "未声明 _provenance 时应为 None");
        assert_eq!(file.php.len(), 1, "版本条目本身不应受影响");
    }

    #[test]
    fn test_provenance_tolerates_partial_fields() {
        // 只声明 updated_at，其余字段全部缺失 —— 所有字段可选，解析不应失败
        let json = r#"{
            "_provenance": { "updated_at": "2026-01-01" },
            "mysql": {}
        }"#;
        let file: ManifestFile = serde_json::from_str(json).expect("字段部分缺失时仍应解析成功");
        let provenance = file._provenance.expect("_provenance 应被解析");

        assert_eq!(provenance.updated_at.as_deref(), Some("2026-01-01"));
        assert!(provenance.sources.is_none(), "未声明的 sources 应为 None");
        assert!(
            provenance.config_baseline.is_none(),
            "未声明的 config_baseline 应为 None"
        );
        assert!(provenance.notes.is_none(), "未声明的 notes 应为 None");
    }

    #[test]
    fn test_provenance_ignores_unknown_shape() {
        // 即使是完全陌生的嵌套结构（而非字符串/对象），也不能让整份清单解析失败
        let json = r#"{
            "_provenance": {
                "updated_at": "2026-02-02",
                "sources": { "future_key": "some://url" },
                "unknown_meta": { "anything": 42 }
            },
            "redis": {}
        }"#;
        let file: ManifestFile =
            serde_json::from_str(json).expect("含未知字段的 _provenance 应被容错");
        let provenance = file._provenance.expect("_provenance 应被解析");

        assert_eq!(provenance.updated_at.as_deref(), Some("2026-02-02"));
        let sources = provenance.sources.expect("sources 应被解析");
        assert_eq!(
            sources.get("future_key").map(String::as_str),
            Some("some://url")
        );
    }
}
