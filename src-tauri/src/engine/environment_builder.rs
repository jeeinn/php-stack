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
    
    /// 生成优化的 docker-compose 配置
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
    
    /// 获取数据卷配置
    fn get_volumes(spec: &ServiceSpec) -> Option<Vec<String>> {
        let volume_path = match spec.software_type {
            SoftwareType::MySQL => "./data/mysql:/var/lib/mysql",
            SoftwareType::Redis => "./data/redis:/data",
            SoftwareType::Nginx => "./data/www:/var/www/html",
            _ => return None,
        };
        
        Some(vec![volume_path.to_string()])
    }
    
    /// 获取依赖关系
    fn get_dependencies(spec: &ServiceSpec) -> Option<Vec<String>> {
        match spec.software_type {
            SoftwareType::PHP => {
                // PHP 可能依赖 MySQL 和 Redis
                Some(vec!["mysql".to_string(), "redis".to_string()])
            }
            SoftwareType::Nginx => {
                // Nginx 依赖 PHP-FPM
                Some(vec!["php".to_string()])
            }
            _ => None,
        }
    }
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
}
