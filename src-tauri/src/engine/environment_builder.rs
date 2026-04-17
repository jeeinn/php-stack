/// 环境构建器模块
/// 
/// V1.0: 向导式一键搭建开发环境
/// 支持 PHP + MySQL + Redis + Nginx 的快速部署

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::engine::software_manager::SoftwareType;

/// 环境规格（用户输入）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub services: Vec<ServiceSpec>,
    pub network_name: String,
}

/// 单个服务规格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    pub software_type: SoftwareType,
    pub version: String,
    pub ports: HashMap<u16, u16>,
    pub extensions: Option<Vec<String>>,  // PHP 扩展
}

/// 部署结果
#[derive(Debug, Serialize)]
pub struct DeploymentResult {
    pub success: bool,
    pub services: Vec<ServiceStatus>,
    pub logs: Vec<String>,
}

/// 服务状态
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub name: String,
    pub status: String,  // "running", "failed", "pending"
}

/// 版本兼容性检查器
pub struct CompatibilityChecker;

impl CompatibilityChecker {
    /// 验证环境规格的兼容性
    pub fn validate(spec: &EnvironmentSpec) -> Result<(), String> {
        // 1. 检查至少有一个服务
        if spec.services.is_empty() {
            return Err("至少需要选择一个服务".to_string());
        }
        
        // 2. 检查端口冲突
        let mut used_ports = HashMap::new();
        for service in &spec.services {
            for (host_port, _container_port) in &service.ports {
                if let Some(existing_service) = used_ports.get(host_port) {
                    return Err(format!(
                        "端口 {} 被 {} 和 {} 同时使用",
                        host_port, existing_service, Self::service_type_name(&service.software_type)
                    ));
                }
                used_ports.insert(host_port, Self::service_type_name(&service.software_type));
            }
        }
        
        // 3. 检查 PHP 扩展兼容性
        for service in &spec.services {
            if service.software_type == SoftwareType::PHP {
                if let Some(extensions) = &service.extensions {
                    for ext in extensions {
                        if !Self::is_valid_php_extension(ext) {
                            return Err(format!("不支持的 PHP 扩展: {}", ext));
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 获取服务类型的显示名称
    fn service_type_name(service_type: &SoftwareType) -> String {
        match service_type {
            SoftwareType::PHP => "PHP".to_string(),
            SoftwareType::MySQL => "MySQL".to_string(),
            SoftwareType::Redis => "Redis".to_string(),
            SoftwareType::Nginx => "Nginx".to_string(),
            _ => format!("{:?}", service_type),
        }
    }
    
    /// 检查 PHP 扩展是否有效
    fn is_valid_php_extension(ext: &str) -> bool {
        let valid_extensions = [
            "mysqli", "pdo_mysql", "redis", "gd", "mbstring", "curl",
            "zip", "intl", "opcache", "bcmath", "soap", "xml",
            "pdo", "json", "ctype", "session",
        ];
        valid_extensions.contains(&ext)
    }
}

/// 环境构建器
pub struct EnvironmentBuilder {
    pub compose_manager: crate::engine::compose_manager::ComposeManager,
}

impl EnvironmentBuilder {
    pub fn new() -> Self {
        Self {
            compose_manager: crate::engine::compose_manager::ComposeManager::new("."),
        }
    }
    
    /// 生成优化的 docker-compose 配置（包含镜像构建）
    pub async fn generate_compose_with_build(
        &self,
        spec: &EnvironmentSpec,
    ) -> Result<crate::engine::compose_manager::DockerCompose, String> {
        use crate::engine::compose_manager::{DockerCompose, NetworkConfig};
        
        let mut compose = DockerCompose {
            version: None,  // Docker Compose v2+ 不再需要
            networks: HashMap::from([
                (spec.network_name.clone(), NetworkConfig {
                    driver: "bridge".to_string(),
                    external: Some(true),
                })
            ]),
            services: HashMap::new(),
            volumes: None,
        };
        
        // 为每个服务生成配置
        for service_spec in &spec.services {
            let service_config = self.build_service_config_with_image_build(service_spec, &spec.network_name).await?;
            let service_name = Self::get_service_name(service_spec);
            compose.services.insert(service_name, service_config);
        }
        
        Ok(compose)
    }
    
    /// 生成优化的 docker-compose 配置（不包含镜像构建，用于预览）
    pub async fn generate_compose(
        &self,
        spec: &EnvironmentSpec,
    ) -> Result<crate::engine::compose_manager::DockerCompose, String> {
        use crate::engine::compose_manager::{DockerCompose, NetworkConfig};
        
        let mut compose = DockerCompose {
            version: None,  // Docker Compose v2+ 不再需要
            networks: HashMap::from([
                (spec.network_name.clone(), NetworkConfig {
                    driver: "bridge".to_string(),
                    external: Some(true),
                })
            ]),
            services: HashMap::new(),
            volumes: None,
        };
        
        // 为每个服务生成配置
        for service_spec in &spec.services {
            let service_config = self.build_service_config(service_spec, &spec.network_name)?;
            let service_name = Self::get_service_name(service_spec);
            compose.services.insert(service_name, service_config);
        }
        
        Ok(compose)
    }
    
    /// 构建单个服务的配置（包含镜像构建）
    async fn build_service_config_with_image_build(
        &self,
        spec: &ServiceSpec,
        network_name: &str,
    ) -> Result<crate::engine::compose_manager::ServiceConfig, String> {
        use crate::engine::compose_manager::ServiceConfig;
        
        let service_name = Self::get_service_name(spec);
        
        // 如果是 PHP 且有扩展，先构建自定义镜像
        let image = if spec.software_type == SoftwareType::PHP {
            if let Some(extensions) = &spec.extensions {
                if !extensions.is_empty() {
                    log::info!("🔨 构建 PHP 自定义镜像 (版本: {}, 扩展: {:?})", spec.version, extensions);
                    build_custom_php_image(&spec.version, extensions).await?
                } else {
                    format!("php:{}-fpm", spec.version)
                }
            } else {
                format!("php:{}-fpm", spec.version)
            }
        } else {
            Self::get_image_name(spec)
        };
        
        // 端口映射
        let ports: Vec<String> = spec.ports.iter()
            .map(|(host, container)| format!("{}:{}", host, container))
            .collect();
        
        // 环境变量
        let environment = Self::get_environment_vars(spec);
        
        // 数据卷
        let volumes = Self::get_volumes(spec);
        
        // 依赖关系
        let depends_on = Self::get_dependencies(spec);
        
        Ok(ServiceConfig {
            image,
            container_name: service_name.clone(),
            networks: vec![network_name.to_string()],
            ports: if ports.is_empty() { None } else { Some(ports) },
            volumes: if volumes.as_ref().map_or(true, |v| v.is_empty()) {
                None
            } else {
                volumes
            },
            environment: if environment.as_ref().map_or(true, |e| e.is_empty()) {
                None
            } else {
                environment
            },
            depends_on: if depends_on.as_ref().map_or(true, |d| d.is_empty()) {
                None
            } else {
                depends_on
            },
            restart: Some("unless-stopped".to_string()),
            working_dir: None,
        })
    }
    
    /// 构建单个服务的配置
    fn build_service_config(
        &self,
        spec: &ServiceSpec,
        network_name: &str,
    ) -> Result<crate::engine::compose_manager::ServiceConfig, String> {
        use crate::engine::compose_manager::ServiceConfig;
        
        let service_name = Self::get_service_name(spec);
        let image = Self::get_image_name(spec);
        
        // 端口映射
        let ports: Vec<String> = spec.ports.iter()
            .map(|(host, container)| format!("{}:{}", host, container))
            .collect();
        
        // 环境变量
        let environment = Self::get_environment_vars(spec);
        
        // 数据卷
        let volumes = Self::get_volumes(spec);
        
        // 依赖关系
        let depends_on = Self::get_dependencies(spec);
        
        Ok(ServiceConfig {
            image,
            container_name: service_name.clone(),
            networks: vec![network_name.to_string()],
            ports: if ports.is_empty() { None } else { Some(ports) },
            volumes: if volumes.as_ref().map_or(true, |v| v.is_empty()) {
                None
            } else {
                volumes
            },
            environment: if environment.as_ref().map_or(true, |e| e.is_empty()) {
                None
            } else {
                environment
            },
            depends_on: if depends_on.as_ref().map_or(true, |d| d.is_empty()) {
                None
            } else {
                depends_on
            },
            restart: Some("unless-stopped".to_string()),
            working_dir: None,
        })
    }
    
    /// 获取服务名称
    fn get_service_name(spec: &ServiceSpec) -> String {
        let type_prefix = match spec.software_type {
            SoftwareType::PHP => "php",
            SoftwareType::MySQL => "mysql",
            SoftwareType::Redis => "redis",
            SoftwareType::Nginx => "nginx",
            _ => "unknown",
        };
        
        let version_dashed = spec.version.replace('.', "-");
        format!("ps-{}-{}", type_prefix, version_dashed)
    }
    
    /// 获取镜像名称
    fn get_image_name(spec: &ServiceSpec) -> String {
        match spec.software_type {
            SoftwareType::PHP => {
                // 如果有自定义扩展，需要构建自定义镜像
                if let Some(extensions) = &spec.extensions {
                    if !extensions.is_empty() {
                        // TODO: 实现自定义镜像构建
                        // 暂时使用官方镜像
                        return format!("php:{}-fpm", spec.version);
                    }
                }
                format!("php:{}-fpm", spec.version)
            }
            SoftwareType::MySQL => format!("mysql:{}", spec.version),
            SoftwareType::Redis => format!("redis:{}", spec.version),
            SoftwareType::Nginx => format!("nginx:{}", spec.version),
            _ => format!("unknown:latest"),
        }
    }
    
    /// 获取环境变量
    fn get_environment_vars(spec: &ServiceSpec) -> Option<HashMap<String, String>> {
        match spec.software_type {
            SoftwareType::MySQL => {
                // V1.0: 固定密码 root123
                Some(HashMap::from([
                    ("MYSQL_ROOT_PASSWORD".to_string(), "root123".to_string()),
                    ("MYSQL_DATABASE".to_string(), "app".to_string()),
                ]))
            }
            _ => None,
        }
    }
    
    /// 获取数据卷配置（参考 dnmp 项目）
    fn get_volumes(spec: &ServiceSpec) -> Option<Vec<String>> {
        let volume_path = match spec.software_type {
            SoftwareType::MySQL => "./data/mysql:/var/lib/mysql",
            SoftwareType::Redis => "./data/redis:/data",
            SoftwareType::Nginx => {
                // Nginx 需要挂载 www 目录和配置文件
                return Some(vec![
                    "./data/www:/var/www/html".to_string(),
                    "./nginx/conf.d:/etc/nginx/conf.d".to_string(),
                ]);
            }
            SoftwareType::PHP => {
                // PHP 也需要挂载 www 目录
                return Some(vec![
                    "./data/www:/var/www/html".to_string(),
                ]);
            }
            _ => return None,
        };
        
        Some(vec![volume_path.to_string()])
    }
    
    /// 获取依赖关系（参考 dnmp 项目）
    fn get_dependencies(spec: &ServiceSpec) -> Option<Vec<String>> {
        match spec.software_type {
            SoftwareType::Nginx => {
                // Nginx 必须依赖 PHP-FPM（参考 dnmp 项目）
                Some(vec!["php".to_string()])
            }
            _ => None,  // V1.0: 简化依赖，PHP不强制依赖MySQL/Redis
        }
    }
}

// ==================== PHP 扩展注册表 ====================

/// PHP 扩展类型
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionType {
    Builtin,  // 内置扩展（直接启用）
    Core,     // 核心扩展（docker-php-ext-install）
    Pecl,     // PECL 扩展（pecl install）
}

/// 扩展元数据
#[derive(Debug, Clone)]
pub struct ExtensionMetadata {
    pub name: String,
    pub ext_type: ExtensionType,
    pub system_deps: Vec<String>,      // 系统依赖包
    pub pecl_version: Option<String>,  // PECL 版本号
    pub config_flags: Vec<String>,     // 编译参数
}

/// PHP 扩展注册表
pub struct ExtensionRegistry;

impl ExtensionRegistry {
    /// 获取扩展元数据
    pub fn get_extension(name: &str) -> Option<ExtensionMetadata> {
        match name {
            // 内置扩展
            "pdo" | "json" | "ctype" | "session" => Some(ExtensionMetadata {
                name: name.to_string(),
                ext_type: ExtensionType::Builtin,
                system_deps: vec![],
                pecl_version: None,
                config_flags: vec![],
            }),
            
            // 核心扩展
            "mysqli" | "pdo_mysql" => Some(ExtensionMetadata {
                name: name.to_string(),
                ext_type: ExtensionType::Core,
                system_deps: vec!["libmariadb-dev".to_string()],
                pecl_version: None,
                config_flags: vec![],
            }),
            "gd" => Some(ExtensionMetadata {
                name: "gd".to_string(),
                ext_type: ExtensionType::Core,
                system_deps: vec![
                    "libpng-dev".to_string(),
                    "libjpeg-dev".to_string(),
                    "libfreetype6-dev".to_string(),
                ],
                pecl_version: None,
                config_flags: vec![
                    "--with-freetype".to_string(),
                    "--with-jpeg".to_string(),
                ],
            }),
            "mbstring" | "curl" | "zip" | "intl" | "bcmath" | "soap" | "xml" | "opcache" => {
                Some(ExtensionMetadata {
                    name: name.to_string(),
                    ext_type: ExtensionType::Core,
                    system_deps: vec![],
                    pecl_version: None,
                    config_flags: vec![],
                })
            },
            
            // PECL 扩展
            "redis" => Some(ExtensionMetadata {
                name: "redis".to_string(),
                ext_type: ExtensionType::Pecl,
                system_deps: vec![],
                pecl_version: Some("6.0.2".to_string()),  // PHP 8.2+ 需要 6.0+
                config_flags: vec![],
            }),
            "xdebug" => Some(ExtensionMetadata {
                name: "xdebug".to_string(),
                ext_type: ExtensionType::Pecl,
                system_deps: vec![],
                pecl_version: Some("3.3.0".to_string()),
                config_flags: vec![],
            }),
            
            _ => None,
        }
    }
    
    /// 检查扩展是否有效
    pub fn is_valid_extension(name: &str) -> bool {
        Self::get_extension(name).is_some()
    }
}

// ==================== Dockerfile 生成器 ====================

/// 生成自定义 PHP Dockerfile
pub fn generate_php_dockerfile(
    php_version: &str,
    extensions: &[String],
) -> Result<String, String> {
    use crate::engine::mirror_config::MirrorConfig;
    
    // 加载镜像源配置
    let mirror_config = MirrorConfig::load_from_env()?;
    
    let mut dockerfile = format!("FROM php:{}-fpm\n\n", php_version);
    
    // 添加镜像源配置
    let mirror_snippet = mirror_config.to_dockerfile_snippet();
    if !mirror_snippet.is_empty() {
        dockerfile.push_str(&mirror_snippet);
    }
    
    // 收集所有需要的系统依赖和扩展
    let mut all_system_deps = Vec::new();
    let mut core_extensions = Vec::new();
    let mut pecl_extensions = Vec::new();
    
    for ext_name in extensions {
        if let Some(metadata) = ExtensionRegistry::get_extension(ext_name) {
            match metadata.ext_type {
                ExtensionType::Builtin => {
                    // 内置扩展无需安装
                    log::info!("✅ 内置扩展: {}", ext_name);
                }
                ExtensionType::Core => {
                    all_system_deps.extend(metadata.system_deps);
                    core_extensions.push((ext_name.clone(), metadata.config_flags));
                }
                ExtensionType::Pecl => {
                    all_system_deps.extend(metadata.system_deps);
                    pecl_extensions.push((
                        ext_name.clone(),
                        metadata.pecl_version,
                    ));
                }
            }
        } else {
            return Err(format!("未知或不支持的 PHP 扩展: {}", ext_name));
        }
    }
    
    // 去重并排序系统依赖
    if !all_system_deps.is_empty() {
        all_system_deps.sort();
        all_system_deps.dedup();
        
        dockerfile.push_str("# 安装系统依赖\n");
        dockerfile.push_str("RUN apt-get update && apt-get install -y --no-install-recommends \\\n");
        for (i, dep) in all_system_deps.iter().enumerate() {
            if i < all_system_deps.len() - 1 {
                dockerfile.push_str(&format!("    {} \\\n", dep));
            } else {
                dockerfile.push_str(&format!("    {} && \\\n", dep));
            }
        }
        dockerfile.push_str("    rm -rf /var/lib/apt/lists/*\n\n");
    }
    
    // 安装核心扩展
    if !core_extensions.is_empty() {
        dockerfile.push_str("# 安装 PHP 核心扩展\n");
        for (ext_name, config_flags) in &core_extensions {
            if config_flags.is_empty() {
                dockerfile.push_str(&format!(
                    "RUN docker-php-ext-install -j$(nproc) {}\n",
                    ext_name
                ));
            } else {
                dockerfile.push_str(&format!(
                    "RUN docker-php-ext-configure {} {} && \\\n    docker-php-ext-install -j$(nproc) {}\n",
                    ext_name,
                    config_flags.join(" "),
                    ext_name
                ));
            }
        }
        dockerfile.push('\n');
    }
    
    // 安装 PECL 扩展
    if !pecl_extensions.is_empty() {
        dockerfile.push_str("# 安装 PECL 扩展\n");
        for (ext_name, version) in &pecl_extensions {
            if let Some(ver) = version {
                dockerfile.push_str(&format!(
                    "RUN pecl install {}-{} && docker-php-ext-enable {}\n",
                    ext_name, ver, ext_name
                ));
            } else {
                dockerfile.push_str(&format!(
                    "RUN pecl install {} && docker-php-ext-enable {}\n",
                    ext_name, ext_name
                ));
            }
        }
        dockerfile.push('\n');
    }
    
    // 设置工作目录
    dockerfile.push_str("WORKDIR /var/www/html\n");
    
    Ok(dockerfile)
}

// ==================== 镜像构建 ====================

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// 生成唯一的镜像标签
fn generate_image_tag(php_version: &str, extensions: &[String]) -> String {
    let mut sorted_exts = extensions.to_vec();
    sorted_exts.sort();
    
    // 使用扩展列表的哈希作为标签的一部分
    let hash = calculate_hash(&sorted_exts.join(","));
    let short_hash = &hash.to_string()[..8];
    
    format!("php:{}-custom-{}", php_version, short_hash)
}

/// 计算哈希值
fn calculate_hash(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

/// 检查镜像是否存在
async fn image_exists(image_tag: &str) -> bool {
    use tokio::process::Command;
    
    let output = Command::new("docker")
        .args(&["image", "inspect", image_tag])
        .output()
        .await;
    
    output.map_or(false, |o| o.status.success())
}

/// 构建自定义 PHP 镜像
pub async fn build_custom_php_image(
    php_version: &str,
    extensions: &[String],
) -> Result<String, String> {
    use tokio::process::Command;
    use crate::engine::mirror_config::MirrorConfig;
    
    // 加载镜像源配置
    let mirror_config = MirrorConfig::load_from_env()?;
    
    // 生成唯一的镜像标签
    let image_tag = generate_image_tag(php_version, extensions);
    
    // 检查镜像是否已存在（缓存命中）
    if image_exists(&image_tag).await {
        log::info!("✅ 使用缓存镜像: {}", image_tag);
        return Ok(image_tag);
    }
    
    // 生成 Dockerfile
    let dockerfile = generate_php_dockerfile(php_version, extensions)?;
    
    log::info!("📝 生成的 Dockerfile:\n{}", dockerfile);
    
    // 创建临时构建目录
    let build_dir = format!("./build/php-{}-custom", php_version);
    tokio::fs::create_dir_all(&build_dir).await
        .map_err(|e| format!("创建构建目录失败: {}", e))?;
    
    tokio::fs::write(format!("{}/Dockerfile", build_dir), &dockerfile).await
        .map_err(|e| format!("写入 Dockerfile 失败: {}", e))?;
    
    // 执行 docker build
    log::info!("🔨 开始构建镜像: {}", image_tag);
    log::info!("📦 扩展列表: {:?}", extensions);
    
    let start_time = std::time::Instant::now();
    
    // 准备构建参数
    let mut build_args = vec!["build"];
    
    // 添加代理配置
    let proxy_args = mirror_config.to_build_args();
    for arg in &proxy_args {
        build_args.push("--build-arg");
        build_args.push(arg);
    }
    
    build_args.extend_from_slice(&["-t", &image_tag, &build_dir]);
    
    let output = Command::new("docker")
        .args(&build_args)
        .output()
        .await
        .map_err(|e| format!("Docker 命令执行失败: {}", e))?;
    
    let elapsed = start_time.elapsed();
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // 分析错误类型
        if stderr.contains("timeout") || stderr.contains("network") {
            return Err(format!(
                "⚠️ 网络超时，请检查网络连接或切换镜像源\n\n错误详情:\n{}",
                stderr
            ));
        } else if stderr.contains("not found") {
            return Err(format!(
                "❌ 扩展不存在或版本不兼容\n\n错误详情:\n{}",
                stderr
            ));
        } else {
            return Err(format!(
                "❌ 镜像构建失败 (耗时: {:.2}s)\n\n错误详情:\n{}",
                elapsed.as_secs_f64(),
                stderr
            ));
        }
    }
    
    log::info!("✅ 镜像构建成功 (耗时: {:.2}s): {}", elapsed.as_secs_f64(), image_tag);
    
    Ok(image_tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_empty_services() {
        let spec = EnvironmentSpec {
            services: vec![],
            network_name: "test-network".to_string(),
        };
        
        assert!(CompatibilityChecker::validate(&spec).is_err());
    }
    
    #[test]
    fn test_validate_port_conflict() {
        let spec = EnvironmentSpec {
            services: vec![
                ServiceSpec {
                    software_type: SoftwareType::MySQL,
                    version: "8.0".to_string(),
                    ports: HashMap::from([(3306u16, 3306u16)]),
                    extensions: None,
                },
                ServiceSpec {
                    software_type: SoftwareType::Redis,
                    version: "7.0".to_string(),
                    ports: HashMap::from([(3306u16, 6379u16)]),  // 冲突
                    extensions: None,
                },
            ],
            network_name: "test-network".to_string(),
        };
        
        assert!(CompatibilityChecker::validate(&spec).is_err());
    }
    
    #[test]
    fn test_get_service_name() {
        let spec = ServiceSpec {
            software_type: SoftwareType::PHP,
            version: "8.2".to_string(),
            ports: HashMap::new(),
            extensions: None,
        };
        
        let name = EnvironmentBuilder::get_service_name(&spec);
        assert_eq!(name, "ps-php-8-2");
    }
    
    #[test]
    fn test_get_image_name() {
        let spec = ServiceSpec {
            software_type: SoftwareType::MySQL,
            version: "8.0".to_string(),
            ports: HashMap::new(),
            extensions: None,
        };
        
        let image = EnvironmentBuilder::get_image_name(&spec);
        assert_eq!(image, "mysql:8.0");
    }
    
    #[test]
    fn test_extension_registry() {
        // 测试内置扩展
        let ext = ExtensionRegistry::get_extension("pdo").unwrap();
        assert_eq!(ext.ext_type, ExtensionType::Builtin);
        
        // 测试核心扩展
        let ext = ExtensionRegistry::get_extension("mysqli").unwrap();
        assert_eq!(ext.ext_type, ExtensionType::Core);
        assert!(!ext.system_deps.is_empty());
        
        // 测试 PECL 扩展
        let ext = ExtensionRegistry::get_extension("redis").unwrap();
        assert_eq!(ext.ext_type, ExtensionType::Pecl);
        assert!(ext.pecl_version.is_some());
        
        // 测试无效扩展
        assert!(ExtensionRegistry::get_extension("invalid").is_none());
    }
    
    #[test]
    fn test_generate_php_dockerfile_basic() {
        let dockerfile = generate_php_dockerfile("8.2", &["mysqli".to_string()]).unwrap();
        
        assert!(dockerfile.contains("FROM php:8.2-fpm"));
        assert!(dockerfile.contains("docker-php-ext-install"));
        assert!(dockerfile.contains("mysqli"));
    }
    
    #[test]
    fn test_generate_php_dockerfile_with_pecl() {
        let dockerfile = generate_php_dockerfile("8.2", &["redis".to_string()]).unwrap();
        
        assert!(dockerfile.contains("pecl install"));
        assert!(dockerfile.contains("redis"));
        assert!(dockerfile.contains("docker-php-ext-enable"));
    }
    
    #[test]
    fn test_generate_image_tag() {
        let tag1 = generate_image_tag("8.2", &["mysqli".to_string(), "redis".to_string()]);
        let tag2 = generate_image_tag("8.2", &["redis".to_string(), "mysqli".to_string()]);
        
        // 顺序不同，但哈希应该相同
        assert_eq!(tag1, tag2);
        assert!(tag1.starts_with("php:8.2-custom-"));
    }
}
