// Integration tests for workspace and version management commands
// These tests verify the end-to-end workspace and version workflow

use app_lib::engine::workspace_manager::WorkspaceManager;
use app_lib::engine::version_manifest::VersionManifest;
use app_lib::engine::user_override_manager::UserOverrideManager;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_workspace_manager_save_and_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    // Save workspace config
    let save_result = WorkspaceManager::save_workspace(workspace_path.to_str().unwrap());
    assert!(save_result.is_ok(), "Save workspace should succeed");

    // Load workspace config
    let load_result = WorkspaceManager::load_workspace();
    assert!(load_result.is_ok(), "Load workspace should succeed");

    let config = load_result.unwrap();
    assert!(config.is_some(), "Workspace config should exist");

    let config = config.unwrap();
    assert_eq!(config.workspace_path, workspace_path.to_str().unwrap());
}

#[test]
fn test_workspace_manager_load_nonexistent() {
    // When no workspace.json exists, load should return None
    let load_result = WorkspaceManager::load_workspace();
    assert!(load_result.is_ok(), "Load workspace should succeed even if file doesn't exist");
}

#[test]
fn test_version_manifest_load() {
    let manifest = VersionManifest::new();

    // Test that we can get entries for each service type
    let php_entries = manifest.get_available_entries(&app_lib::engine::version_manifest::ServiceType::Php);
    assert!(!php_entries.is_empty(), "Should have PHP versions");

    let mysql_entries = manifest.get_available_entries(&app_lib::engine::version_manifest::ServiceType::Mysql);
    assert!(!mysql_entries.is_empty(), "Should have MySQL versions");

    let redis_entries = manifest.get_available_entries(&app_lib::engine::version_manifest::ServiceType::Redis);
    assert!(!redis_entries.is_empty(), "Should have Redis versions");

    let nginx_entries = manifest.get_available_entries(&app_lib::engine::version_manifest::ServiceType::Nginx);
    assert!(!nginx_entries.is_empty(), "Should have Nginx versions");
}

#[test]
fn test_version_manifest_validate_id() {
    let manifest = VersionManifest::new();

    // Test valid IDs
    let php_entries = manifest.get_available_entries(&app_lib::engine::version_manifest::ServiceType::Php);
    if let Some((id, _)) = php_entries.first() {
        assert!(manifest.is_id_valid(&app_lib::engine::version_manifest::ServiceType::Php, id));
    }

    // Test invalid ID
    assert!(!manifest.is_id_valid(&app_lib::engine::version_manifest::ServiceType::Php, "nonexistent_version"));
}

#[test]
fn test_version_manifest_recommended() {
    let manifest = VersionManifest::new();

    // Each service type should have a recommended version
    let php_recommended = manifest.get_recommended_entry(&app_lib::engine::version_manifest::ServiceType::Php);
    assert!(php_recommended.is_some(), "Should have recommended PHP version");

    let mysql_recommended = manifest.get_recommended_entry(&app_lib::engine::version_manifest::ServiceType::Mysql);
    assert!(mysql_recommended.is_some(), "Should have recommended MySQL version");

    let redis_recommended = manifest.get_recommended_entry(&app_lib::engine::version_manifest::ServiceType::Redis);
    assert!(redis_recommended.is_some(), "Should have recommended Redis version");

    let nginx_recommended = manifest.get_recommended_entry(&app_lib::engine::version_manifest::ServiceType::Nginx);
    assert!(nginx_recommended.is_some(), "Should have recommended Nginx version");
}

#[test]
fn test_user_override_manager_save_and_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path().to_path_buf();

    let mut manager = UserOverrideManager::new(&project_root);

    // Initially, no overrides should exist
    let has_override = manager.has_user_override(
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82"
    );
    assert!(!has_override, "Should not have override initially");

    // Save an override
    let override_config = app_lib::engine::user_override_manager::UserVersionOverride {
        image_tag: "custom/php:8.2".to_string(),
        description: Some("Custom PHP 8.2".to_string()),
    };

    let save_result = manager.save_user_override(
        &project_root,
        app_lib::engine::version_manifest::ServiceType::Php,
        "php82".to_string(),
        override_config,
    );
    assert!(save_result.is_ok(), "Save override should succeed");

    // Now should have override
    let has_override = manager.has_user_override(
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82"
    );
    assert!(has_override, "Should have override after save");

    // Get merged entry should return custom values
    let merged = manager.get_merged_entry(
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82"
    );
    assert!(merged.is_some(), "Should get merged entry");
    let merged = merged.unwrap();
    assert_eq!(merged.image_tag, "custom/php:8.2");
    assert_eq!(merged.description, Some("Custom PHP 8.2".to_string()));
}

