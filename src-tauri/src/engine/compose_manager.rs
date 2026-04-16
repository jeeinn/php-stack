use serde::{Serialize, Deserialize};
use std::fs;
use std::collections::HashMap;
use crate::engine::software_manager::{InstalledSoftware, SoftwareType};
use crate::engine::restart_analyzer::{RestartAnalyzer, RestartImpact};

/// Docker Compose 配置文件结构
#[derive(Debug, Serialize, Deserialize)]
pub struct DockerCompose {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,  // Docker Compose v2+ 不再需要，但保留兼容性
    pub networks: HashMap<String, NetworkConfig>,
    pub services: HashMap<String, ServiceConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<HashMap<String, VolumeConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub driver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub image: String,
    pub container_name: String,
    pub networks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeConfig {
    pub driver: String,
}

pub struct ComposeManager {
    compose_path: String,
}

impl ComposeManager {
    pub fn new(project_root: &str) -> Self {
        Self {
            compose_path: format!("{}/docker-compose.yml", project_root),
        }
    }

    /// 根据已安装的容器重建 docker-compose.yml
    pub async fn rebuild_from_containers(
        &self,
        containers: &[InstalledSoftware]
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut compose = DockerCompose {
            version: None,  // Docker Compose v2+ 不再需要 version 字段
            networks: HashMap::from([
                ("php-stack-network".to_string(), NetworkConfig {
                    driver: "bridge".to_string(),
                    external: Some(true), // 网络已由 NetworkManager 创建
                })
            ]),
            services: HashMap::new(),
            volumes: None,
        };

        // 为每个容器生成服务配置
        for container in containers {
            let service_name = self.extract_service_name(&container.name);
            let service_config = self.build_service_config(container)?;
            compose.services.insert(service_name, service_config);
        }

        // 如果没有服务，生成最小化的 compose 文件
        let yaml = if compose.services.is_empty() {
            // 生成一个最小化的 compose 文件（不包含 version）
            format!(
                "networks:\n  php-stack-network:\n    driver: bridge\n    external: true\nservices: {{}}\n"
            )
        } else {
            serde_yaml::to_string(&compose)?
        };
        
        fs::write(&self.compose_path, &yaml)?;

        log::info!("✅ docker-compose.yml 已更新 ({} 个服务)", compose.services.len());
        Ok(())
    }

