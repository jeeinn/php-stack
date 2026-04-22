use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use chrono::Local;

use super::env_parser::EnvFile;
use super::version_manifest::{VersionManifest, ServiceType as VmServiceType};
use super::user_override_manager::UserOverrideManager;
use super::mirror_config_manager::UserMirrorConfig;

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
                    format!("PHP-{}", ver)
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
                    env.set(
                        &format!("PHP{}_VERSION", ver),
                        &service.version,
                    );
                    env.set(
                        &format!("PHP{}_HOST_PORT", ver),
                        &service.host_port.to_string(),
                    );
                    if let Some(exts) = &service.extensions {
                        env.set(
                            &format!("PHP{}_EXTENSIONS", ver),
                            &exts.join(","),
                        );
                    }
                    env.set(
                        &format!("PHP{}_PHP_CONF_FILE", ver),
                        &format!("./services/php{}/php.ini", ver),
                    );
                    env.set(
                        &format!("PHP{}_FPM_CONF_FILE", ver),
                        &format!("./services/php{}/php-fpm.conf", ver),
                    );
                    env.set(
                        &format!("PHP{}_LOG_DIR", ver),
                        &format!("./logs/php{}", ver),
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
                    
                    // Get the correct image tag (user override > default manifest)
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Mysql, &service.version)
                        .map(|info| info.tag.clone())
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Mysql, &service.version)
                                .map(|info| info.tag.clone())
                                .unwrap_or(service.version.clone())
                        });
                    
                    env.set(&format!("MYSQL{}_VERSION", ver), &image_tag);
                    env.set(&format!("MYSQL{}_HOST_PORT", ver), &service.host_port.to_string());
                    env.set("MYSQL_ROOT_PASSWORD", "root");
                    env.set(&format!("MYSQL{}_CONF_FILE", ver), &format!("./services/mysql{}/mysql.cnf", ver));
                    env.set(&format!("MYSQL{}_DATA_DIR", ver), &format!("./data/mysql{}", ver));
                    env.set(&format!("MYSQL{}_LOG_DIR", ver), &format!("./logs/mysql{}", ver));
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
                    
                    // Get the correct image tag (user override > default manifest)
                    // 注意：使用 version_base（纯版本号）来查找用户覆盖配置
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Redis, version_base)
                        .map(|info| info.tag.clone())
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Redis, version_base)
                                .map(|info| info.tag.clone())
                                .unwrap_or(service.version.clone())
                        });
                    
                    env.set(&format!("REDIS{}_VERSION", ver), &image_tag);
                    env.set(&format!("REDIS{}_HOST_PORT", ver), &service.host_port.to_string());
                    env.set(&format!("REDIS{}_CONF_FILE", ver), &format!("./services/redis{}/redis.conf", ver));
                    env.set(&format!("REDIS{}_DATA_DIR", ver), &format!("./data/redis{}", ver));
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
                    
                    // Get the correct image tag (user override > default manifest)
                    // 注意：使用 version_base（纯版本号）来查找用户覆盖配置
                    let image_tag = override_manager
                        .get_merged_image_info(&VmServiceType::Nginx, version_base)
                        .map(|info| info.tag.clone())
                        .unwrap_or_else(|| {
                            manifest
                                .get_image_info(&VmServiceType::Nginx, version_base)
                                .map(|info| info.tag.clone())
                                .unwrap_or(service.version.clone())
                        });
                    
                    env.set(&format!("NGINX{}_VERSION", ver), &image_tag);
                    env.set(&format!("NGINX{}_HTTP_HOST_PORT", ver), &service.host_port.to_string());
                    env.set(&format!("NGINX{}_BUILD_CONTEXT", ver), &format!("./services/nginx{}", ver));
                    env.set(&format!("NGINX{}_CONF_FILE", ver), &format!("./services/nginx{}/nginx.conf", ver));
                    env.set(&format!("NGINX{}_CONFD_DIR", ver), &format!("./services/nginx{}/conf.d", ver));
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
                    lines.push(format!("  php{}:", ver));
                    lines.push(format!("    build:"));
                    lines.push(format!("      context: ./services/php{}", ver));
                    lines.push(format!("      args:"));
                    lines.push(format!("        PHP_EXTENSIONS: \"${{PHP{}_EXTENSIONS}}\"", ver));
                    lines.push(format!("        TZ: \"${{TZ}}\""));
                    // 镜像源配置（仅容器内依赖）
                    lines.push(format!("        DEBIAN_MIRROR_DOMAIN: \"${{APT_MIRROR:-deb.debian.org}}\""));
                    lines.push(format!("        COMPOSER_MIRROR: \"${{COMPOSER_MIRROR:-https://packagist.org}}\""));
                    lines.push(format!("        GITHUB_PROXY: \"${{GITHUB_PROXY:-}}\""));
                    lines.push(format!("    container_name: ps-php{}", ver));
                    lines.push(format!("    expose:"));
                    lines.push(format!("      - 9000"));
                    lines.push(format!("    volumes:"));
                    lines.push(format!("      - ${{SOURCE_DIR}}:/www/:rw"));
                    lines.push(format!(
                        "      - ${{PHP{}_PHP_CONF_FILE}}:/usr/local/etc/php/php.ini",
                        ver
                    ));
                    lines.push(format!(
                        "      - ${{PHP{}_FPM_CONF_FILE}}:/usr/local/etc/php-fpm.d/www.conf",
                        ver
                    ));
                    lines.push(format!(
                        "      - ${{PHP{}_LOG_DIR}}:/var/log/php",
                        ver
                    ));
                    lines.push(format!("    restart: always"));
                    lines.push(format!("    networks:"));
                    lines.push(format!("      - php-stack-network"));
                    lines.push(String::new());
                }
                ServiceType::MySQL => {
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let ver = if version_parts.len() >= 2 {
                        format!("{}{}", version_parts[0], version_parts[1])
                    } else {
                        "80".to_string()
                    };
                    
                    lines.push(format!("  mysql{}:", ver));
                    lines.push(format!("    image: mysql:${{MYSQL{}_VERSION}}", ver));
                    lines.push(format!("    container_name: ps-mysql{}", ver));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{MYSQL{}_HOST_PORT}}:3306\"", ver));
                    lines.push(format!("    volumes:"));
                    lines.push(format!(
                        "      - ${{MYSQL{}_CONF_FILE}}:/etc/mysql/conf.d/mysql.cnf:ro", ver
                    ));
                    lines.push(format!(
                        "      - ${{MYSQL{}_DATA_DIR}}:/var/lib/mysql/:rw", ver
                    ));
                    lines.push(format!(
                        "      - ${{MYSQL{}_LOG_DIR}}:/var/log/mysql/:rw", ver
                    ));
                    lines.push(format!("    restart: always"));
                    lines.push(format!("    environment:"));
                    lines.push(format!(
                        "      MYSQL_ROOT_PASSWORD: \"${{MYSQL_ROOT_PASSWORD}}\""
                    ));
                    lines.push(format!("      TZ: \"${{TZ}}\""));
                    lines.push(format!("    networks:"));
                    lines.push(format!("      - php-stack-network"));
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
                    
                    lines.push(format!("  redis{}:", ver));
                    lines.push(format!("    image: redis:${{REDIS{}_VERSION}}", ver));
                    lines.push(format!("    container_name: ps-redis{}", ver));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{REDIS{}_HOST_PORT}}:6379\"", ver));
                    lines.push(format!("    volumes:"));
                    lines.push(format!(
                        "      - ${{REDIS{}_CONF_FILE}}:/etc/redis.conf:ro", ver
                    ));
                    lines.push(format!(
                        "      - ${{REDIS{}_DATA_DIR}}:/data/:rw", ver
                    ));
                    lines.push(format!("    restart: always"));
                    lines.push(format!("    entrypoint: [\"redis-server\", \"/etc/redis.conf\"]"));
                    lines.push(format!("    networks:"));
                    lines.push(format!("      - php-stack-network"));
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
                    
                    lines.push(format!("  nginx{}:", ver));
                    lines.push(format!("    build:"));
                    lines.push(format!("      context: ${{NGINX{}_BUILD_CONTEXT}}", ver));
                    lines.push(format!("    container_name: ps-nginx{}", ver));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{NGINX{}_HTTP_HOST_PORT}}:80\"", ver));
                    lines.push(format!("    volumes:"));
                    lines.push(format!("      - ${{SOURCE_DIR}}:/www/:rw"));
                    lines.push(format!(
                        "      - ${{NGINX{}_CONF_FILE}}:/etc/nginx/nginx.conf", ver
                    ));
                    lines.push(format!(
                        "      - ${{NGINX{}_CONFD_DIR}}:/etc/nginx/conf.d", ver
                    ));
                    lines.push(format!(
                        "      - ${{NGINX_LOG_DIR}}:/var/log/nginx"
                    ));
                    lines.push(format!("    restart: always"));
                    lines.push(format!("    networks:"));
                    lines.push(format!("      - php-stack-network"));
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
            .map_err(|e| format!("获取程序路径失败: {}", e))?
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
            return Err(format!("模板文件不存在: {:?}", template_path));
        }
        
        // Create destination directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }
        
        // Skip if destination exists and is identical to template
        if dest_path.exists() {
            use std::io::Read;
            let mut template_file = std::fs::File::open(&template_path)
                .map_err(|e| format!("打开模板文件失败: {}", e))?;
            let mut dest_file = std::fs::File::open(dest_path)
                .map_err(|e| format!("打开目标文件失败: {}", e))?;
            
            let mut template_buf = Vec::new();
            let mut dest_buf = Vec::new();
            
            template_file.read_to_end(&mut template_buf)
                .map_err(|e| format!("读取模板文件失败: {}", e))?;
            dest_file.read_to_end(&mut dest_buf)
                .map_err(|e| format!("读取目标文件失败: {}", e))?;
            
            if template_buf == dest_buf {
                // Files are identical, skip copy
                return Ok(());
            }
        }
        
        // Copy file (will overwrite if different)
        std::fs::copy(&template_path, dest_path)
            .map_err(|e| format!("复制文件 {:?} 到 {:?} 失败: {}", template_path, dest_path, e))?;
        
        Ok(())
    }

    /// Create services/, data/, logs/ directory structure.
    pub fn generate_service_dirs(config: &EnvConfig, root: &Path) -> Result<(), String> {
        // Create top-level directories
        std::fs::create_dir_all(root.join("services"))
            .map_err(|e| format!("创建 services/ 目录失败: {}", e))?;
        std::fs::create_dir_all(root.join("data"))
            .map_err(|e| format!("创建 data/ 目录失败: {}", e))?;
        std::fs::create_dir_all(root.join("logs"))
            .map_err(|e| format!("创建 logs/ 目录失败: {}", e))?;

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    let service_dir = root.join(format!("services/php{}", ver));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/php{}/ 目录失败: {}", ver, e))?;

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
                    } else if service.version.starts_with("8.5") {
                        "php85/Dockerfile"
                    } else {
                        "php85/Dockerfile"  // Default to latest for unknown versions
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
                    } else if service.version.starts_with("8.5") {
                        "php85/php.ini"
                    } else {
                        "php85/php.ini"
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
                    } else if service.version.starts_with("8.5") {
                        "php85/php-fpm.conf"
                    } else {
                        "php85/php-fpm.conf"
                    };
                    Self::copy_template_file(
                        fpm_conf_template,
                        &service_dir.join("php-fpm.conf"),
                    )?;

                    // Create log directory
                    std::fs::create_dir_all(root.join(format!("logs/php{}", ver)))
                        .map_err(|e| format!("创建 logs/php{}/ 目录失败: {}", ver, e))?;
                }
                ServiceType::MySQL => {
                    // Generate service directory name: mysql{major}{minor}
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("mysql{}{}", version_parts[0], version_parts[1])
                    } else {
                        "mysql80".to_string()
                    };
                    
                    let service_dir = root.join(format!("services/{}", service_dir_name));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{}/ 目录失败: {}", service_dir_name, e))?;

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
                    std::fs::create_dir_all(root.join(format!("data/{}", service_dir_name)))
                        .map_err(|e| format!("创建 data/{}/ 目录失败: {}", service_dir_name, e))?;
                    std::fs::create_dir_all(root.join(format!("logs/{}", service_dir_name)))
                        .map_err(|e| format!("创建 logs/{}/ 目录失败: {}", service_dir_name, e))?;
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
                    
                    let service_dir = root.join(format!("services/{}", service_dir_name));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{}/ 目录失败: {}", service_dir_name, e))?;

                    // Copy redis.conf from template
                    // Use redis72 as base template for all versions
                    Self::copy_template_file(
                        "redis72/redis.conf",
                        &service_dir.join("redis.conf"),
                    )?;

                    // Create data directory
                    std::fs::create_dir_all(root.join(format!("data/{}", service_dir_name)))
                        .map_err(|e| format!("创建 data/{}/ 目录失败: {}", service_dir_name, e))?;
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
                    
                    let service_dir = root.join(format!("services/{}", service_dir_name));
                    std::fs::create_dir_all(&service_dir)
                        .map_err(|e| format!("创建 services/{}/ 目录失败: {}", service_dir_name, e))?;
                    std::fs::create_dir_all(root.join(format!("services/{}/conf.d", service_dir_name)))
                        .map_err(|e| format!("创建 services/{}/conf.d/ 目录失败: {}", service_dir_name, e))?;

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
                        .map_err(|e| format!("创建 logs/nginx/ 目录失败: {}", e))?;
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
                existing_items.push(item.to_string());
            }
        }
        
        if existing_items.is_empty() {
            return Ok(BackupState::NothingToBackup);
        }
        
        // Pre-check: verify all backup target paths don't exist (avoid overwriting old backups)
        for item in &existing_items {
            let backup_name = format!("{}_{}", item, timestamp);
            let backup_path = project_root.join(&backup_name);
            
            if backup_path.exists() {
                return Err(format!("备份文件已存在，请删除后重试: {}", backup_name));
            }
        }
        
        Ok(BackupState::Ready { 
            timestamp, 
            items: existing_items 
        })
    }

    /// Rollback all backed up items in reverse order
    fn rollback_all(rollback_list: &[(PathBuf, PathBuf)]) -> Result<(), String> {
        // Reverse order to ensure dependencies are restored correctly
        for (backup_path, original_path) in rollback_list.iter().rev() {
            if let Err(e) = std::fs::rename(backup_path, original_path) {
                eprintln!("⚠️  回滚失败 {:?} -> {:?}: {}", backup_path, original_path, e);
                return Err(format!("回滚失败: {}", e));
            }
        }
        Ok(())
    }

    /// Phase 2: Execute backup with atomic rollback
    /// If any item fails to backup, all previously backed up items will be rolled back
    fn execute_backup(state: BackupState, project_root: &Path) -> Result<Vec<String>, String> {
        match state {
            BackupState::NothingToBackup => Ok(vec![]),
            BackupState::Ready { timestamp, items } => {
                let mut backed_up = Vec::new();
                let mut rollback_list: Vec<(PathBuf, PathBuf)> = Vec::new();
                
                // Try to backup each item
                for item in &items {
                    let item_path = project_root.join(item);
                    let backup_name = format!("{}_{}", item, timestamp);
                    let backup_path = project_root.join(&backup_name);
                    
                    match std::fs::rename(&item_path, &backup_path) {
                        Ok(()) => {
                            eprintln!("✅ 已备份: {} -> {}", item, backup_name);
                            backed_up.push(backup_name.clone());
                            rollback_list.push((backup_path, item_path.clone()));
                        }
                        Err(e) => {
                            eprintln!("❌ 备份 {} 失败: {}", item, e);
                            
                            // Immediate rollback on failure
                            if !rollback_list.is_empty() {
                                eprintln!("🔄 正在回滚已备份的 {} 项...", rollback_list.len());
                                if let Err(rollback_err) = Self::rollback_all(&rollback_list) {
                                    eprintln!("⚠️  严重错误：回滚也失败: {}", rollback_err);
                                    return Err(format!(
                                        "备份 {} 失败，且回滚操作也失败（请手动恢复）: {}\n回滚错误: {}",
                                        item, e, rollback_err
                                    ));
                                }
                                eprintln!("✅ 回滚成功，所有文件已恢复原状");
                            }
                            
                            return Err(format!(
                                "备份 {} 失败，已自动回滚之前的操作 {} ",
                                item, e
                            ));
                        }
                    }
                }
                
                eprintln!("✅ 备份完成，共 {} 项", backed_up.len());
                Ok(backed_up)
            }
        }
    }

    /// Backup existing configuration files by renaming them with timestamp suffix.
    /// This function implements atomic backup with automatic rollback on failure.
    /// Format: .env -> .env_YYYYMMDD_HHMMSS
    ///         services/ -> services_YYYYMMDD_HHMMSS/
    pub fn backup_existing_config(project_root: &Path) -> Result<Vec<String>, String> {
        eprintln!("🔍 开始预检查备份...");
        
        // Phase 1: Pre-check
        let backup_state = Self::precheck_backup(project_root)?;
        
        // Phase 2: Execute with rollback
        eprintln!("📦 执行备份...");
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
                .map_err(|e| format!("读取 .env 文件失败: {}", e))?;
            Some(
                EnvFile::parse(&content)
                    .map_err(|e| format!("解析 .env 文件失败: {}", e))?,
            )
        } else {
            None
        };

        // Generate and write .env
        let env_file = Self::generate_env(config, existing_env.as_ref());
        std::fs::write(&env_path, env_file.format())
            .map_err(|e| format!("写入 .env 文件失败: {}", e))?;

        // Generate and write docker-compose.yml
        let compose = Self::generate_compose(config);
        std::fs::write(project_root.join("docker-compose.yml"), compose)
            .map_err(|e| format!("写入 docker-compose.yml 失败: {}", e))?;

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
            
            let npmrc_content = format!("registry={}\n", npm_mirror);
            let npmrc_path = workspace_path.join(".npmrc");
            std::fs::write(&npmrc_path, npmrc_content)
                .map_err(|e| format!("写入 .npmrc 文件失败: {}", e))?;
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

        for service in &config.services {
            match &service.service_type {
                ServiceType::PHP => {
                    let ver = service.version.replace('.', "");
                    keys.insert(format!("PHP{}_VERSION", ver));
                    keys.insert(format!("PHP{}_HOST_PORT", ver));
                    keys.insert(format!("PHP{}_EXTENSIONS", ver));
                    keys.insert(format!("PHP{}_PHP_CONF_FILE", ver));
                    keys.insert(format!("PHP{}_FPM_CONF_FILE", ver));
                    keys.insert(format!("PHP{}_LOG_DIR", ver));
                }
                ServiceType::MySQL => {
                    keys.insert("MYSQL_VERSION".to_string());
                    keys.insert("MYSQL_HOST_PORT".to_string());
                    keys.insert("MYSQL_ROOT_PASSWORD".to_string());
                    keys.insert("MYSQL_CONF_FILE".to_string());
                    keys.insert("MYSQL_LOG_DIR".to_string());
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
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "8.2");
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
        // MySQL 8.0 uses tag "8.0" from version_manifest.json
        assert_eq!(map.get("MYSQL80_VERSION").unwrap(), "8.0");
        assert_eq!(map.get("MYSQL80_HOST_PORT").unwrap(), "3306");
        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
        // Redis 7.0 uses tag "7.0-alpine" from version_manifest.json
        assert_eq!(map.get("REDIS70_VERSION").unwrap(), "7.0-alpine");
        assert_eq!(map.get("REDIS70_HOST_PORT").unwrap(), "6379");
        // Nginx 1.25 uses tag "1.25-alpine" from version_manifest.json
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "1.25-alpine");
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
        };

        let env = ConfigGenerator::generate_env(&config, Some(&existing_env));
        let map = env.to_map();

        // Custom variable preserved
        assert_eq!(map.get("CUSTOM_VAR").unwrap(), "hello");
        // Managed variable updated
        assert_eq!(map.get("SOURCE_DIR").unwrap(), "./www");
        // New managed variable added (uses tag from version_manifest.json)
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "1.25-alpine");
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
        };

        let env = ConfigGenerator::generate_env(&config, None);
        let map = env.to_map();

        // PHP 7.4 vars
        assert_eq!(map.get("PHP74_VERSION").unwrap(), "7.4");
        assert_eq!(map.get("PHP74_HOST_PORT").unwrap(), "9074");
        assert_eq!(map.get("PHP74_EXTENSIONS").unwrap(), "pdo_mysql");

        // PHP 8.2 vars
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "8.2");
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
}