#[test]
fn test_user_override_manager_remove() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path().to_path_buf();

    let mut manager = UserOverrideManager::new(&project_root);

    // Save an override
    let override_config = app_lib::engine::user_override_manager::UserVersionOverride {
        image_tag: "custom/php:8.2".to_string(),
        description: None,
    };

    manager.save_user_override(
        &project_root,
        app_lib::engine::version_manifest::ServiceType::Php,
        "php82".to_string(),
        override_config,
    ).unwrap();

    // Remove the override
    let remove_result = manager.remove_user_override(
        &project_root,
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82",
    );
    assert!(remove_result.is_ok(), "Remove override should succeed");

    // Should not have override anymore
    let has_override = manager.has_user_override(
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82"
    );
    assert!(!has_override, "Should not have override after remove");
}

#[test]
fn test_user_override_manager_reset_all() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_root = temp_dir.path().to_path_buf();

    let mut manager = UserOverrideManager::new(&project_root);

    // Save multiple overrides
    let override1 = app_lib::engine::user_override_manager::UserVersionOverride {
        image_tag: "custom/php:8.2".to_string(),
        description: None,
    };

    let override2 = app_lib::engine::user_override_manager::UserVersionOverride {
        image_tag: "custom/mysql:8.0".to_string(),
        description: None,
    };

    manager.save_user_override(
        &project_root,
        app_lib::engine::version_manifest::ServiceType::Php,
        "php82".to_string(),
        override1,
    ).unwrap();

    manager.save_user_override(
        &project_root,
        app_lib::engine::version_manifest::ServiceType::Mysql,
        "mysql80".to_string(),
        override2,
    ).unwrap();

    // Reset all overrides
    let reset_result = manager.reset_all_overrides(&project_root);
    assert!(reset_result.is_ok(), "Reset all overrides should succeed");

    // Should not have any overrides
    let has_php_override = manager.has_user_override(
        &app_lib::engine::version_manifest::ServiceType::Php,
        "php82"
    );
    let has_mysql_override = manager.has_user_override(
        &app_lib::engine::version_manifest::ServiceType::Mysql,
        "mysql80"
    );

    assert!(!has_php_override, "Should not have PHP override after reset");
    assert!(!has_mysql_override, "Should not have MySQL override after reset");
}

#[test]
fn test_env_parser_integration() {
    use app_lib::engine::env_parser::EnvFile;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let env_path = temp_dir.path().join(".env");

    // Create a sample .env file
    let env_content = r#"# PHP-Stack Configuration
SOURCE_DIR=./www
TZ=Asia/Shanghai

# PHP Settings
PHP82_VERSION=8.2-fpm
PHP82_HOST_PORT=9000
PHP82_EXTENSIONS=mysqli,mbstring,pdo_mysql

# MySQL Settings
MYSQL80_VERSION=8.0
MYSQL80_HOST_PORT=3306
MYSQL_ROOT_PASSWORD=secret123
"#;
    fs::write(&env_path, env_content).expect("Failed to write .env file");

    // Parse the file
    let content = fs::read_to_string(&env_path).expect("Failed to read .env file");
    let env = EnvFile::parse(&content).expect("Failed to parse .env file");

    // Verify values
    assert_eq!(env.get("SOURCE_DIR"), Some("./www"));
    assert_eq!(env.get("TZ"), Some("Asia/Shanghai"));
    assert_eq!(env.get("PHP82_VERSION"), Some("8.2-fpm"));
    assert_eq!(env.get("PHP82_HOST_PORT"), Some("9000"));
    assert_eq!(env.get("MYSQL_ROOT_PASSWORD"), Some("secret123"));

    // Modify a value
    let mut env = env;
    env.set("MYSQL_ROOT_PASSWORD", "new_secret");

    // Format and re-parse
    let formatted = env.format();
    let env2 = EnvFile::parse(&formatted).expect("Failed to re-parse .env file");
    assert_eq!(env2.get("MYSQL_ROOT_PASSWORD"), Some("new_secret"));

    // Save back to file
    fs::write(&env_path, formatted).expect("Failed to write updated .env file");

    // Read and verify
    let content2 = fs::read_to_string(&env_path).expect("Failed to read updated .env file");
    let env3 = EnvFile::parse(&content2).expect("Failed to parse updated .env file");
    assert_eq!(env3.get("MYSQL_ROOT_PASSWORD"), Some("new_secret"));
}