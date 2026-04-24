use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::io::Write;
use chrono::Local;
use zip::write::FileOptions;

use super::env_parser::EnvFile;
use super::version_manifest::{VersionManifest, ServiceType as VmServiceType};
use super::user_override_manager::UserOverrideManager;
use super::mirror_config_manager::UserMirrorConfig;
use crate::app_log;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceType {
    PHP,
    MySQL,
    Redis,
    Nginx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub service_type: ServiceType,
    pub version: String,
    pub host_port: u16,
    pub extensions: Option<Vec<String>>, // Only for PHP
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    pub services: Vec<ServiceEntry>,
    pub source_dir: String,
    pub timezone: String,
    pub mysql_root_password: Option<String>,  // MySQL root密码（可选）
}

pub struct ConfigGenerator;

/// Backup state enum for two-phase commit
enum BackupState {
    NothingToBackup,
    Ready {
        timestamp: String,
        items: Vec<String>,
    },
}

impl ConfigGenerator {
    /// Get project root directory (parent of src-tauri)
    fn get_project_root() -> std::path::PathBuf {
        if cfg!(debug_assertions) {
            // 开发模式：项目根目录（src-tauri 的父目录）
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // target/debug/
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // target/
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // src-tauri/
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // 项目根目录/
                .unwrap_or(std::path::PathBuf::from("."))
        } else {
            // 生产模式：可执行文件所在目录
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or(std::path::PathBuf::from("."))
        }
    }

