use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::version_manifest::{
    extract_version_numbers, normalize_service_kind, VersionEntry, VersionManifest,
};
use crate::app_log;

/// 用户版本覆盖 / 自定义条目（`entry_kind` 必填；不做旧格式兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "entry_kind", rename_all = "snake_case")]
pub enum UserVersionOverride {
    /// 覆盖已有清单 ID 的镜像标签
    Override {
        image_tag: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// 完整自定义版本条目（ID 不得与当前清单冲突）
    Custom {
        display_name: String,
        image_tag: String,
        service_dir: String,
        default_port: u16,
        show_port: bool,
        #[serde(default)]
        eol: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

impl UserVersionOverride {
    pub fn image_tag(&self) -> &str {
        match self {
            Self::Override { image_tag, .. } | Self::Custom { image_tag, .. } => image_tag,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Override { description, .. } | Self::Custom { description, .. } => {
                description.as_deref()
            }
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom { .. })
    }
}

/// 合并后的列表项：id + entry + 是否有用户覆盖/自定义
pub struct MergedVersionItem {
    pub id: String,
    pub entry: VersionEntry,
    pub has_user_override: bool,
    pub is_custom: bool,
}

/// 用户版本覆盖管理器
pub struct UserOverrideManager {
    /// 默认版本清单
    default_manifest: VersionManifest,
    /// 用户覆盖配置（key = 服务 kind，如 php / mysql / mongodb）
    user_overrides: HashMap<String, HashMap<String, UserVersionOverride>>,
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

    /// 从文件加载用户覆盖配置；缺 `entry_kind` 或结构不符视为空
    fn load_user_overrides(
        project_root: &Path,
    ) -> HashMap<String, HashMap<String, UserVersionOverride>> {
        let overrides_path =
            super::user_config::path(project_root, super::user_config::VERSION_OVERRIDES);

        if !overrides_path.exists() {
            app_log!(
                info,
                "engine::user_override",
                "No user override file, using defaults"
            );
            return HashMap::new();
        }

        app_log!(
            info,
            "engine::user_override",
            "Loading user overrides: {:?}",
            overrides_path
        );

        match std::fs::read_to_string(&overrides_path) {
            Ok(content) => {
                match serde_json::from_str::<HashMap<String, HashMap<String, UserVersionOverride>>>(
                    &content,
                ) {
                    Ok(raw) => {
                        let mut result = HashMap::new();
                        let mut override_count = 0;

                        for (service_key, versions) in raw {
                            let kind = normalize_service_kind(&service_key);
                            if kind.is_empty() {
                                continue;
                            }
                            override_count += versions.len();
                            app_log!(
                                info,
                                "engine::user_override",
                                "{kind}: {} version override(s)",
                                versions.len()
                            );
                            result.insert(kind, versions);
                        }

                        app_log!(
                            info,
                            "engine::user_override",
                            "Loaded {} service type(s), {override_count} override(s)",
                            result.len()
                        );
                        result
                    }
                    Err(e) => {
                        app_log!(
                            warn,
                            "engine::user_override",
                            "failed to parse override file (ignored): {e}"
                        );
                        HashMap::new()
                    }
                }
            }
            Err(e) => {
                app_log!(
                    error,
                    "engine::user_override",
                    "failed to read override file: {e}"
                );
                HashMap::new()
            }
        }
    }

    fn persist(&self, project_root: &Path) -> Result<(), String> {
        super::user_config::ensure_dir(project_root)?;
        let overrides_path =
            super::user_config::path(project_root, super::user_config::VERSION_OVERRIDES);

        let mut raw: HashMap<String, HashMap<String, UserVersionOverride>> = HashMap::new();
        for (service_kind, versions) in &self.user_overrides {
            if versions.is_empty() {
                continue;
            }
            raw.insert(service_kind.clone(), versions.clone());
        }

        let json =
            serde_json::to_string_pretty(&raw).map_err(|e| format!("serialize failed: {e}"))?;
        std::fs::write(&overrides_path, json).map_err(|e| format!("failed to write file: {e}"))?;
        Ok(())
    }

    /// 获取合并后的版本条目
    ///
    /// - `override`：叠在 manifest 基线上，只换 image_tag / description
    /// - `custom` 且清单无该 ID：返回完整自定义条目
    /// - `custom` 且清单已有该 ID（日后 sync 收录）：按 override 语义合并
    pub fn get_merged_entry(&self, service_kind: &str, id: &str) -> Option<VersionEntry> {
        let kind = normalize_service_kind(service_kind);
        let stored = self
            .user_overrides
            .get(&kind)
            .and_then(|entries| entries.get(id));

        if let Some(user_override) = stored {
            match user_override {
                UserVersionOverride::Override {
                    image_tag,
                    description,
                } => {
                    if let Some(default_info) = self.default_manifest.get_entry(&kind, id) {
                        app_log!(
                            info,
                            "engine::user_override",
                            "{kind} {id} using override tag: {image_tag}"
                        );
                        return Some(VersionEntry {
                            display_name: default_info.display_name.clone(),
                            image_tag: image_tag.clone(),
                            service_dir: default_info.service_dir.clone(),
                            default_port: default_info.default_port,
                            show_port: default_info.show_port,
                            eol: default_info.eol,
                            description: description
                                .clone()
                                .or_else(|| default_info.description.clone()),
                        });
                    }
                    // orphan override：无清单基线，忽略
                    app_log!(
                        warn,
                        "engine::user_override",
                        "orphan override ignored (no manifest id): {id}"
                    );
                }
                UserVersionOverride::Custom {
                    display_name,
                    image_tag,
                    service_dir,
                    default_port,
                    show_port,
                    eol,
                    description,
                } => {
                    if let Some(default_info) = self.default_manifest.get_entry(&kind, id) {
                        // sync 日后收录同 ID：降级为 override 语义
                        app_log!(
                            info,
                            "engine::user_override",
                            "{kind} {id} custom demoted to override (now in manifest)"
                        );
                        return Some(VersionEntry {
                            display_name: default_info.display_name.clone(),
                            image_tag: image_tag.clone(),
                            service_dir: default_info.service_dir.clone(),
                            default_port: default_info.default_port,
                            show_port: default_info.show_port,
                            eol: default_info.eol,
                            description: description
                                .clone()
                                .or_else(|| default_info.description.clone()),
                        });
                    }
                    return Some(VersionEntry {
                        display_name: display_name.clone(),
                        image_tag: image_tag.clone(),
                        service_dir: service_dir.clone(),
                        default_port: *default_port,
                        show_port: *show_port,
                        eol: *eol,
                        description: description.clone(),
                    });
                }
            }
        }

        self.default_manifest.get_entry(&kind, id).cloned()
    }

    /// 合并后的完整列表（清单 + 自定义），按版本号降序
    pub fn list_merged_entries(&self, service_kind: &str) -> Vec<MergedVersionItem> {
        let kind = normalize_service_kind(service_kind);
        let mut items: Vec<MergedVersionItem> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for (id, _) in self.default_manifest.get_available_entries(&kind) {
            if let Some(entry) = self.get_merged_entry(&kind, id) {
                let has_user_override = self.has_user_override(&kind, id);
                // 清单内 ID 即便磁盘是 custom，合并后也按「覆盖」展示（is_custom=false）
                let is_custom = false;
                items.push(MergedVersionItem {
                    id: id.clone(),
                    entry,
                    has_user_override,
                    is_custom,
                });
                seen_ids.insert(id.clone());
            }
        }

        if let Some(overrides) = self.user_overrides.get(&kind) {
            for (id, stored) in overrides {
                if seen_ids.contains(id) {
                    continue;
                }
                if !stored.is_custom() {
                    continue;
                }
                if let Some(entry) = self.get_merged_entry(&kind, id) {
                    items.push(MergedVersionItem {
                        id: id.clone(),
                        entry,
                        has_user_override: true,
                        is_custom: true,
                    });
                }
            }
        }

        items.sort_by_key(|a| std::cmp::Reverse(extract_version_numbers(&a.id)));
        items
    }

    /// ID 是否有效（清单或 custom 条目）
    pub fn is_id_valid(&self, service_kind: &str, id: &str) -> bool {
        self.get_merged_entry(service_kind, id).is_some()
    }

    /// 推荐的默认 service_dir（同服务清单中版本号最高的目录）
    pub fn suggested_service_dir(&self, service_kind: &str) -> Option<String> {
        self.default_manifest
            .get_available_entries(service_kind)
            .into_iter()
            .next()
            .map(|(_, e)| e.service_dir.clone())
    }

    /// 保存对已有清单 ID 的镜像覆盖；若 ID 已是 custom 条目则只更新其 image_tag/description
    pub fn save_user_override(
        &mut self,
        project_root: &Path,
        service_kind: &str,
        id: String,
        image_tag: String,
        description: Option<String>,
    ) -> Result<(), String> {
        let kind = normalize_service_kind(service_kind);
        if image_tag.trim().is_empty() {
            return Err("image_tag must not be empty".to_string());
        }

        if let Some(UserVersionOverride::Custom {
            image_tag: tag,
            description: desc,
            ..
        }) = self
            .user_overrides
            .get_mut(&kind)
            .and_then(|m| m.get_mut(&id))
        {
            *tag = image_tag;
            *desc = description;
            return self.persist(project_root);
        }

        if self.default_manifest.get_entry(&kind, &id).is_none() {
            return Err(format!(
                "version id '{id}' not in manifest; use add_custom_version instead"
            ));
        }

        self.user_overrides.entry(kind).or_default().insert(
            id,
            UserVersionOverride::Override {
                image_tag,
                description,
            },
        );
        self.persist(project_root)
    }

    /// 新增完整自定义版本条目
    pub fn add_custom_version(
        &mut self,
        project_root: &Path,
        service_kind: &str,
        id: String,
        entry: VersionEntry,
    ) -> Result<(), String> {
        let kind = normalize_service_kind(service_kind);
        let id = id.trim().to_string();
        if id.is_empty() {
            return Err("id must not be empty".to_string());
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "id must be alphanumeric (optional _/-), e.g. redis84 or php86".to_string(),
            );
        }
        if self.default_manifest.get_entry(&kind, &id).is_some() {
            return Err(format!(
                "id '{id}' already exists in manifest; edit it instead of adding"
            ));
        }
        if self.has_user_override(&kind, &id) {
            return Err(format!("id '{id}' already exists as a custom entry"));
        }
        if entry.image_tag.trim().is_empty() {
            return Err("image_tag must not be empty".to_string());
        }
        if entry.display_name.trim().is_empty() {
            return Err("display_name must not be empty".to_string());
        }
        if entry.service_dir.trim().is_empty() {
            return Err("service_dir must not be empty".to_string());
        }
        if entry.default_port == 0 {
            return Err("default_port must be > 0".to_string());
        }

        self.user_overrides.entry(kind).or_default().insert(
            id,
            UserVersionOverride::Custom {
                display_name: entry.display_name,
                image_tag: entry.image_tag,
                service_dir: entry.service_dir,
                default_port: entry.default_port,
                show_port: entry.show_port,
                eol: entry.eol,
                description: entry.description,
            },
        );
        self.persist(project_root)
    }

    /// 更新已有 custom 条目的 image_tag / default_port（可选 display_name）
    pub fn upsert_custom_version_fields(
        &mut self,
        project_root: &Path,
        service_kind: &str,
        id: &str,
        image_tag: &str,
        default_port: u16,
        display_name: Option<&str>,
    ) -> Result<(), String> {
        let kind = normalize_service_kind(service_kind);
        if image_tag.trim().is_empty() {
            return Err("image_tag must not be empty".to_string());
        }
        if default_port == 0 {
            return Err("default_port must be > 0".to_string());
        }
        let Some(entries) = self.user_overrides.get_mut(&kind) else {
            return Err(format!("no overrides for kind '{kind}'"));
        };
        match entries.get_mut(id) {
            Some(UserVersionOverride::Custom {
                display_name: name,
                image_tag: tag,
                default_port: port,
                ..
            }) => {
                *tag = image_tag.to_string();
                *port = default_port;
                if let Some(dn) = display_name.map(str::trim).filter(|s| !s.is_empty()) {
                    *name = dn.to_string();
                }
                self.persist(project_root)
            }
            Some(UserVersionOverride::Override { .. }) => Err(format!(
                "id '{id}' is an override entry, not custom"
            )),
            None => Err(format!("custom entry '{id}' not found for {kind}")),
        }
    }

    /// 是否存在 entry_kind=custom 的条目（不含清单 / override）
    pub fn is_custom_entry(&self, service_kind: &str, id: &str) -> bool {
        let kind = normalize_service_kind(service_kind);
        self.user_overrides
            .get(&kind)
            .and_then(|entries| entries.get(id))
            .map(|o| o.is_custom())
            .unwrap_or(false)
    }

    /// 删除某 kind 下全部覆盖/自定义并持久化（用于移除自定义服务种类）
    pub fn remove_kind(&mut self, project_root: &Path, service_kind: &str) -> Result<(), String> {
        let kind = normalize_service_kind(service_kind);
        self.user_overrides.remove(&kind);
        self.persist(project_root)
    }

    /// 删除用户覆盖或自定义条目
    pub fn remove_user_override(
        &mut self,
        project_root: &Path,
        service_kind: &str,
        id: &str,
    ) -> Result<(), String> {
        let kind = normalize_service_kind(service_kind);
        if let Some(entries) = self.user_overrides.get_mut(&kind) {
            entries.remove(id);
        }
        self.persist(project_root)
    }

    /// 重置所有用户覆盖
    pub fn reset_all_overrides(&mut self, project_root: &Path) -> Result<(), String> {
        self.user_overrides.clear();

        let overrides_path =
            super::user_config::path(project_root, super::user_config::VERSION_OVERRIDES);
        if overrides_path.exists() {
            std::fs::remove_file(&overrides_path)
                .map_err(|e| format!("failed to delete file: {e}"))?;
        }

        Ok(())
    }

    /// 检查指定 ID 是否有用户覆盖/自定义
    pub fn has_user_override(&self, service_kind: &str, id: &str) -> bool {
        let kind = normalize_service_kind(service_kind);
        self.user_overrides
            .get(&kind)
            .and_then(|entries| entries.get(id))
            .is_some()
    }

    /// 列出覆盖文件中出现过的服务 kind（含仅有 custom、清单中不存在的）
    pub fn service_kinds(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.user_overrides.keys().cloned().collect();
        keys.sort();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_overrides(root: &Path, json: &str) {
        let dir = root.join(".user-config");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("version_overrides.json"), json).unwrap();
    }

    #[test]
    fn test_load_nonexistent_overrides() {
        let temp = TempDir::new().unwrap();
        let manager = UserOverrideManager::new(temp.path());
        assert!(manager.user_overrides.is_empty());
    }

    #[test]
    fn test_get_default_when_no_override() {
        let temp = TempDir::new().unwrap();
        let manager = UserOverrideManager::new(temp.path());

        let info = manager.get_merged_entry("mysql", "mysql80");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.image_tag, "mysql:8.0");
    }

    #[test]
    fn test_legacy_json_without_entry_kind_is_ignored() {
        let temp = TempDir::new().unwrap();
        // 旧格式：无 entry_kind
        write_overrides(
            temp.path(),
            r#"{ "php": { "php82": { "image_tag": "custom/php:8.2" } } }"#,
        );
        let manager = UserOverrideManager::new(temp.path());
        assert!(manager.user_overrides.is_empty());
        assert!(!manager.has_user_override("php", "php82"));
    }

    #[test]
    fn test_override_merge() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        manager
            .save_user_override(
                temp.path(),
                "php",
                "php82".to_string(),
                "custom/php:8.2".to_string(),
                Some("note".to_string()),
            )
            .unwrap();

        let merged = manager.get_merged_entry("php", "php82").unwrap();
        assert_eq!(merged.image_tag, "custom/php:8.2");
        assert_eq!(merged.description.as_deref(), Some("note"));
        assert_eq!(merged.service_dir, "php82");
    }

