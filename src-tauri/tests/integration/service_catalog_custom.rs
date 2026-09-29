//! 自定义 image 服务端到端：catalog + version override → .env / compose → 回读
//!
//! 覆盖 v0.5 NEW-6：内置四类之外的用户自定义服务往返未被集成测试锁住。

use app_lib::commands::parse_env_to_services;
use app_lib::commands::AUTO_CREATED_DESC_PREFIX;
use app_lib::engine::config_generator::{ConfigGenerator, EnvConfig, ServiceEntry};
use app_lib::engine::service_catalog::{
    build_custom_descriptor, persist_custom_from_catalog, CustomServiceForm, ServiceCatalog,
};
use app_lib::engine::user_override_manager::UserOverrideManager;
use app_lib::engine::version_manifest::{VersionEntry, VersionManifest};
use tempfile::TempDir;

fn seed_mongodb_custom(root: &std::path::Path) -> (String, String) {
    let form = CustomServiceForm {
        id: "mongodb".to_string(),
        display_name: "MongoDB".to_string(),
        container_port: 27017,
        host_port: 27017,
        image_tag: "mongo:7".to_string(),
        version_id: Some("mongodbdefault".to_string()),
        short_alias: Some("mongo".to_string()),
        entrypoint: None,
        data_container_path: Some("/data/db".to_string()),
    };
    let descriptor = build_custom_descriptor(&form).expect("build descriptor");
    let mut catalog = ServiceCatalog::merged(root);
    catalog
        .insert_custom(descriptor.clone())
        .expect("insert custom");
    persist_custom_from_catalog(root, &catalog).expect("persist catalog");

    let mut overrides = UserOverrideManager::new(root);
    overrides
        .add_custom_version(
            root,
            "mongodb",
            "mongodbdefault".to_string(),
            VersionEntry {
                display_name: "MongoDB (mongodbdefault)".to_string(),
                image_tag: "mongo:7".to_string(),
                service_dir: "mongodbdefault".to_string(),
                default_port: 27017,
                show_port: true,
                eol: false,
                description: Some(format!("{AUTO_CREATED_DESC_PREFIX} mongodb")),
            },
        )
        .expect("add custom version");

    (descriptor.id, "mongodbdefault".to_string())
}

fn config_with_mongodb(version: &str) -> EnvConfig {
    EnvConfig {
        services: vec![
            ServiceEntry {
                service_type: "php".to_string(),
                version: "php82".to_string(),
                host_port: 9000,
                extensions: None,
            },
            ServiceEntry {
                service_type: "mongodb".to_string(),
                version: version.to_string(),
                host_port: 27017,
                extensions: None,
            },
        ],
        timezone: "Asia/Shanghai".to_string(),
        mysql_root_password: None,
        sites: vec![],
    }
}

#[test]
fn custom_image_service_env_compose_and_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    let (_kind, version) = seed_mongodb_custom(root);

    let config = config_with_mongodb(&version);
    ConfigGenerator::validate(&config, Some(root)).expect("validate with custom kind");

    let env = ConfigGenerator::generate_env(&config, None, root);
    let map = env.to_map();
    assert_eq!(
        map.get("MONGODBDEFAULT_VERSION").map(String::as_str),
        Some("mongo:7")
    );
    assert_eq!(
        map.get("MONGODBDEFAULT_HOST_PORT").map(String::as_str),
        Some("27017")
    );
    assert_eq!(
        map.get("MONGODBDEFAULT_DATA_DIR").map(String::as_str),
        Some("./data/mongodbdefault")
    );
    assert!(map.contains_key("PHP82_VERSION"));

    let compose = ConfigGenerator::generate_compose(&config, root);
    assert!(
        compose.contains("mongodbdefault:"),
        "compose should define mongodbdefault service, got:\n{compose}"
    );
    assert!(
        compose.contains("mongo:7") || compose.contains("${MONGODBDEFAULT_VERSION}"),
        "compose should reference mongo image, got:\n{compose}"
    );

    // 回读：parse_env_to_services 应还原 mongodb 实例
    let override_manager = UserOverrideManager::new(root);
    let manifest = VersionManifest::new();
    let parsed = parse_env_to_services(&map, &manifest, Some(&override_manager), root);
    let mongo = parsed
        .iter()
        .find(|s| s.service_type == "mongodb" && s.version == version)
        .expect("parsed services should include mongodb");
    assert_eq!(mongo.host_port, 27017);
    assert!(
        parsed.iter().any(|s| s.service_type == "php"),
        "php should still round-trip"
    );
}

#[test]
fn removing_custom_kind_stops_env_emission() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    let (kind, version) = seed_mongodb_custom(root);

    let config = config_with_mongodb(&version);
    let env_with = ConfigGenerator::generate_env(&config, None, root);
    assert!(env_with.to_map().contains_key("MONGODBDEFAULT_VERSION"));

    // 移除 catalog + overrides（模拟 deleteCustomKind）
    let mut catalog = ServiceCatalog::merged(root);
    catalog.remove_custom(&kind).expect("remove custom");
    persist_custom_from_catalog(root, &catalog).expect("persist");
    let mut overrides = UserOverrideManager::new(root);
    overrides
        .remove_kind(root, &kind)
        .expect("remove kind overrides");

    let php_only = EnvConfig {
        services: vec![ServiceEntry {
            service_type: "php".to_string(),
            version: "php82".to_string(),
            host_port: 9000,
            extensions: None,
        }],
        timezone: "Asia/Shanghai".to_string(),
        mysql_root_password: None,
        sites: vec![],
    };

    // 全新生成不应再写入 mongodb 键
    let env_fresh = ConfigGenerator::generate_env(&php_only, None, root);
    assert!(!env_fresh.to_map().contains_key("MONGODBDEFAULT_VERSION"));
    assert!(env_fresh.to_map().contains_key("PHP82_VERSION"));

    // 合并已有 .env：已删 kind 的前缀不在 known_prefixes 内，键可能残留（NEW-1 取舍），
    // 但 parse_env_to_services 不应再回读成服务
    let env_merged = ConfigGenerator::generate_env(&php_only, Some(&env_with), root);
    let override_manager = UserOverrideManager::new(root);
    let parsed = parse_env_to_services(
        &env_merged.to_map(),
        &VersionManifest::new(),
        Some(&override_manager),
        root,
    );
    assert!(
        !parsed.iter().any(|s| s.service_type == "mongodb"),
        "deleted custom kind must not round-trip from residual env keys"
    );

    // catalog 已无 mongodb 时，带 mongodb 的 config 应校验失败
    let err = ConfigGenerator::validate(&config, Some(root)).unwrap_err();
    assert!(
        err.contains("Unknown service type") || err.contains("mongodb"),
        "expected unknown kind error, got: {err}"
    );
}
