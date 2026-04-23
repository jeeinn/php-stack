// Integration tests for configuration generation
// These tests verify the end-to-end config generation workflow

use app_lib::engine::config_generator;
use app_lib::engine::env_parser;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_generation_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();
    
    // Create a sample environment configuration
    let mut env_map = std::collections::HashMap::new();
    env_map.insert("PHP_VERSION".to_string(), "8.2".to_string());
    env_map.insert("MYSQL_HOST_PORT".to_string(), "3306".to_string());
    env_map.insert("NGINX_HOST_PORT".to_string(), "8080".to_string());
    
    // Generate .env file
    let env_result = config_generator::generate_env_file(&workspace_path, &env_map);
    assert!(env_result.is_ok(), "Env file generation should succeed");
    
    // Verify .env file exists
    let env_path = workspace_path.join(".env");
    assert!(env_path.exists(), ".env file should exist");
    
    // Read and parse the generated .env file
    let env_content = fs::read_to_string(&env_path).expect("Failed to read .env file");
    let parsed_env = env_parser::EnvFile::parse(&env_content).expect("Failed to parse .env file");
    
    // Verify the content matches what we put in
    assert_eq!(parsed_env.get("PHP_VERSION"), Some("8.2"));
    assert_eq!(parsed_env.get("MYSQL_HOST_PORT"), Some("3306"));
    assert_eq!(parsed_env.get("NGINX_HOST_PORT"), Some("8080"));
}

#[test]
fn test_docker_compose_generation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();
    
    // Create a sample environment configuration
    let mut env_map = std::collections::HashMap::new();
    env_map.insert("PHP_VERSION".to_string(), "8.2".to_string());
    env_map.insert("MYSQL_HOST_PORT".to_string(), "3306".to_string());
    env_map.insert("NGINX_HOST_PORT".to_string(), "8080".to_string());
    
    // Generate docker-compose.yml
    let compose_result = config_generator::generate_docker_compose(&workspace_path, &env_map);
    assert!(compose_result.is_ok(), "Docker compose generation should succeed");
    
    // Verify docker-compose.yml exists
    let compose_path = workspace_path.join("docker-compose.yml");
    assert!(compose_path.exists(), "docker-compose.yml should exist");
    
    // Read and verify the content
    let compose_content = fs::read_to_string(&compose_path).expect("Failed to read docker-compose.yml");
    assert!(compose_content.contains("php"), "Compose file should contain php service");
    assert!(compose_content.contains("mysql"), "Compose file should contain mysql service");
    assert!(compose_content.contains("nginx"), "Compose file should contain nginx service");
}

#[test]
fn test_port_conflict_detection() {
    // Test port conflict detection functionality
    let used_ports = vec![3306, 8080, 6379];
    let new_port = 3306; // This conflicts with an existing port
    
    let has_conflict = config_generator::check_port_conflict(new_port, &used_ports);
    assert!(has_conflict, "Should detect port conflict");
    
    let non_conflicting_port = 3307; // This doesn't conflict
    let no_conflict = config_generator::check_port_conflict(non_conflicting_port, &used_ports);
    assert!(!no_conflict, "Should not detect port conflict");
}