    /// 构建单个服务的配置
    fn build_service_config(
        &self,
        container: &InstalledSoftware
    ) -> Result<ServiceConfig, Box<dyn std::error::Error>> {
        let spec = &container.spec;
        
        // 端口映射: "宿主机端口:容器端口"
        let ports: Vec<String> = spec.port_mappings.iter()
            .map(|(container_port, host_port)| {
                format!("{}:{}", host_port, container_port)
            })
            .collect();

        // 数据卷挂载
        let volumes = if let Some(volume_path) = &spec.volume_path {
            if let Some(container_path) = spec.software_type.default_volume_path() {
                Some(vec![format!("{}:{}", volume_path, container_path)])
            } else {
                None
            }
        } else {
            None
        };

        // 依赖关系
        let depends_on = self.determine_dependencies(&spec.software_type);

        // 构建环境变量（如果有）
        let environment = if spec.env_vars.is_empty() { 
            None 
        } else { 
            Some(spec.env_vars.clone()) 
        };

        Ok(ServiceConfig {
            image: if let Some(ref custom_image) = spec.custom_image {
                custom_image.clone()
            } else {
                format!("{}:{}", 
                    spec.software_type.default_image_prefix(),
                    spec.version
                )
            },
            container_name: container.name.clone(),
            networks: vec!["php-stack-network".to_string()],
            // 关键修复：空数组时不输出该字段
            ports: if ports.is_empty() { 
                None 
            } else { 
                Some(ports) 
            },
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

    /// 确定服务依赖关系
    fn determine_dependencies(
        &self,
        software_type: &SoftwareType
    ) -> Option<Vec<String>> {
        match software_type {
            SoftwareType::PHP => {
                // PHP 通常依赖数据库和缓存
                Some(vec![
                    "mysql".to_string(),
                    "redis".to_string(),
                ])
            }
            SoftwareType::Nginx => {
                // Nginx 依赖 PHP-FPM
                Some(vec!["php".to_string()])
            }
            _ => None,
        }
    }

    /// 从容器名提取服务名（去掉 ps- 前缀和版本号）
    fn extract_service_name(&self, container_name: &str) -> String {
        // ps-php-8-2 → php
        // ps-mysql-5-7 → mysql
        container_name
            .trim_start_matches("ps-")
            .split('-')
            .next()
            .unwrap_or(container_name)
            .to_string()
    }

    /// 执行 docker-compose up -d（应用变更）
    pub async fn apply_changes(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::process::Command;

        log::info!("🔄 应用 docker-compose 变更...");

        // 使用 --remove-orphans 自动清理不再定义的服务
        // 使用 --force-recreate 强制重新创建容器以避免名称冲突
        let output = Command::new("docker")
            .args(&[
                "compose",
                "-f", &self.compose_path,
                "up", "-d", "--remove-orphans"
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // 如果是容器名称冲突，先停止并删除冲突的容器
            if stderr.contains("is already in use") {
                log::warn!("⚠️ 检测到容器名称冲突，尝试清理...");
                return self.handle_container_conflict().await;
            }
            
            return Err(format!(
                "docker compose 执行失败: {}",
                stderr
            ).into());
        }

        log::info!("✅ 服务已成功应用");
        Ok(())
    }

    /// 处理容器名称冲突
    async fn handle_container_conflict(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::process::Command;
        
        // 先停止所有相关容器
        log::info!("🛑 停止所有 php-stack 容器...");
        let _ = Command::new("docker")
            .args(&["stop"])
            .arg("ps-php-5-6")
            .arg("ps-php-7-4")
            .arg("ps-php-8-2")
            .arg("ps-mysql-5-7")
            .arg("ps-mysql-8-0")
            .arg("ps-redis-6-2")
            .arg("ps-redis-7-0")
            .arg("ps-nginx-1-24")
            .arg("ps-mongodb-5-0")
            .output()
            .await;
        
        // 等待一下确保容器完全停止
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // 再次尝试 up
        log::info!("🔄 重新启动服务...");
        let output = Command::new("docker")
            .args(&[
                "compose",
                "-f", &self.compose_path,
                "up", "-d", "--remove-orphans"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(format!(
                "清理后仍然失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ).into());
        }

        log::info!("✅ 冲突已解决，服务已成功启动");
        Ok(())
    }

    /// 智能重启：只重启受影响的容器
    pub async fn smart_restart(
        &self,
        affected_services: &[String]
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::process::Command;

        for service in affected_services {
            log::info!("🔄 重启服务: {}", service);
            
            let output = Command::new("docker")
                .args(&[
                    "compose",
                    "-f", &self.compose_path,
                    "restart", service
                ])
                .output()
                .await?;

            if !output.status.success() {
                log::warn!("⚠️ 重启服务 {} 失败: {}", service, String::from_utf8_lossy(&output.stderr));
            }
        }

        Ok(())
    }

    /// 分析服务修改的影响范围（智能重启的核心）
    pub fn analyze_restart_impact(
        &self,
        modified_service_name: &str,
        installed_containers: &[InstalledSoftware],
    ) -> RestartImpact {
        // 提取服务名（去除版本号）
        let service_name = RestartAnalyzer::extract_service_name(modified_service_name);
        
        // 获取所有已安装的服务名
        let all_services: Vec<String> = installed_containers
            .iter()
            .map(|c| RestartAnalyzer::extract_service_name(&c.name))
            .collect();
        
        log::info!("🔍 分析 {} 修改的影响范围...", service_name);
        log::info!("   已安装服务: {:?}", all_services);
        
        // 使用分析引擎计算影响范围
        let impact = RestartAnalyzer::analyze_dependencies(&service_name, &all_services);
        
        log::info!("✅ 影响分析完成: {} 个服务需要重启", impact.total_affected);
        for chain in &impact.dependency_chain {
            log::info!("   {}", chain);
        }
        
        impact
    }

    /// 智能重启：基于依赖分析的最小化重启
    pub async fn smart_restart_with_analysis(
        &self,
        modified_service_name: &str,
        installed_containers: &[InstalledSoftware],
    ) -> Result<RestartImpact, Box<dyn std::error::Error>> {
        // 1. 分析影响范围
        let impact = self.analyze_restart_impact(modified_service_name, installed_containers);
        
        // 2. 如果没有受影响的服务，直接返回
        if impact.services_to_restart.is_empty() {
            return Ok(impact);
        }
        
        // 3. 执行智能重启
        log::info!("🔄 开始智能重启 {} 个服务...", impact.services_to_restart.len());
        self.smart_restart(&impact.services_to_restart).await?;
        
        log::info!("✅ 智能重启完成");
        Ok(impact)
    }

    /// 获取当前 compose 文件路径
    pub fn get_compose_path(&self) -> &str {
        &self.compose_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_extract_service_name() {
        let manager = ComposeManager::new(".");
        
        assert_eq!(manager.extract_service_name("ps-php-8-2"), "php");
        assert_eq!(manager.extract_service_name("ps-mysql-5-7"), "mysql");
        assert_eq!(manager.extract_service_name("ps-nginx-1-24"), "nginx");
        assert_eq!(manager.extract_service_name("ps-redis-7-0"), "redis");
    }

    #[test]
    fn test_determine_dependencies() {
        let manager = ComposeManager::new(".");
        
        // PHP 应该依赖 mysql 和 redis
        let php_deps = manager.determine_dependencies(&SoftwareType::PHP);
        assert!(php_deps.is_some());
        let deps = php_deps.unwrap();
        assert!(deps.contains(&"mysql".to_string()));
        assert!(deps.contains(&"redis".to_string()));
        
        // Nginx 应该依赖 php
        let nginx_deps = manager.determine_dependencies(&SoftwareType::Nginx);
        assert!(nginx_deps.is_some());
        assert_eq!(nginx_deps.unwrap(), vec!["php".to_string()]);
        
        // MySQL 不应该有依赖
        let mysql_deps = manager.determine_dependencies(&SoftwareType::MySQL);
        assert!(mysql_deps.is_none());
    }

    #[test]
    fn test_build_service_config_basic() {
        let manager = ComposeManager::new(".");
        
        let container = InstalledSoftware {
            id: "test-id".to_string(),
            name: "ps-php-8-2".to_string(),
            spec: crate::engine::software_manager::SoftwareSpec {
                software_type: SoftwareType::PHP,
                version: "8.2".to_string(),
                custom_image: None,
                port_mappings: HashMap::from([
                    (9000, 9000),
                ]),
                volume_path: None,
                env_vars: HashMap::new(),
                extra_args: Vec::new(),
            },
            status: "running".to_string(),
            created_at: "2026-04-16T00:00:00Z".to_string(),
        };
        
        let config = manager.build_service_config(&container).unwrap();
        
        assert_eq!(config.image, "php:8.2");
        assert_eq!(config.container_name, "ps-php-8-2");
        assert_eq!(config.networks, vec!["php-stack-network"]);
        assert!(config.ports.is_some());
        assert_eq!(config.ports.unwrap(), vec!["9000:9000"]);
        assert_eq!(config.restart, Some("unless-stopped".to_string()));
    }
}