    /// Validate config: check for port conflicts.
    /// Returns Err with message containing conflicting port and service names.
    pub fn validate(config: &EnvConfig) -> Result<(), String> {
        let mut port_services: std::collections::HashMap<u16, Vec<String>> =
            std::collections::HashMap::new();

        for service in &config.services {
            let name = match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "-");
                    format!("PHP-{ver}")
                }
                ServiceType::MySQL => "MySQL".to_string(),
                ServiceType::Redis => "Redis".to_string(),
                ServiceType::Nginx => "Nginx".to_string(),
            };
            port_services
                .entry(service.host_port)
                .or_default()
                .push(name);
        }

        for (port, services) in &port_services {
            if services.len() > 1 {
                return Err(format!(
                    "端口冲突: 端口 {} 被以下服务同时使用: {}",
                    port,
                    services.join(", ")
                ));
            }
        }

        Ok(())
    }

    /// Generate .env file content from EnvConfig.
    /// If existing_env is provided, preserve user custom variables.
    /// Also merges mirror configuration from .user_mirror_config.json.
    pub fn generate_env(config: &EnvConfig, existing_env: Option<&EnvFile>) -> EnvFile {
        // Collect all managed keys so we know what NOT to treat as custom
        let managed_keys = Self::managed_keys(config);

        let mut env = if let Some(existing) = existing_env {
            existing.clone()
        } else {
            EnvFile { lines: Vec::new() }
        };

        // Set global variables
        env.set("SOURCE_DIR", &config.source_dir);
        env.set("TZ", &config.timezone);
        env.set("DATA_DIR", "./data");

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    
                    // Get the full image tag from version_manifest or user override
                    let manifest = VersionManifest::new();
                    let project_root = Self::get_project_root();
                    let override_manager = UserOverrideManager::new(&project_root);
                    
                    // Get merged image info (user override > default manifest)
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Php, &service.version)
                        .map(|info| format!("{}:{}", info.image, info.tag))
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Php, &service.version)
                                .map(|info| format!("{}:{}", info.image, info.tag))
                                .unwrap_or(format!("php:{}-fpm", service.version))
                        });
                    
                    // Set the full image tag (e.g., php:8.2-fpm or php:5.6-fpm-alpine)
                    env.set(
                        &format!("PHP{ver}_VERSION"),
                        &image_tag,
                    );
                    env.set(
                        &format!("PHP{ver}_HOST_PORT"),
                        &service.host_port.to_string(),
                    );
                    if let Some(exts) = &service.extensions {
                        env.set(
                            &format!("PHP{ver}_EXTENSIONS"),
                            &exts.join(","),
                        );
                    }
                    env.set(
                        &format!("PHP{ver}_PHP_CONF_FILE"),
                        &format!("./services/php{ver}/php.ini"),
                    );
                    env.set(
                        &format!("PHP{ver}_FPM_CONF_FILE"),
                        &format!("./services/php{ver}/php-fpm.conf"),
                    );
                    env.set(
                        &format!("PHP{ver}_LOG_DIR"),
                        &format!("./logs/php{ver}"),
                    );
                }
                ServiceType::MySQL => {
                    let manifest = VersionManifest::new();
                    // Try to load user overrides (if file exists)
                    let project_root = Self::get_project_root();
                    let override_manager = UserOverrideManager::new(&project_root);
                    
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "80".to_string()
                    };
                    
                    // Get the full image tag (user override > default manifest)
                    // Format: mysql:8.4
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Mysql, &service.version)
                        .map(|info| format!("{}:{}", info.image, info.tag))
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Mysql, &service.version)
                                .map(|info| format!("{}:{}", info.image, info.tag))
                                .unwrap_or(format!("mysql:{}", service.version))
                        });
                    
                    env.set(&format!("MYSQL{ver}_VERSION"), &image_tag);
                    env.set(&format!("MYSQL{ver}_HOST_PORT"), &service.host_port.to_string());
                    
                    // 设置MySQL root密码（优先使用用户配置的密码）
                    let root_password = config.mysql_root_password.as_deref().unwrap_or("root");
                    env.set("MYSQL_ROOT_PASSWORD", root_password);
                    
                    env.set(&format!("MYSQL{ver}_CONF_FILE"), &format!("./services/mysql{ver}/mysql.cnf"));
                    env.set(&format!("MYSQL{ver}_DATA_DIR"), &format!("./data/mysql{ver}"));
                    env.set(&format!("MYSQL{ver}_LOG_DIR"), &format!("./logs/mysql{ver}"));
                }
                ServiceType::Redis => {
                    let manifest = VersionManifest::new();
                    let project_root = Self::get_project_root();
                    let override_manager = UserOverrideManager::new(&project_root);
                    
                    // Generate service directory name: redis{major}{minor}
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "72".to_string()
                    };
                    
                    // Get the full image tag (user override > default manifest)
                    // Format: redis:7.2-alpine
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Redis, version_base)
                        .map(|info| format!("{}:{}", info.image, info.tag))
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Redis, version_base)
                                .map(|info| format!("{}:{}", info.image, info.tag))
                                .unwrap_or(format!("redis:{}-alpine", version_base))
                        });
                    
                    env.set(&format!("REDIS{ver}_VERSION"), &image_tag);
                    env.set(&format!("REDIS{ver}_HOST_PORT"), &service.host_port.to_string());
                    env.set(&format!("REDIS{ver}_CONF_FILE"), &format!("./services/redis{ver}/redis.conf"));
                    env.set(&format!("REDIS{ver}_DATA_DIR"), &format!("./data/redis{ver}"));
                }
                ServiceType::Nginx => {
                    let manifest = VersionManifest::new();
                    let project_root = Self::get_project_root();
                    let override_manager = UserOverrideManager::new(&project_root);
                    
                    // Generate service directory name: nginx{major}{minor}
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "127".to_string()
                    };
                    
                    // Get the full image tag (user override > default manifest)
                    // Format: nginx:1.27-alpine
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Nginx, version_base)
                        .map(|info| format!("{}:{}", info.image, info.tag))
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Nginx, version_base)
                                .map(|info| format!("{}:{}", info.image, info.tag))
                                .unwrap_or(format!("nginx:{}-alpine", version_base))
                        });
                    
                    env.set(&format!("NGINX{ver}_VERSION"), &image_tag);
                    env.set(&format!("NGINX{ver}_HTTP_HOST_PORT"), &service.host_port.to_string());
                    env.set(&format!("NGINX{ver}_BUILD_CONTEXT"), &format!("./services/nginx{ver}"));
                    env.set(&format!("NGINX{ver}_CONF_FILE"), &format!("./services/nginx{ver}/nginx.conf"));
                    env.set(&format!("NGINX{ver}_CONFD_DIR"), &format!("./services/nginx{ver}/conf.d"));
                    env.set("NGINX_LOG_DIR", "./logs/nginx");
                }
            }
        }

        // Merge mirror configuration from .user_mirror_config.json
        let project_root = Self::get_project_root();
        if let Ok(user_mirror_config) = UserMirrorConfig::load(&project_root) {
            // APT Mirror
            if let Some(apt_cat) = user_mirror_config.get_category("apt") {
                if apt_cat.enabled && !apt_cat.source.is_empty() {
                    env.set("APT_MIRROR", &apt_cat.source);
                }
            }
            
            // Composer Mirror
            if let Some(composer_cat) = user_mirror_config.get_category("composer") {
                if composer_cat.enabled && !composer_cat.source.is_empty() {
                    env.set("COMPOSER_MIRROR", &composer_cat.source);
                }
            }
            
            // NPM Mirror
            if let Some(npm_cat) = user_mirror_config.get_category("npm") {
                if npm_cat.enabled && !npm_cat.source.is_empty() {
                    env.set("NPM_MIRROR", &npm_cat.source);
                }
            }
            
            // GitHub Proxy
            if let Some(github_cat) = user_mirror_config.get_category("github_proxy") {
                if github_cat.enabled && !github_cat.source.is_empty() {
                    env.set("GITHUB_PROXY", &github_cat.source);
                }
            }
        }

        // managed_keys is used to identify which keys are managed by ConfigGenerator
        // All other keys in existing_env are preserved automatically since we started from a clone
        let _ = managed_keys;

        env
    }

    /// Generate docker-compose.yml content using ${VAR} interpolation.
    /// Reference dnmp pattern: each service uses ${VAR} for image, ports, volumes.
    pub fn generate_compose(config: &EnvConfig) -> String {
        let mut lines: Vec<String> = Vec::new();
        // Note: 'version' attribute is obsolete in modern Docker Compose, omit it
        lines.push("services:".to_string());

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    lines.push(format!("  php{ver}:"));
                    lines.push("    build:".to_string());
                    lines.push(format!("      context: ./services/php{ver}"));
                    lines.push("      args:".to_string());
                    // Pass the full image tag to Dockerfile's PHP_BASE_IMAGE ARG
                    lines.push(format!("        PHP_BASE_IMAGE: \"${{PHP{ver}_VERSION}}\""));
                    lines.push(format!("        PHP_EXTENSIONS: \"${{PHP{ver}_EXTENSIONS}}\""));
                    lines.push("        TZ: \"${TZ}\"".to_string());
                    // 镜像源配置（Debian APT 加速，适用于所有 PHP 版本）
                    // 注意：所有 PHP Dockerfile 现已统一使用 Debian 基础镜像（与 version_manifest.json 一致）
                    lines.push("        DEBIAN_MIRROR_DOMAIN: \"${APT_MIRROR:-deb.debian.org}\"".to_string());
                    lines.push("        COMPOSER_MIRROR: \"${COMPOSER_MIRROR:-https://packagist.org}\"".to_string());
                    lines.push("        GITHUB_PROXY: \"${GITHUB_PROXY:-}\"".to_string());
                    lines.push(format!("    container_name: ps-php{ver}"));
                    lines.push("    expose:".to_string());
                    lines.push("      - 9000".to_string());
                    lines.push("    volumes:".to_string());
                    lines.push("      - ${SOURCE_DIR}:/www/:rw".to_string());
                    lines.push(format!(
                        "      - ${{PHP{ver}_PHP_CONF_FILE}}:/usr/local/etc/php/php.ini"
                    ));
                    lines.push(format!(
                        "      - ${{PHP{ver}_FPM_CONF_FILE}}:/usr/local/etc/php-fpm.d/www.conf"
                    ));
                    lines.push(format!(
                        "      - ${{PHP{ver}_LOG_DIR}}:/var/log/php"
                    ));
                    lines.push("    restart: always".to_string());
                    lines.push("    networks:".to_string());
                    lines.push("      - php-stack-network".to_string());
                    lines.push(String::new());
                }
                ServiceType::MySQL => {
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "80".to_string()
                    };
                    
                    lines.push(format!("  mysql{ver}:"));
                    // Use full image tag directly (e.g., mysql:8.4)
                    lines.push(format!("    image: ${{MYSQL{ver}_VERSION}}"));
                    lines.push(format!("    container_name: ps-mysql{ver}"));
                    lines.push("    ports:".to_string());
                    lines.push(format!("      - \"${{MYSQL{ver}_HOST_PORT}}:3306\""));
                    lines.push("    volumes:".to_string());
                    lines.push(format!(
                        "      - ${{MYSQL{ver}_CONF_FILE}}:/etc/mysql/conf.d/mysql.cnf:ro"
                    ));
                    lines.push(format!(
                        "      - ${{MYSQL{ver}_DATA_DIR}}:/var/lib/mysql/:rw"
                    ));
                    lines.push(format!(
                        "      - ${{MYSQL{ver}_LOG_DIR}}:/var/log/mysql/:rw"
                    ));
                    lines.push("    restart: always".to_string());
                    lines.push("    environment:".to_string());
                    lines.push("      MYSQL_ROOT_PASSWORD: \"${MYSQL_ROOT_PASSWORD}\"".to_string());
                    lines.push("      TZ: \"${TZ}\"".to_string());
                    lines.push("    networks:".to_string());
                    lines.push("      - php-stack-network".to_string());
                    lines.push(String::new());
                }
                ServiceType::Redis => {
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "72".to_string()
                    };
                    
                    lines.push(format!("  redis{ver}:"));
                    // Use full image tag directly (e.g., redis:7.2-alpine)
                    lines.push(format!("    image: ${{REDIS{ver}_VERSION}}"));
                    lines.push(format!("    container_name: ps-redis{ver}"));
                    lines.push("    ports:".to_string());
                    lines.push(format!("      - \"${{REDIS{ver}_HOST_PORT}}:6379\""));
                    lines.push("    volumes:".to_string());
                    lines.push(format!(
                        "      - ${{REDIS{ver}_CONF_FILE}}:/etc/redis.conf:ro"
                    ));
                    lines.push(format!(
                        "      - ${{REDIS{ver}_DATA_DIR}}:/data/:rw"
                    ));
                    lines.push("    restart: always".to_string());
                    lines.push("    entrypoint: [\"redis-server\", \"/etc/redis.conf\"]".to_string());
                    lines.push("    networks:".to_string());
                    lines.push("      - php-stack-network".to_string());
                    lines.push(String::new());
                }
                ServiceType::Nginx => {
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "127".to_string()
                    };
                    
                    lines.push(format!("  nginx{ver}:"));
                    lines.push("    build:".to_string());
                    lines.push(format!("      context: ${{NGINX{ver}_BUILD_CONTEXT}}"));
                    lines.push("      args:".to_string());
                    // Pass the full image tag to Dockerfile's NGINX_BASE_IMAGE ARG
                    lines.push(format!("        NGINX_BASE_IMAGE: \"${{NGINX{ver}_VERSION}}\""));
                    lines.push(format!("    container_name: ps-nginx{ver}"));
                    lines.push("    ports:".to_string());
                    lines.push(format!("      - \"${{NGINX{ver}_HTTP_HOST_PORT}}:80\""));
                    lines.push("    volumes:".to_string());
                    lines.push("      - ${SOURCE_DIR}:/www/:rw".to_string());
                    lines.push(format!(
                        "      - ${{NGINX{ver}_CONF_FILE}}:/etc/nginx/nginx.conf"
                    ));
                    lines.push(format!(
                        "      - ${{NGINX{ver}_CONFD_DIR}}:/etc/nginx/conf.d"
                    ));
                    lines.push("      - ${NGINX_LOG_DIR}:/var/log/nginx".to_string());
                    lines.push("    restart: always".to_string());
                    lines.push("    networks:".to_string());
                    lines.push("      - php-stack-network".to_string());
                    lines.push(String::new());
                }
            }
        }

        lines.push("networks:".to_string());
        lines.push("  php-stack-network:".to_string());
        lines.push("    driver: bridge".to_string());

        lines.join("\n")
    }

    /// Copy template file from src-tauri/services to project services directory
    fn copy_template_file(template_name: &str, dest_path: &Path) -> Result<(), String> {
        // Get the executable directory
        let exe_dir = std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent()
            .ok_or("无法获取程序所在目录")?
            .to_path_buf();
        
        // Determine template source path
        let template_path = if cfg!(debug_assertions) {
            // Development mode: src-tauri/services/
            // current_exe() -> src-tauri/target/debug/app.exe
            exe_dir
                .parent()       // target/debug/ -> target/
                .and_then(|p| p.parent())   // target/ -> src-tauri/
                .map(|p| p.join("services").join(template_name))
        } else {
            // Production mode: executable_dir/services/
            Some(exe_dir.join("services").join(template_name))
        };
        
        let template_path = template_path
            .ok_or("无法定位模板目录")?;
        
        if !template_path.exists() {
            return Err(format!("模板文件不存在: {}", template_path.display()));
        }
        
        // Create destination directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {e}"))?;
        }
        
        // Skip if destination exists and is identical to template
        if dest_path.exists() {
            use std::io::Read;
            let mut template_file = std::fs::File::open(&template_path)
                .map_err(|e| format!("打开模板文件失败: {e}"))?;
            let mut dest_file = std::fs::File::open(dest_path)
                .map_err(|e| format!("打开目标文件失败: {e}"))?;
            
            let mut template_buf = Vec::new();
            let mut dest_buf = Vec::new();
            
            template_file.read_to_end(&mut template_buf)
                .map_err(|e| format!("读取模板文件失败: {e}"))?;
            dest_file.read_to_end(&mut dest_buf)
                .map_err(|e| format!("读取目标文件失败: {e}"))?;
            
            if template_buf == dest_buf {
                // Files are identical, skip copy
                return Ok(());
            }
        }
        
        // Copy file (will overwrite if different)
        std::fs::copy(&template_path, dest_path)
            .map_err(|e| format!("复制文件 {} 到 {} 失败: {e}", template_path.display(), dest_path.display()))?;
        
        Ok(())
    }

    /// Create services/, data/, logs/ directory structure.
    pub fn generate_service_dirs(config: &EnvConfig, root: &Path) -> Result<(), String> {
        // Create top-level directories
        std::fs::create_dir_all(root.join("services"))
            .map_err(|e| format!("创建 services/ 目录失败: {e}"))?;
        std::fs::create_dir_all(root.join("data"))
            .map_err(|e| format!("创建 data/ 目录失败: {e}"))?;
        std::fs::create_dir_all(root.join("logs"))
            .map_err(|e| format!("创建 logs/ 目录失败: {e}"))?;

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    let service_dir = root.join(format!("services/php{ver}"));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/php{ver}/ 目录失败: {e}"))?;

                    // Copy Dockerfile from template
                    let dockerfile_template = if service.version.starts_with("5.") {
                        "php56/Dockerfile"
                    } else if service.version.starts_with("7.") {
                        "php74/Dockerfile"
                    } else if service.version.starts_with("8.0") {
                        "php80/Dockerfile"
                    } else if service.version.starts_with("8.1") {
                        "php81/Dockerfile"
                    } else if service.version.starts_with("8.2") {
                        "php82/Dockerfile"
                    } else if service.version.starts_with("8.3") {
                        "php83/Dockerfile"
                    } else if service.version.starts_with("8.4") {
                        "php84/Dockerfile"
                    } else {
                        "php85/Dockerfile"  // Default to latest for unknown versions (including 8.5+)
                    };
                    Self::copy_template_file(
                        dockerfile_template,
                        &service_dir.join("Dockerfile"),
                    )?;

                    // Copy php.ini from template
                    let php_ini_template = if service.version.starts_with("5.") {
                        "php56/php.ini"
                    } else if service.version.starts_with("7.") {
                        "php74/php.ini"
                    } else if service.version.starts_with("8.0") {
                        "php80/php.ini"
                    } else if service.version.starts_with("8.1") {
                        "php81/php.ini"
                    } else if service.version.starts_with("8.2") {
                        "php82/php.ini"
                    } else if service.version.starts_with("8.3") {
                        "php83/php.ini"
                    } else if service.version.starts_with("8.4") {
                        "php84/php.ini"
                    } else {
                        "php85/php.ini"  // Default to latest for unknown versions (including 8.5+)
                    };
                    Self::copy_template_file(
                        php_ini_template,
                        &service_dir.join("php.ini"),
                    )?;

                    // Copy php-fpm.conf from template
                    let fpm_conf_template = if service.version.starts_with("5.") {
                        "php56/php-fpm.conf"
                    } else if service.version.starts_with("7.") {
                        "php74/php-fpm.conf"
                    } else if service.version.starts_with("8.0") {
                        "php80/php-fpm.conf"
                    } else if service.version.starts_with("8.1") {
                        "php81/php-fpm.conf"
                    } else if service.version.starts_with("8.2") {
                        "php82/php-fpm.conf"
                    } else if service.version.starts_with("8.3") {
                        "php83/php-fpm.conf"
                    } else if service.version.starts_with("8.4") {
                        "php84/php-fpm.conf"
                    } else {
                        "php85/php-fpm.conf"  // Default to latest for unknown versions (including 8.5+)
                    };
                    Self::copy_template_file(
                        fpm_conf_template,
                        &service_dir.join("php-fpm.conf"),
                    )?;

                    // Create log directory
                    std::fs::create_dir_all(root.join(format!("logs/php{ver}")))
                        .map_err(|e| format!("创建 logs/php{ver}/ 目录失败: {e}"))?;
                }
                ServiceType::MySQL => {
                    // Generate service directory name: mysql{major}{minor}
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("mysql{}{}", version_parts[0], version_parts[1])
                    } else {
                        "mysql80".to_string()
                    };
                    
                    let service_dir = root.join(format!("services/{service_dir_name}"));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{service_dir_name}/ 目录失败: {e}"))?;

                    // Copy mysql.cnf from template
                    // For now, use mysql80 as base template for all versions
                    // In future, can add version-specific templates
                    let template_name = if service.version.starts_with("5.") {
                        "mysql57/mysql.cnf"
                    } else {
                        "mysql80/mysql.cnf"
                    };
                    Self::copy_template_file(
                        template_name,
                        &service_dir.join("mysql.cnf"),
                    )?;

                    // Create data and log directories
                    std::fs::create_dir_all(root.join(format!("data/{service_dir_name}")))
                        .map_err(|e| format!("创建 data/{service_dir_name}/ 目录失败: {e}"))?;
                    std::fs::create_dir_all(root.join(format!("logs/{service_dir_name}")))
                        .map_err(|e| format!("创建 logs/{service_dir_name}/ 目录失败: {e}"))?;
                }
                ServiceType::Redis => {
                    // Generate service directory name: redis{major}{minor}
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("redis{}{}", version_parts[0], version_parts[1])
                    } else {
                        "redis72".to_string()
                    };
                    
                    let service_dir = root.join(format!("services/{service_dir_name}"));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{service_dir_name}/ 目录失败: {e}"))?;

                    // Copy redis.conf from template
                    // Use redis72 as base template for all versions
                    Self::copy_template_file(
                        "redis72/redis.conf",
                        &service_dir.join("redis.conf"),
                    )?;

                    // Create data directory
                    std::fs::create_dir_all(root.join(format!("data/{service_dir_name}")))
                        .map_err(|e| format!("创建 data/{service_dir_name}/ 目录失败: {e}"))?;
                }
                ServiceType::Nginx => {
                    // Generate service directory name: nginx{major}{minor}
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("nginx{}{}", version_parts[0], version_parts[1])
                    } else {
                        "nginx127".to_string()
                    };
                    
                    let service_dir = root.join(format!("services/{service_dir_name}"));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{service_dir_name}/ 目录失败: {e}"))?;
                    std::fs::create_dir_all(root.join(format!("services/{service_dir_name}/conf.d")))
                        .map_err(|e| format!("创建 services/{service_dir_name}/conf.d/ 目录失败: {e}"))?;

                    // Copy Dockerfile from template
                    Self::copy_template_file(
                        "nginx127/Dockerfile",
                        &service_dir.join("Dockerfile"),
                    )?;

                    // Copy nginx.conf from template
                    Self::copy_template_file(
                        "nginx127/nginx.conf",
                        &service_dir.join("nginx.conf"),
                    )?;

                    // Copy default.conf from template
                    Self::copy_template_file(
                        "nginx127/conf.d/default.conf",
                        &service_dir.join("conf.d/default.conf"),
                    )?;

                    // Create log directory
                    std::fs::create_dir_all(root.join("logs/nginx"))
                        .map_err(|e| format!("创建 logs/nginx/ 目录失败: {e}"))?;
                }
            }
        }

        Ok(())
    }

    /// Phase 1: Pre-check backup feasibility
    /// Returns BackupState or error if pre-check fails
    fn precheck_backup(project_root: &Path) -> Result<BackupState, String> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        
        // List of files/directories to backup
        let items_to_backup = vec![".env", "docker-compose.yml", "services"];
        let mut existing_items = Vec::new();
        
        // Check which items exist
        for item in &items_to_backup {
            let path = project_root.join(item);
            if path.exists() {
                existing_items.push((*item).to_string());
            }
        }
        
        if existing_items.is_empty() {
            return Ok(BackupState::NothingToBackup);
        }
        
        // Pre-check: verify backup zip file doesn't exist (avoid overwriting old backups)
        let backup_zip_name = format!("config_backup_{timestamp}.zip");
        let backup_zip_path = project_root.join(&backup_zip_name);
        
        if backup_zip_path.exists() {
            return Err(format!("备份文件已存在，请删除后重试: {backup_zip_name}"));
        }
        
        Ok(BackupState::Ready { 
            timestamp, 
            items: existing_items 
        })
    }

    /// Phase 2: Execute backup with atomic rollback
    /// Creates a ZIP archive containing all config files
    fn execute_backup(state: BackupState, project_root: &Path) -> Result<Vec<String>, String> {
        match state {
            BackupState::NothingToBackup => Ok(vec![]),
            BackupState::Ready { timestamp, items } => {
                let backup_zip_name = format!("config_backup_{timestamp}.zip");
                let backup_zip_path = project_root.join(&backup_zip_name);
                
                app_log!(info, "engine::config_generator", "开始创建配置备份: {}", backup_zip_name);
                
                // Create ZIP file
                let file = std::fs::File::create(&backup_zip_path)
                    .map_err(|e| format!("创建备份文件失败: {e}"))?;
                let mut zip = zip::ZipWriter::new(file);
                let zip_options = FileOptions::<()>::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                
                let mut backed_up_count = 0;
                
                // Add each item to the ZIP
                for item in &items {
                    let item_path = project_root.join(item);
                    
                    if item_path.is_file() {
                        // Add single file
                        match std::fs::read(&item_path) {
                            Ok(content) => {
                                zip.start_file(item, zip_options)
                                    .map_err(|e| format!("添加文件到ZIP失败: {e}"))?;
                                zip.write_all(&content)
                                    .map_err(|e| format!("写入文件内容失败: {e}"))?;
                                app_log!(info, "engine::config_generator", "已添加到备份: {}", item);
                                backed_up_count += 1;
                            }
                            Err(e) => {
                                app_log!(error, "engine::config_generator", "读取文件 {} 失败: {}", item, e);
                                // Continue with other files, don't fail entire backup
                            }
                        }
                    } else if item_path.is_dir() {
                        // Add directory recursively
                        match Self::add_dir_to_zip_recursive(&mut zip, &item_path, item, zip_options) {
                            Ok(count) => {
                                app_log!(info, "engine::config_generator", "已添加目录 {} ({} 个文件)", item, count);
                                backed_up_count += count;
                            }
                            Err(e) => {
                                app_log!(error, "engine::config_generator", "添加目录 {} 失败: {}", item, e);
                                // Continue with other items
                            }
                        }
                    }
                }
                
                // Add user custom configuration files (same as backup_engine.rs)
                let user_config_files = vec![
                    ".user_mirror_config.json",
                    ".user_version_overrides.json",
                ];
                
                for config_file in &user_config_files {
                    let config_path = project_root.join(config_file);
                    if config_path.exists() {
                        match std::fs::read(&config_path) {
                            Ok(content) => {
                                zip.start_file(config_file, zip_options)
                                    .map_err(|e| format!("添加用户配置文件到ZIP失败: {e}"))?;
                                zip.write_all(&content)
                                    .map_err(|e| format!("写入用户配置文件失败: {e}"))?;
                                app_log!(info, "engine::config_generator", "已添加用户配置: {}", config_file);
                                backed_up_count += 1;
                            }
                            Err(e) => {
                                app_log!(warn, "engine::config_generator", "读取用户配置文件 {} 失败: {}", config_file, e);
                                // Continue with other config files
                            }
                        }
                    }
                }
                
                // Finish ZIP file
                zip.finish().map_err(|e| format!("完成ZIP文件失败: {e}"))?;
                
                if backed_up_count == 0 {
                    // No files were successfully added, delete the empty ZIP
                    let _ = std::fs::remove_file(&backup_zip_path);
                    app_log!(warn, "engine::config_generator", "没有文件被成功备份，已删除空ZIP文件");
                    return Ok(vec![]);
                }
                
                app_log!(info, "engine::config_generator", "备份完成: {} (共 {} 个文件/目录项)", backup_zip_name, items.len());
                Ok(vec![backup_zip_name])
            }
        }
    }
    
    /// Recursively add directory contents to ZIP
    fn add_dir_to_zip_recursive(
        zip: &mut zip::ZipWriter<std::fs::File>,
        dir_path: &Path,
        zip_base_path: &str,
        options: FileOptions<()>,
    ) -> Result<usize, String> {
        let mut file_count = 0;
        
        for entry in std::fs::read_dir(dir_path)
            .map_err(|e| format!("读取目录失败: {e}"))?
        {
            let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(file_name) = path.file_name() {
                    let zip_path = format!("{}/{}", zip_base_path, file_name.to_string_lossy());
                    match std::fs::read(&path) {
                        Ok(content) => {
                            zip.start_file(&zip_path, options)
                                .map_err(|e| format!("添加文件到ZIP失败: {e}"))?;
                            zip.write_all(&content)
                                .map_err(|e| format!("写入文件内容失败: {e}"))?;
                            file_count += 1;
                        }
                        Err(e) => {
                            app_log!(warn, "engine::config_generator", "跳过文件 {:?}: {}", path, e);
                        }
                    }
                }
            } else if path.is_dir() {
                if let Some(dir_name) = path.file_name() {
                    let sub_zip_path = format!("{}/{}", zip_base_path, dir_name.to_string_lossy());
                    let sub_count = Self::add_dir_to_zip_recursive(zip, &path, &sub_zip_path, options)?;
                    file_count += sub_count;
                }
            }
        }
        
        Ok(file_count)
    }

    /// Backup existing configuration files by creating a ZIP archive.
    /// Format: config_backup_YYYYMMDD_HHMMSS.zip
    /// Contains: .env, docker-compose.yml, services/, .user_mirror_config.json, .user_version_overrides.json
    pub fn backup_existing_config(project_root: &Path) -> Result<Vec<String>, String> {
        app_log!(info, "engine::config_generator", "开始预检查备份...");
        
        // Phase 1: Pre-check
        let backup_state = Self::precheck_backup(project_root)?;
        
        // Phase 2: Execute with rollback
        app_log!(info, "engine::config_generator", "执行备份...");
        Self::execute_backup(backup_state, project_root)
    }

    /// Apply config: write .env, docker-compose.yml, create directories.
    /// If enable_backup is true, backup existing config files before overwriting.
    pub async fn apply(config: &EnvConfig, project_root: &Path, enable_backup: bool) -> Result<Vec<String>, String> {
        // Validate first
        Self::validate(config)?;

        // Backup existing config if requested
        let mut backed_up_files = Vec::new();
        if enable_backup {
            backed_up_files = Self::backup_existing_config(project_root)?;
        }

        // Read existing .env if present
        let env_path = project_root.join(".env");
        let existing_env = if env_path.exists() {
            let content = std::fs::read_to_string(&env_path)
                .map_err(|e| format!("读取 .env 文件失败: {e}"))?;
            Some(
                EnvFile::parse(&content)
                    .map_err(|e| format!("解析 .env 文件失败: {e}"))?,
            )
        } else {
            None
        };

        // Generate and write .env
        let env_file = Self::generate_env(config, existing_env.as_ref());
        std::fs::write(&env_path, env_file.format())
            .map_err(|e| format!("写入 .env 文件失败: {e}"))?;

        // Generate and write docker-compose.yml
        let compose = Self::generate_compose(config);
        std::fs::write(project_root.join("docker-compose.yml"), compose)
            .map_err(|e| format!("写入 docker-compose.yml 失败: {e}"))?;

        // Create directory structure
        Self::generate_service_dirs(config, project_root)?;
        
        // Generate .npmrc file in workspace path if NPM mirror is configured
        let npm_mirror = env_file.get("NPM_MIRROR").unwrap_or("");
        if !npm_mirror.is_empty() && npm_mirror != "https://registry.npmjs.org" {
            // 从 workspace.json 获取工作区路径
            let workspace_path = if let Some(workspace_config) = crate::engine::workspace_manager::WorkspaceManager::load_workspace()? {
                PathBuf::from(workspace_config.workspace_path)
            } else {
                // 如果没有配置 workspace，使用项目根目录作为后备
                project_root.to_path_buf()
            };
            
            let npmrc_content = format!("registry={npm_mirror}\n");
            let npmrc_path = workspace_path.join(".npmrc");
            std::fs::write(&npmrc_path, npmrc_content)
                .map_err(|e| format!("写入 .npmrc 文件失败: {e}"))?;
        }

        Ok(backed_up_files)
    }

    /// Collect all keys managed by ConfigGenerator for a given config.
    /// Used to distinguish managed keys from user custom variables.
    fn managed_keys(config: &EnvConfig) -> HashSet<String> {
        let mut keys = HashSet::new();
        keys.insert("SOURCE_DIR".to_string());
        keys.insert("TZ".to_string());
        keys.insert("DATA_DIR".to_string());
        keys.insert("MYSQL_ROOT_PASSWORD".to_string());

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    keys.insert(format!("PHP{ver}_VERSION"));
                    keys.insert(format!("PHP{ver}_HOST_PORT"));
                    keys.insert(format!("PHP{ver}_EXTENSIONS"));
                    keys.insert(format!("PHP{ver}_PHP_CONF_FILE"));
                    keys.insert(format!("PHP{ver}_FPM_CONF_FILE"));
                    keys.insert(format!("PHP{ver}_LOG_DIR"));
                }
                ServiceType::MySQL => {
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "80".to_string()
                    };
                    keys.insert(format!("MYSQL{ver}_VERSION"));
                    keys.insert(format!("MYSQL{ver}_HOST_PORT"));
                    keys.insert(format!("MYSQL{ver}_CONF_FILE"));
                    keys.insert(format!("MYSQL{ver}_DATA_DIR"));
                    keys.insert(format!("MYSQL{ver}_LOG_DIR"));
                }
                ServiceType::Redis => {
                    keys.insert("REDIS_VERSION".to_string());
                    keys.insert("REDIS_HOST_PORT".to_string());
                    keys.insert("REDIS_CONF_FILE".to_string());
                }
                ServiceType::Nginx => {
                    keys.insert("NGINX_VERSION".to_string());
                    keys.insert("NGINX_HTTP_HOST_PORT".to_string());
                    keys.insert("NGINX_CONF_FILE".to_string());
                    keys.insert("NGINX_CONFD_DIR".to_string());
                    keys.insert("NGINX_LOG_DIR".to_string());
                }
            }
        }

        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_basic_config() -> EnvConfig {
        EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: ServiceType::PHP,
                    version: "8.2".to_string(),
                    host_port: 9000,
                    extensions: Some(vec!["pdo_mysql".to_string(), "gd".to_string()]),
                },
                ServiceEntry {
                    service_type: ServiceType::MySQL,
                    version: "8.0".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: ServiceType::Redis,
                    version: "7.0".to_string(),
                    host_port: 6379,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: ServiceType::Nginx,
                    version: "1.25".to_string(),
                    host_port: 80,
                    extensions: None,
                },
            ],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        }
    }

    #[test]
    fn test_validate_no_conflict() {
        let config = make_basic_config();
        assert!(ConfigGenerator::validate(&config).is_ok());
    }

    #[test]
    fn test_validate_port_conflict() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: ServiceType::MySQL,
                    version: "8.0".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: ServiceType::Redis,
                    version: "7.0".to_string(),
                    host_port: 3306, // conflict!
                    extensions: None,
                },
            ],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        };
        let result = ConfigGenerator::validate(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("端口冲突"));
        assert!(err.contains("3306"));
        assert!(err.contains("MySQL"));
        assert!(err.contains("Redis"));
    }

    #[test]
    fn test_generate_env_basic() {
        let config = make_basic_config();
        let env = ConfigGenerator::generate_env(&config, None);
        let map = env.to_map();

        assert_eq!(map.get("SOURCE_DIR").unwrap(), "./www");
        assert_eq!(map.get("TZ").unwrap(), "Asia/Shanghai");
        assert_eq!(map.get("DATA_DIR").unwrap(), "./data");
        // PHP VERSION now contains full image tag (e.g., php:8.2-fpm)
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "php:8.2-fpm");
        assert_eq!(map.get("PHP82_HOST_PORT").unwrap(), "9000");
        assert_eq!(map.get("PHP82_EXTENSIONS").unwrap(), "pdo_mysql,gd");
        assert_eq!(
            map.get("PHP82_PHP_CONF_FILE").unwrap(),
            "./services/php82/php.ini"
        );
        assert_eq!(
            map.get("PHP82_FPM_CONF_FILE").unwrap(),
            "./services/php82/php-fpm.conf"
        );
        assert_eq!(map.get("PHP82_LOG_DIR").unwrap(), "./logs/php82");
        // MySQL 8.0 uses full image tag "mysql:8.0" from version_manifest.json
        assert_eq!(map.get("MYSQL80_VERSION").unwrap(), "mysql:8.0");
        assert_eq!(map.get("MYSQL80_HOST_PORT").unwrap(), "3306");
        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
        // Redis 7.0 uses full image tag "redis:7.0-alpine" from version_manifest.json
        assert_eq!(map.get("REDIS70_VERSION").unwrap(), "redis:7.0-alpine");
        assert_eq!(map.get("REDIS70_HOST_PORT").unwrap(), "6379");
        // Nginx 1.25 uses full image tag "nginx:1.25-alpine" from version_manifest.json
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "nginx:1.25-alpine");
        assert_eq!(map.get("NGINX125_HTTP_HOST_PORT").unwrap(), "80");
    }

    #[test]
    fn test_generate_env_preserves_custom_vars() {
        let existing_content = "# My custom config\nCUSTOM_VAR=hello\nSOURCE_DIR=./old";
        let existing_env = EnvFile::parse(existing_content).unwrap();

        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: ServiceType::Nginx,
                version: "1.25".to_string(),
                host_port: 80,
                extensions: None,
            }],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        };

        let env = ConfigGenerator::generate_env(&config, Some(&existing_env));
        let map = env.to_map();

        // Custom variable preserved
        assert_eq!(map.get("CUSTOM_VAR").unwrap(), "hello");
        // Managed variable updated
        assert_eq!(map.get("SOURCE_DIR").unwrap(), "./www");
        // New managed variable added (uses full image tag from version_manifest.json)
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "nginx:1.25-alpine");
    }

    #[test]
    fn test_generate_env_multiple_php() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: ServiceType::PHP,
                    version: "7.4".to_string(),
                    host_port: 9074,
                    extensions: Some(vec!["pdo_mysql".to_string()]),
                },
                ServiceEntry {
                    service_type: ServiceType::PHP,
                    version: "8.2".to_string(),
                    host_port: 9082,
                    extensions: Some(vec!["gd".to_string(), "curl".to_string()]),
                },
            ],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        };

        let env = ConfigGenerator::generate_env(&config, None);
        let map = env.to_map();

        // PHP 7.4 vars (full image tag)
        assert_eq!(map.get("PHP74_VERSION").unwrap(), "php:7.4-fpm");
        assert_eq!(map.get("PHP74_HOST_PORT").unwrap(), "9074");
        assert_eq!(map.get("PHP74_EXTENSIONS").unwrap(), "pdo_mysql");

        // PHP 8.2 vars (full image tag)
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "php:8.2-fpm");
        assert_eq!(map.get("PHP82_HOST_PORT").unwrap(), "9082");
        assert_eq!(map.get("PHP82_EXTENSIONS").unwrap(), "gd,curl");
    }

    #[test]
    fn test_generate_compose_uses_interpolation() {
        let config = make_basic_config();
        let compose = ConfigGenerator::generate_compose(&config);

        // Should contain ${VAR} interpolation, not hardcoded values
        assert!(compose.contains("${MYSQL80_VERSION}"));
        assert!(compose.contains("${MYSQL80_HOST_PORT}"));
        assert!(compose.contains("${REDIS70_VERSION}"));
        assert!(compose.contains("${REDIS70_HOST_PORT}"));
        assert!(compose.contains("${NGINX125_HTTP_HOST_PORT}"));
        assert!(compose.contains("${SOURCE_DIR}"));
        assert!(compose.contains("${PHP82_EXTENSIONS}"));
        assert!(compose.contains("${PHP82_PHP_CONF_FILE}"));
        assert!(compose.contains("${TZ}"));

        // Should NOT contain hardcoded values for versions/ports
        assert!(!compose.contains("image: mysql:8.0"));
        assert!(!compose.contains("\"3306:3306\""));
    }

    #[test]
    fn test_generate_compose_multiple_php() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: ServiceType::PHP,
                    version: "7.4".to_string(),
                    host_port: 9074,
                    extensions: Some(vec!["pdo_mysql".to_string()]),
                },
                ServiceEntry {
                    service_type: ServiceType::PHP,
                    version: "8.2".to_string(),
                    host_port: 9082,
                    extensions: Some(vec!["gd".to_string()]),
                },
            ],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        };

        let compose = ConfigGenerator::generate_compose(&config);

        // Should have 2 PHP service definitions
        assert!(compose.contains("container_name: ps-php74"));
        assert!(compose.contains("container_name: ps-php82"));

        // Each should have its own service block
        assert!(compose.contains("  php74:"));
        assert!(compose.contains("  php82:"));

        // Each should reference its own variables
        assert!(compose.contains("${PHP74_EXTENSIONS}"));
        assert!(compose.contains("${PHP82_EXTENSIONS}"));
    }

    #[test]
    fn test_generate_env_mysql_root_password() {
        // 测试自定义MySQL root密码
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: ServiceType::MySQL,
                version: "8.0".to_string(),
                host_port: 3306,
                extensions: None,
            }],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: Some("mypassword123".to_string()),
        };

        let env = ConfigGenerator::generate_env(&config, None);
        let map = env.to_map();

        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "mypassword123");
    }

    #[test]
    fn test_generate_env_mysql_default_password() {
        // 测试默认MySQL root密码（未设置时）
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: ServiceType::MySQL,
                version: "8.0".to_string(),
                host_port: 3306,
                extensions: None,
            }],
            source_dir: "./www".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
        };

        let env = ConfigGenerator::generate_env(&config, None);
        let map = env.to_map();

        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
    }
}
