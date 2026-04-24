// Integration tests for backup and restore functionality
// These tests verify the end-to-end backup and restore workflow

use app_lib::engine::backup_engine::BackupEngine;
use app_lib::engine::restore_engine::RestoreEngine;
use app_lib::engine::backup_manifest::BackupOptions;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_backup_and_restore_workflow() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();
    
    // Create a sample .env file
    let env_content = "PHP_VERSION=8.2\nMYSQL_PORT=3306\n";
    let env_path = workspace_path.join(".env");
    fs::write(&env_path, env_content).expect("Failed to write .env file");
    
    // Create a sample docker-compose.yml file
    let compose_content = "version: '3'\nservices:\n  php:\n    image: php:8.2\n";
    let compose_path = workspace_path.join("docker-compose.yml");
    fs::write(&compose_path, compose_content).expect("Failed to write docker-compose.yml");
    
    // Test backup creation
    let backup_path = workspace_path.join("backup.zip");
    let options = BackupOptions {
        include_projects: false,
        project_patterns: vec![],
        include_logs: false,
    };
    
    let backup_result = BackupEngine::create_backup(
        backup_path.to_str().unwrap(),
        options,
        &workspace_path,
        None, // No app handle in tests
    ).await;
    
    assert!(backup_result.is_ok(), "Backup should succeed");
    
    // Verify backup file exists
    assert!(backup_path.exists(), "Backup file should exist");
    
    // Test restore from backup
    let restore_workspace = temp_dir.path().join("restored");
    fs::create_dir(&restore_workspace).expect("Failed to create restore dir");
    
    let restore_result = RestoreEngine::restore(
        backup_path.to_str().unwrap(),
        &restore_workspace,
        None, // No app handle in tests
    ).await;
    
    assert!(restore_result.is_ok(), "Restore should succeed");
    
    // Verify restored files exist
    let restored_env = restore_workspace.join(".env");
    assert!(restored_env.exists(), "Restored .env should exist");
    
    let restored_compose = restore_workspace.join("docker-compose.yml");
    assert!(restored_compose.exists(), "Restored docker-compose.yml should exist");
}

#[tokio::test]
async fn test_backup_with_database_export() {
    // This test would require a running MySQL container
    // For now, we'll just test the structure without actual DB export
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();
    
    // Create minimal environment for backup
    let env_content = "MYSQL_HOST_PORT=3306\n";
    let env_path = workspace_path.join(".env");
    fs::write(&env_path, env_content).expect("Failed to write .env file");
    
    let backup_path = workspace_path.join("backup_with_db.zip");
    let options = BackupOptions {
        include_projects: false,
        project_patterns: vec![],
        include_logs: false,
    };
    
    let backup_result = BackupEngine::create_backup(
        backup_path.to_str().unwrap(),
        options,
        &workspace_path,
        None,
    ).await;
    
    // Even without a real database, the backup should succeed
    assert!(backup_result.is_ok(), "Backup should succeed even without DB");
}