    #[test]
    fn test_custom_orphan_appears_in_list() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        manager
            .add_custom_version(
                temp.path(),
                "redis",
                "redis99".to_string(),
                VersionEntry {
                    display_name: "Redis 9.9".to_string(),
                    image_tag: "redis:9.9-alpine".to_string(),
                    service_dir: "redis82".to_string(),
                    default_port: 6379,
                    show_port: true,
                    eol: false,
                    description: Some("custom".to_string()),
                },
            )
            .unwrap();

        let list = manager.list_merged_entries("redis");
        let custom = list.iter().find(|i| i.id == "redis99").unwrap();
        assert!(custom.is_custom);
        assert!(custom.has_user_override);
        assert_eq!(custom.entry.image_tag, "redis:9.9-alpine");
        assert!(manager.is_id_valid("redis", "redis99"));
    }

    #[test]
    fn test_custom_id_conflict_with_manifest_rejected() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        let err = manager
            .add_custom_version(
                temp.path(),
                "redis",
                "redis82".to_string(),
                VersionEntry {
                    display_name: "Redis 8.2".to_string(),
                    image_tag: "redis:8.2-alpine".to_string(),
                    service_dir: "redis82".to_string(),
                    default_port: 6379,
                    show_port: true,
                    eol: false,
                    description: None,
                },
            )
            .unwrap_err();
        assert!(err.contains("already exists in manifest"));
    }

    #[test]
    fn test_custom_demoted_when_manifest_has_id() {
        let temp = TempDir::new().unwrap();
        // 直接写入与清单 ID 同名的 custom（模拟日后 sync 收录）
        write_overrides(
            temp.path(),
            r#"{
              "redis": {
                "redis82": {
                  "entry_kind": "custom",
                  "display_name": "My Redis",
                  "image_tag": "my/redis:custom",
                  "service_dir": "should-not-use",
                  "default_port": 1,
                  "show_port": false,
                  "eol": true,
                  "description": "kept"
                }
              }
            }"#,
        );
        let manager = UserOverrideManager::new(temp.path());
        let merged = manager.get_merged_entry("redis", "redis82").unwrap();
        assert_eq!(merged.image_tag, "my/redis:custom");
        assert_eq!(merged.description.as_deref(), Some("kept"));
        // 目录/端口跟官方
        assert_eq!(merged.service_dir, "redis82");
        assert_ne!(merged.default_port, 1);

        let list = manager.list_merged_entries("redis");
        let item = list.iter().find(|i| i.id == "redis82").unwrap();
        assert!(!item.is_custom);
        assert!(item.has_user_override);
    }

    #[test]
    fn test_upsert_custom_version_fields() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        manager
            .add_custom_version(
                temp.path(),
                "mongodb",
                "mongodefault".to_string(),
                VersionEntry {
                    display_name: "MongoDB default".to_string(),
                    image_tag: "mongo:6".to_string(),
                    service_dir: "mongodefault".to_string(),
                    default_port: 27017,
                    show_port: true,
                    eol: false,
                    description: None,
                },
            )
            .unwrap();

        manager
            .upsert_custom_version_fields(
                temp.path(),
                "mongodb",
                "mongodefault",
                "mongo:7",
                27018,
                Some("MongoDB 7"),
            )
            .unwrap();

        let entry = manager.get_merged_entry("mongodb", "mongodefault").unwrap();
        assert_eq!(entry.image_tag, "mongo:7");
        assert_eq!(entry.default_port, 27018);
        assert_eq!(entry.display_name, "MongoDB 7");
        assert!(manager.is_custom_entry("mongodb", "mongodefault"));
    }

    #[test]
    fn test_remove_kind() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        manager
            .add_custom_version(
                temp.path(),
                "mongodb",
                "mongodefault".to_string(),
                VersionEntry {
                    display_name: "MongoDB".to_string(),
                    image_tag: "mongo:6".to_string(),
                    service_dir: "mongodefault".to_string(),
                    default_port: 27017,
                    show_port: true,
                    eol: false,
                    description: None,
                },
            )
            .unwrap();
        assert!(manager.service_kinds().contains(&"mongodb".to_string()));

        manager.remove_kind(temp.path(), "mongodb").unwrap();
        assert!(!manager.service_kinds().contains(&"mongodb".to_string()));
        assert!(manager.get_merged_entry("mongodb", "mongodefault").is_none());
    }

    #[test]
    fn test_add_custom_version_rejects_zero_port() {
        let temp = TempDir::new().unwrap();
        let mut manager = UserOverrideManager::new(temp.path());
        let err = manager
            .add_custom_version(
                temp.path(),
                "mongodb",
                "mongodefault".to_string(),
                VersionEntry {
                    display_name: "MongoDB".to_string(),
                    image_tag: "mongo:6".to_string(),
                    service_dir: "mongodefault".to_string(),
                    default_port: 0,
                    show_port: true,
                    eol: false,
                    description: None,
                },
            )
            .unwrap_err();
        assert!(err.contains("default_port must be > 0"));
    }
}
