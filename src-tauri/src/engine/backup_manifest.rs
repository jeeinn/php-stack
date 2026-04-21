use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 备份选项
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupOptions {
    pub include_projects: bool,
    pub project_patterns: Vec<String>,
    pub include_logs: bool,
}

/// 服务信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestService {
    pub name: String,
    pub image: String,
    pub version: String,
    pub ports: HashMap<u16, u16>, // host_port -> container_port
}

/// 备份清单
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupManifest {
    /// 备份格式版本号
    pub version: String,
    /// 创建时间戳（ISO 8601）
    pub timestamp: String,
    /// php-stack 应用版本
    pub app_version: String,
    /// 操作系统信息
    pub os_info: String,
    /// 服务列表
    pub services: Vec<ManifestService>,
    /// 备份选项
    pub options: BackupOptions,
    /// 文件清单（路径 → SHA256）
    pub files: HashMap<String, String>,
    /// 错误信息（部分失败时记录）
    pub errors: Vec<String>,
}

impl BackupManifest {
    /// 序列化为格式化的 JSON 字符串（缩进 2 空格）
    pub fn serialize(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化 manifest 失败: {}", e))
    }

    /// 从 JSON 字符串反序列化，缺少必需字段时返回描述性错误
    pub fn deserialize(json: &str) -> Result<Self, String> {
        serde_json::from_str::<Self>(json).map_err(|e| {
            // Try to parse as generic Value to check which fields are missing
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                let mut missing = Vec::new();
                if value.get("version").is_none() {
                    missing.push("version");
                }
                if value.get("timestamp").is_none() {
                    missing.push("timestamp");
                }
                if value.get("services").is_none() {
                    missing.push("services");
                }
                if !missing.is_empty() {
                    return format!("缺少必需字段: {}", missing.join(", "));
                }
            }
            format!("反序列化 manifest 失败: {}", e)
        })
    }

    /// 创建一个新的空 manifest
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os_info: std::env::consts::OS.to_string(),
            services: Vec::new(),
            options: BackupOptions {
                include_projects: false,
                project_patterns: Vec::new(),
                include_logs: false,
            },
            files: HashMap::new(),
            errors: Vec::new(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> BackupManifest {
        let mut ports = HashMap::new();
        ports.insert(3306, 3306);
        ports.insert(8080, 80);

        let mut files = HashMap::new();
        files.insert(
            ".env".to_string(),
            "abc123def456".to_string(),
        );
        files.insert(
            "docker-compose.yml".to_string(),
            "789xyz000111".to_string(),
        );

        BackupManifest {
            version: "1.0.0".to_string(),
            timestamp: "2025-01-15T10:30:00+08:00".to_string(),
            app_version: "0.1.0".to_string(),
            os_info: "linux".to_string(),
            services: vec![
                ManifestService {
                    name: "mysql".to_string(),
                    image: "mysql:8.0".to_string(),
                    version: "8.0".to_string(),
                    ports: ports.clone(),
                },
                ManifestService {
                    name: "redis".to_string(),
                    image: "redis:7".to_string(),
                    version: "7".to_string(),
                    ports: HashMap::new(),
                },
            ],
            options: BackupOptions {
                include_projects: true,
                project_patterns: vec!["*.php".to_string(), "*.html".to_string()],
                include_logs: false,
            },
            files,
            errors: vec!["mysqldump for db2 failed".to_string()],
        }
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let manifest = sample_manifest();
        let json = manifest.serialize().expect("serialize should succeed");
        let deserialized =
            BackupManifest::deserialize(&json).expect("deserialize should succeed");
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn test_deserialize_missing_version() {
        let json = r#"{
            "timestamp": "2025-01-15T10:30:00+08:00",
            "app_version": "0.1.0",
            "os_info": "linux",
            "services": [],
            "options": {
                "include_projects": false,
                "project_patterns": [],
                "include_logs": false
            },
            "files": {},
            "errors": []
        }"#;
        let result = BackupManifest::deserialize(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("version"),
            "Error should mention 'version', got: {}",
            err
        );
    }

    #[test]
    fn test_deserialize_missing_timestamp() {
        let json = r#"{
            "version": "1.0.0",
            "app_version": "0.1.0",
            "os_info": "linux",
            "services": [],
            "options": {
                "include_projects": false,
                "project_patterns": [],
                "include_logs": false
            },
            "files": {},
            "errors": []
        }"#;
        let result = BackupManifest::deserialize(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("timestamp"),
            "Error should mention 'timestamp', got: {}",
            err
        );
    }

    #[test]
    fn test_deserialize_missing_services() {
        let json = r#"{
            "version": "1.0.0",
            "timestamp": "2025-01-15T10:30:00+08:00",
            "app_version": "0.1.0",
            "os_info": "linux",
            "options": {
                "include_projects": false,
                "project_patterns": [],
                "include_logs": false
            },
            "files": {},
            "errors": []
        }"#;
        let result = BackupManifest::deserialize(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("services"),
            "Error should mention 'services', got: {}",
            err
        );
    }

    #[test]
    fn test_new_manifest() {
        let manifest = BackupManifest::new();
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(manifest.os_info, std::env::consts::OS);
        assert!(manifest.services.is_empty());
        assert!(!manifest.options.include_projects);
        assert!(manifest.options.project_patterns.is_empty());
        assert!(!manifest.options.include_logs);
        assert!(manifest.files.is_empty());
        assert!(manifest.errors.is_empty());
        // timestamp should be a non-empty string
        assert!(!manifest.timestamp.is_empty());
    }

    #[test]
    fn test_serialize_pretty_format() {
        let manifest = BackupManifest::new();
        let json = manifest.serialize().expect("serialize should succeed");
        // Pretty-printed JSON should contain newlines and indentation
        assert!(json.contains('\n'), "Output should contain newlines");
        assert!(
            json.contains("  "),
            "Output should contain 2-space indentation"
        );
        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("Output should be valid JSON");
        assert!(parsed.is_object());
        assert!(parsed.get("version").is_some());
        assert!(parsed.get("timestamp").is_some());
        assert!(parsed.get("services").is_some());
    }
}
