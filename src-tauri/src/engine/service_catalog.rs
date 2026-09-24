//! 服务目录（Catalog）：内置描述符 + 用户自定义服务合并。
//!
//! 生成器按 `generator` 字段路由（`php` / `nginx` / `image`），不再靠硬编码枚举扩展。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::user_config;
use crate::app_log;

/// 将历史 PascalCase / 大小写混用规范为小写 kind id。
pub fn normalize_service_kind(raw: &str) -> String {
    match raw {
        "PHP" | "Php" => "php".to_string(),
        "MySQL" | "Mysql" | "MYSQL" => "mysql".to_string(),
        "Redis" | "REDIS" => "redis".to_string(),
        "Nginx" | "NGINX" => "nginx".to_string(),
        other => other.to_lowercase(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GeneratorKind {
    Php,
    Nginx,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectInfo {
    pub container_port: u16,
    #[serde(default)]
    pub short_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    pub env_suffix: String,
    pub container_path: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VolumeSpec {
    #[serde(default)]
    pub conf: Option<VolumeMount>,
    #[serde(default)]
    pub data: Option<VolumeMount>,
    #[serde(default)]
    pub log: Option<VolumeMount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraEnvSpec {
    pub name: String,
    /// 目前仅支持从 EnvConfig.mysql_root_password 取值
    pub from: String,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractSpec {
    pub dest_name: String,
    pub default_src: String,
    #[serde(default)]
    pub paths_by_major: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDescriptor {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub ui_order: i32,
    #[serde(default)]
    pub required_min: u32,
    #[serde(default)]
    pub default_on_new_workspace: bool,
    #[serde(default = "default_true")]
    pub multi_instance: bool,
    pub generator: GeneratorKind,
    #[serde(default)]
    pub builtin: bool,
    pub container_port: u16,
    #[serde(default = "default_host_port_suffix")]
    pub host_port_env_suffix: String,
    #[serde(default)]
    pub short_alias: Option<String>,
    #[serde(default)]
    pub conf_key_file: Option<String>,
    #[serde(default)]
    pub fallback_template_dir: Option<String>,
    #[serde(default)]
    pub volumes: VolumeSpec,
    #[serde(default)]
    pub extra_env: Vec<ExtraEnvSpec>,
    #[serde(default)]
    pub compose_environment: Vec<String>,
    #[serde(default)]
    pub entrypoint: Option<Vec<String>>,
    #[serde(default)]
    pub extract: Option<ExtractSpec>,
    pub connect: ConnectInfo,
}

fn default_true() -> bool {
    true
}

fn default_host_port_suffix() -> String {
    "HOST_PORT".to_string()
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    services: Vec<ServiceDescriptor>,
}

/// 用户自定义服务文件（`.user-config/custom_services.json`）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomServicesFile {
    #[serde(default)]
    pub services: Vec<ServiceDescriptor>,
}

pub struct ServiceCatalog {
    by_id: HashMap<String, ServiceDescriptor>,
    ordered: Vec<ServiceDescriptor>,
}

impl ServiceCatalog {
    /// 仅内置目录（无工作区自定义）
    pub fn builtin() -> Self {
        let json = include_str!("../../services/service_catalog.json");
        Self::from_json(json).unwrap_or_else(|e| {
            panic!("Failed to parse embedded service_catalog.json: {e}");
        })
    }

    /// 内置 + 工作区自定义合并
    pub fn merged(project_root: &Path) -> Self {
        let mut catalog = Self::builtin();
        match load_custom_services(project_root) {
            Ok(custom) => {
                for mut svc in custom.services {
                    svc.builtin = false;
                    svc.generator = GeneratorKind::Image;
                    if let Err(e) = catalog.insert_custom(svc) {
                        app_log!(
                            warn,
                            "engine::service_catalog",
                            "skip custom service: {e}"
                        );
                    }
                }
            }
            Err(e) => {
                app_log!(
                    warn,
                    "engine::service_catalog",
                    "failed to load custom services: {e}"
                );
            }
        }
        catalog
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let file: CatalogFile = serde_json::from_str(json)
            .map_err(|e| format!("failed to parse service_catalog: {e}"))?;
        let mut by_id = HashMap::new();
        let mut ordered = Vec::new();
        for mut svc in file.services {
            svc.id = normalize_service_kind(&svc.id);
            if svc.id.is_empty() {
                return Err("service id cannot be empty".to_string());
            }
            if by_id.contains_key(&svc.id) {
                return Err(format!("duplicate service id in catalog: {}", svc.id));
            }
            by_id.insert(svc.id.clone(), svc.clone());
            ordered.push(svc);
        }
        ordered.sort_by_key(|s| s.ui_order);
        Ok(Self { by_id, ordered })
    }

    pub fn get(&self, id: &str) -> Option<&ServiceDescriptor> {
        let kind = normalize_service_kind(id);
        self.by_id.get(&kind)
    }

    pub fn list(&self) -> &[ServiceDescriptor] {
        &self.ordered
    }

    pub fn kinds(&self) -> Vec<String> {
        self.ordered.iter().map(|s| s.id.clone()).collect()
    }

    /// 插入用户自定义服务（禁止覆盖内置 id）
    pub fn insert_custom(&mut self, mut svc: ServiceDescriptor) -> Result<(), String> {
        svc.id = normalize_service_kind(&svc.id);
        validate_custom_id(&svc.id)?;
        if let Some(existing) = self.by_id.get(&svc.id) {
            if existing.builtin {
                return Err(format!("service id '{}' conflicts with builtin", svc.id));
            }
            // 更新已有自定义
            self.by_id.insert(svc.id.clone(), svc.clone());
            if let Some(slot) = self.ordered.iter_mut().find(|s| s.id == svc.id) {
                *slot = svc;
            }
            self.ordered.sort_by_key(|s| s.ui_order);
            return Ok(());
        }
        svc.builtin = false;
        svc.generator = GeneratorKind::Image;
        self.by_id.insert(svc.id.clone(), svc.clone());
        self.ordered.push(svc);
        self.ordered.sort_by_key(|s| s.ui_order);
        Ok(())
    }

    pub fn remove_custom(&mut self, id: &str) -> Result<(), String> {
        let kind = normalize_service_kind(id);
        match self.by_id.get(&kind) {
            Some(s) if s.builtin => Err(format!("cannot remove builtin service '{kind}'")),
            Some(_) => {
                self.by_id.remove(&kind);
                self.ordered.retain(|s| s.id != kind);
                Ok(())
            }
            None => Err(format!("custom service '{kind}' not found")),
        }
    }
}

pub fn validate_custom_id(id: &str) -> Result<(), String> {
    let re_ok = id.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_lowercase()
        } else {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
        }
    });
    if id.is_empty() || !re_ok {
        return Err(
            "service id must match ^[a-z][a-z0-9_-]*$ (lowercase letter, then alnum/_/-)"
                .to_string(),
        );
    }
    Ok(())
}

pub fn load_custom_services(project_root: &Path) -> Result<CustomServicesFile, String> {
    let path = user_config::path(project_root, user_config::CUSTOM_SERVICES);
    if !path.exists() {
        return Ok(CustomServicesFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read custom_services.json: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("failed to parse custom_services.json: {e}"))
}

pub fn save_custom_services(
    project_root: &Path,
    file: &CustomServicesFile,
) -> Result<(), String> {
    user_config::ensure_dir(project_root)?;
    let path = user_config::path(project_root, user_config::CUSTOM_SERVICES);
    let content = serde_json::to_string_pretty(file)
        .map_err(|e| format!("failed to serialize custom_services: {e}"))?;
    std::fs::write(&path, content).map_err(|e| format!("failed to write custom_services.json: {e}"))
}

/// 将合并后的自定义服务写回（仅非 builtin）
pub fn persist_custom_from_catalog(
    project_root: &Path,
    catalog: &ServiceCatalog,
) -> Result<(), String> {
    let file = CustomServicesFile {
        services: catalog
            .list()
            .iter()
            .filter(|s| !s.builtin)
            .cloned()
            .collect(),
    };
    save_custom_services(project_root, &file)
}

/// 简化表单：用于 UI 新建/更新自定义服务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomServiceForm {
    pub id: String,
    pub display_name: String,
    pub container_port: u16,
    /// 默认版本的宿主机端口（写入 version 映射）
    pub host_port: u16,
    pub image_tag: String,
    /// 版本 ID；缺省为 `{id}default`
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub short_alias: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<Vec<String>>,
    /// 数据卷容器路径，如 `/data/db`
    #[serde(default)]
    pub data_container_path: Option<String>,
}

/// 从简化表单构建完整 `ServiceDescriptor`（generator=image，builtin=false）
pub fn build_custom_descriptor(form: &CustomServiceForm) -> Result<ServiceDescriptor, String> {
    let id = normalize_service_kind(&form.id);
    validate_custom_id(&id)?;
    if form.display_name.trim().is_empty() {
        return Err("display_name must not be empty".to_string());
    }
    if form.container_port == 0 {
        return Err("container_port must be > 0".to_string());
    }
    if form.host_port == 0 {
        return Err("host_port must be > 0".to_string());
    }
    if form.image_tag.trim().is_empty() {
        return Err("image_tag must not be empty".to_string());
    }
    // 版本 ID 与服务 id 同一字符集（缺省 `{id}default` 已满足）
    let version_id = default_custom_version_id(&id, form.version_id.as_deref());
    validate_custom_id(&version_id)?;

    let short = form
        .short_alias
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let volumes = VolumeSpec {
        // 自定义服务不挂 conf 卷（builtin mysql/redis 仍用 VolumeSpec.conf）
        conf: None,
        data: form.data_container_path.as_ref().and_then(|p| {
            let path = p.trim();
            if path.is_empty() {
                None
            } else {
                Some(VolumeMount {
                    env_suffix: "DATA_DIR".to_string(),
                    container_path: path.to_string(),
                    read_only: false,
                })
            }
        }),
        log: None,
    };

    Ok(ServiceDescriptor {
        id,
        display_name: form.display_name.trim().to_string(),
        ui_order: 100,
        required_min: 0,
        default_on_new_workspace: false,
        multi_instance: true,
        generator: GeneratorKind::Image,
        builtin: false,
        container_port: form.container_port,
        host_port_env_suffix: "HOST_PORT".to_string(),
        short_alias: short.clone(),
        conf_key_file: None,
        fallback_template_dir: None,
        volumes,
        extra_env: vec![],
        compose_environment: vec!["TZ".to_string()],
        entrypoint: form.entrypoint.clone(),
        extract: None,
        connect: ConnectInfo {
            container_port: form.container_port,
            short_name: short,
        },
    })
}

/// 默认版本 ID：`{service_id}default`，或用户指定（如 `mongo7`）
pub fn default_custom_version_id(service_id: &str, version_id: Option<&str>) -> String {
    match version_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(v) => v.to_string(),
        None => format!("{service_id}default"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_legacy_pascal_case() {
        assert_eq!(normalize_service_kind("PHP"), "php");
        assert_eq!(normalize_service_kind("MySQL"), "mysql");
        assert_eq!(normalize_service_kind("mongodb"), "mongodb");
    }

    #[test]
    fn builtin_catalog_has_four_services() {
        let cat = ServiceCatalog::builtin();
        assert_eq!(cat.list().len(), 4);
        assert!(cat.get("php").is_some());
        assert!(cat.get("MySQL").unwrap().generator == GeneratorKind::Image);
        assert!(!cat.get("mysql").unwrap().default_on_new_workspace);
        assert_eq!(cat.get("php").unwrap().required_min, 1);
    }

    #[test]
    fn custom_id_validation() {
        assert!(validate_custom_id("mongodb").is_ok());
        assert!(validate_custom_id("memcached").is_ok());
        assert!(validate_custom_id("mongo-db").is_ok());
        assert!(validate_custom_id("Mongo").is_err());
        assert!(validate_custom_id("1mongo").is_err());
        assert!(validate_custom_id("php").is_ok()); // format ok; conflict checked separately
    }

    #[test]
    fn insert_custom_rejects_builtin_id() {
        let mut cat = ServiceCatalog::builtin();
        let mut svc = cat.get("redis").unwrap().clone();
        svc.id = "mysql".to_string();
        svc.builtin = false;
        assert!(cat.insert_custom(svc).is_err());
    }

    #[test]
    fn insert_and_remove_custom() {
        let mut cat = ServiceCatalog::builtin();
        let svc = ServiceDescriptor {
            id: "mongodb".to_string(),
            display_name: "MongoDB".to_string(),
            ui_order: 50,
            required_min: 0,
            default_on_new_workspace: false,
            multi_instance: true,
            generator: GeneratorKind::Image,
            builtin: false,
            container_port: 27017,
            host_port_env_suffix: "HOST_PORT".to_string(),
            short_alias: Some("mongo".to_string()),
            conf_key_file: None,
            fallback_template_dir: None,
            volumes: VolumeSpec {
                data: Some(VolumeMount {
                    env_suffix: "DATA_DIR".to_string(),
                    container_path: "/data/db".to_string(),
                    read_only: false,
                }),
                ..Default::default()
            },
            extra_env: vec![],
            compose_environment: vec!["TZ".to_string()],
            entrypoint: None,
            extract: None,
            connect: ConnectInfo {
                container_port: 27017,
                short_name: Some("mongo".to_string()),
            },
        };
        cat.insert_custom(svc).expect("insert ok");
        assert!(cat.get("mongodb").is_some());
        cat.remove_custom("mongodb").expect("remove ok");
        assert!(cat.get("mongodb").is_none());
    }

    #[test]
    fn build_custom_descriptor_sets_connect_and_volumes() {
        let form = CustomServiceForm {
            id: "MongoDB".to_string(),
            display_name: "MongoDB".to_string(),
            container_port: 27017,
            host_port: 27017,
            image_tag: "mongo:7".to_string(),
            version_id: Some("mongo7".to_string()),
            short_alias: Some("mongo".to_string()),
            entrypoint: None,
            data_container_path: Some("/data/db".to_string()),
        };
        let d = build_custom_descriptor(&form).expect("ok");
        assert_eq!(d.id, "mongodb");
        assert!(!d.builtin);
        assert_eq!(d.generator, GeneratorKind::Image);
        assert_eq!(d.connect.container_port, 27017);
        assert_eq!(d.connect.short_name.as_deref(), Some("mongo"));
        assert!(d.volumes.data.is_some());
        assert_eq!(default_custom_version_id("mongodb", Some("mongo7")), "mongo7");
        assert_eq!(
            default_custom_version_id("mongodb", None),
            "mongodbdefault"
        );
    }
}
