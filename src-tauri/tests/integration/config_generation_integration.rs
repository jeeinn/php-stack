//! 配置生成端到端集成测试：EnvConfig → validate → .env 内容 → compose 内容
//!
//! 解析细节由 config_generator.rs 的单元测试覆盖，本文件验证完整链路。

use app_lib::engine::config_generator::{ConfigGenerator, EnvConfig, ServiceEntry, ServiceType};

fn sample_config() -> EnvConfig {
    EnvConfig {
        services: vec![
            ServiceEntry {
                service_type: ServiceType::PHP,
                version: "php82".to_string(),
                host_port: 9000,
                extensions: Some(vec!["pdo_mysql".to_string(), "mysqli".to_string()]),
            },
            ServiceEntry {
                service_type: ServiceType::MySQL,
                version: "mysql80".to_string(),
                host_port: 3306,
                extensions: None,
            },
        ],
        source_dir: "./www".to_string(),
        timezone: "Asia/Shanghai".to_string(),
        mysql_root_password: None,
    }
}

#[test]
fn test_validate_accepts_conflict_free_config() {
    ConfigGenerator::validate(&sample_config()).expect("无冲突配置应通过验证");
}

#[test]
fn test_validate_rejects_port_conflict() {
    let mut config = sample_config();
    config.services[1].host_port = 9000; // 与 PHP 服务同端口
    let result = ConfigGenerator::validate(&config);
    assert!(result.is_err(), "同端口配置应验证失败");
    assert!(
        result.unwrap_err().contains("9000"),
        "错误信息应包含冲突端口"
    );
}

#[test]
fn test_generate_env_contains_expected_keys() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let env = ConfigGenerator::generate_env(&sample_config(), None, tmp.path());
    let formatted = env.format();

    assert!(formatted.contains("SOURCE_DIR=./www"), "应包含 SOURCE_DIR");
    assert!(formatted.contains("TZ=Asia/Shanghai"), "应包含 TZ");
    assert!(
        formatted.contains("PHP82_VERSION=php:8.2-fpm"),
        "PHP82_VERSION 应来自 manifest 的 image_tag，实际:\n{formatted}"
    );
    assert!(
        formatted.contains("PHP82_EXTENSIONS=pdo_mysql,mysqli"),
        "PHP82_EXTENSIONS 应包含所选扩展"
    );
    assert!(
        formatted.contains("MYSQL80_HOST_PORT=3306"),
        "MYSQL80_HOST_PORT 应为 3306"
    );
    assert!(
        formatted.contains("MYSQL_ROOT_PASSWORD=root"),
        "未指定密码时应使用默认 root"
    );
}

#[test]
fn test_generate_compose_contains_services() {
    let compose = ConfigGenerator::generate_compose(&sample_config());

    assert!(
        compose.contains("php82"),
        "compose 应包含 php82 服务配置，实际:\n{compose}"
    );
    assert!(
        compose.contains("mysql80"),
        "compose 应包含 mysql80 服务配置"
    );
}

#[test]
fn test_generate_env_preserves_user_variables() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let existing = app_lib::engine::env_parser::EnvFile::parse("MY_CUSTOM_VAR=hello\n").unwrap();

    let env = ConfigGenerator::generate_env(&sample_config(), Some(&existing), tmp.path());
    let formatted = env.format();

    assert!(
        formatted.contains("MY_CUSTOM_VAR=hello"),
        "用户自定义变量应被保留，实际:\n{formatted}"
    );
}
