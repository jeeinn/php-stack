use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::env_parser::EnvFile;

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

impl ConfigGenerator {
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
                    // Generate service directory name: mysql{major}{minor}
                    // e.g., MySQL 5.7 -> mysql57, MySQL 8.0 -> mysql80
                    let version_parts: Vec<&str> = service.version.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("mysql{}{}", version_parts[0], version_parts[1])
                    } else {
                        "mysql80".to_string() // Default to mysql80 for unknown versions
                    };
                    
                    env.set("MYSQL_VERSION", &service.version);
                    env.set("MYSQL_HOST_PORT", &service.host_port.to_string());
                    env.set("MYSQL_ROOT_PASSWORD", "root");
                    env.set("MYSQL_CONF_FILE", &format!("./services/{}/mysql.cnf", service_dir_name));
                    env.set("MYSQL_LOG_DIR", &format!("./logs/{}", service_dir_name));
                }
                ServiceType::Redis => {
                    // Generate service directory name: redis{major}{minor}
                    // e.g., Redis 6.2-alpine -> redis62, Redis 7.0-alpine -> redis70
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("redis{}{}", version_parts[0], version_parts[1])
                    } else {
                        "redis72".to_string() // Default to redis72 for unknown versions
                    };
                    
                    env.set("REDIS_VERSION", &service.version);
                    env.set("REDIS_HOST_PORT", &service.host_port.to_string());
                    env.set("REDIS_CONF_FILE", &format!("./services/{}/redis.conf", service_dir_name));
                }
                ServiceType::Nginx => {
                    // Generate service directory name: nginx{major}{minor}
                    // e.g., Nginx 1.24-alpine -> nginx124, Nginx 1.27-alpine -> nginx127
                    let version_base = service.version.split('-').next().unwrap_or(&service.version);
                    let version_parts: Vec<&str> = version_base.split('.').collect();
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("nginx{}{}", version_parts[0], version_parts[1])
                    } else {
                        "nginx127".to_string() // Default to nginx127 for unknown versions
                    };
                    
                    env.set("NGINX_VERSION", &service.version);
                    env.set("NGINX_HTTP_HOST_PORT", &service.host_port.to_string());
                    env.set("NGINX_CONF_FILE", &format!("./services/{}/nginx.conf", service_dir_name));
                    env.set("NGINX_CONFD_DIR", &format!("./services/{}/conf.d", service_dir_name));
                    env.set("NGINX_LOG_DIR", "./logs/nginx");
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
        lines.push("version: \"3\"".to_string());
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
                    // 镜像源配置
                    lines.push(format!("        DEBIAN_MIRROR_DOMAIN: \"${{APT_MIRROR:-deb.debian.org}}\""));
                    lines.push(format!("        COMPOSER_MIRROR: \"${{COMPOSER_MIRROR:-https://packagist.org}}\""));
                    lines.push(format!("        NPM_REGISTRY: \"${{NPM_MIRROR:-https://registry.npmjs.org}}\""));
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
                    let service_dir_name = if version_parts.len() >= 2 {
                        format!("mysql{}{}", version_parts[0], version_parts[1])
                    } else {
                        "mysql80".to_string()
                    };
                    
                    lines.push(format!("  mysql:"));
                    lines.push(format!("    image: mysql:${{MYSQL_VERSION}}"));
                    lines.push(format!("    container_name: ps-mysql"));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{MYSQL_HOST_PORT}}:3306\""));
                    lines.push(format!("    volumes:"));
                    lines.push(format!(
                        "      - ${{MYSQL_CONF_FILE}}:/etc/mysql/conf.d/mysql.cnf:ro"
                    ));
                    lines.push(format!(
                        "      - ${{DATA_DIR}}/{}:/var/lib/mysql/:rw", service_dir_name
                    ));
                    lines.push(format!(
                        "      - ${{MYSQL_LOG_DIR}}:/var/log/mysql/:rw"
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
                    lines.push(format!("  redis:"));
                    lines.push(format!("    image: redis:${{REDIS_VERSION}}"));
                    lines.push(format!("    container_name: ps-redis"));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{REDIS_HOST_PORT}}:6379\""));
                    lines.push(format!("    volumes:"));
                    lines.push(format!(
                        "      - ${{REDIS_CONF_FILE}}:/etc/redis.conf:ro"
                    ));
                    lines.push(format!(
                        "      - ${{DATA_DIR}}/redis:/data/:rw"
                    ));
                    lines.push(format!("    restart: always"));
                    lines.push(format!("    entrypoint: [\"redis-server\", \"/etc/redis.conf\"]"));
                    lines.push(format!("    networks:"));
                    lines.push(format!("      - php-stack-network"));
                    lines.push(String::new());
                }
                ServiceType::Nginx => {
                    lines.push(format!("  nginx:"));
                    lines.push(format!("    build:"));
                    lines.push(format!("      context: ./services/nginx"));
                    lines.push(format!("    container_name: ps-nginx"));
                    lines.push(format!("    ports:"));
                    lines.push(format!("      - \"${{NGINX_HTTP_HOST_PORT}}:80\""));
                    lines.push(format!("    volumes:"));
                    lines.push(format!("      - ${{SOURCE_DIR}}:/www/:rw"));
                    lines.push(format!(
                        "      - ${{NGINX_CONF_FILE}}:/etc/nginx/nginx.conf"
                    ));
                    lines.push(format!(
                        "      - ${{NGINX_CONFD_DIR}}:/etc/nginx/conf.d"
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

    /// Apply config: write .env, docker-compose.yml, create directories.
    pub async fn apply(config: &EnvConfig, project_root: &Path) -> Result<(), String> {
        // Validate first
        Self::validate(config)?;

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
        
        // Generate .npmrc file if NPM mirror is configured
        let npm_mirror = env_file.get("NPM_MIRROR").unwrap_or("");
        if !npm_mirror.is_empty() && npm_mirror != "https://registry.npmjs.org" {
            let npmrc_content = format!("registry={}\n", npm_mirror);
            let npmrc_path = project_root.join(".npmrc");
            std::fs::write(&npmrc_path, npmrc_content)
                .map_err(|e| format!("写入 .npmrc 文件失败: {}", e))?;
        }

        Ok(())
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
        assert_eq!(map.get("MYSQL_VERSION").unwrap(), "8.0");
        assert_eq!(map.get("MYSQL_HOST_PORT").unwrap(), "3306");
        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
        assert_eq!(map.get("REDIS_VERSION").unwrap(), "7.0");
        assert_eq!(map.get("REDIS_HOST_PORT").unwrap(), "6379");
        assert_eq!(map.get("NGINX_VERSION").unwrap(), "1.25");
        assert_eq!(map.get("NGINX_HTTP_HOST_PORT").unwrap(), "80");
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
        // New managed variable added
        assert_eq!(map.get("NGINX_VERSION").unwrap(), "1.25");
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
        assert!(compose.contains("${MYSQL_VERSION}"));
        assert!(compose.contains("${MYSQL_HOST_PORT}"));
        assert!(compose.contains("${REDIS_VERSION}"));
        assert!(compose.contains("${REDIS_HOST_PORT}"));
        assert!(compose.contains("${NGINX_HTTP_HOST_PORT}"));
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
