use bollard::Docker;
use bollard::query_parameters::{CreateContainerOptions, CreateImageOptions};
use bollard::models::{HostConfig, PortBinding, Mount, MountTypeEnum};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::net::TcpListener;

/// 软件类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SoftwareType {
    #[serde(rename = "PHP")]
    PHP,
    #[serde(rename = "MySQL")]
    MySQL,
    #[serde(rename = "Redis")]
    Redis,
    #[serde(rename = "Nginx")]
    Nginx,
    #[serde(rename = "MongoDB")]
    MongoDB,
}

impl SoftwareType {
    pub fn as_str(&self) -> &str {
        match self {
            SoftwareType::PHP => "php",
            SoftwareType::MySQL => "mysql",
            SoftwareType::Redis => "redis",
            SoftwareType::Nginx => "nginx",
            SoftwareType::MongoDB => "mongodb",
        }
    }

    /// 获取默认镜像前缀
    pub fn default_image_prefix(&self) -> &str {
        match self {
            SoftwareType::PHP => "php",
            SoftwareType::MySQL => "mysql",
            SoftwareType::Redis => "redis",
            SoftwareType::Nginx => "nginx",
            SoftwareType::MongoDB => "mongo",
        }
    }

    /// 获取默认端口映射
    pub fn default_ports(&self) -> Vec<u16> {
        match self {
            SoftwareType::PHP => vec![9000],
            SoftwareType::MySQL => vec![3306],
            SoftwareType::Redis => vec![6379],
            SoftwareType::Nginx => vec![80, 443],
            SoftwareType::MongoDB => vec![27017],
        }
    }

    /// 获取默认数据卷路径（容器内）
    pub fn default_volume_path(&self) -> Option<&str> {
        match self {
            SoftwareType::MySQL => Some("/var/lib/mysql"),
            SoftwareType::Redis => Some("/data"),
            SoftwareType::MongoDB => Some("/data/db"),
            _ => None,
        }
    }
}

/// 软件版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareVersion {
    pub version: String,
    pub image_tag: String,
    pub description: String,
    pub is_stable: bool,
}

/// 软件规格（用户安装时的配置）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareSpec {
    pub software_type: SoftwareType,
    pub version: String,
    pub custom_image: Option<String>, // 自定义镜像名称（可选）
    pub port_mappings: HashMap<u16, u16>, // 容器端口 -> 宿主机端口
    pub volume_path: Option<String>, // 自定义数据卷路径（可选）
    pub env_vars: HashMap<String, String>, // 环境变量
    pub extra_args: Vec<String>, // 额外的 Docker 运行参数
}

/// 已安装的软件实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSoftware {
    pub id: String,
    pub name: String,
    pub spec: SoftwareSpec,
    pub status: String,
    pub created_at: String,
}

use crate::engine::network_manager::NetworkManager;

/// 软件管理器
pub struct SoftwareManager {
    docker: Docker,
    network_manager: NetworkManager,
}

impl SoftwareManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let docker = Docker::connect_with_local_defaults()?;
        let network_manager = NetworkManager::new()?;
        Ok(Self { 
            docker,
            network_manager,
        })
    }

    /// 获取可用软件版本清单
    pub fn get_available_versions(
        &self,
        software_type: &SoftwareType,
    ) -> Result<Vec<SoftwareVersion>, String> {
        // TODO: 可以从 Docker Hub API 拉取，或从本地缓存读取
        // 这里先返回硬编码的常用版本
        match software_type {
            SoftwareType::PHP => Ok(vec![
                SoftwareVersion {
                    version: "5.6".to_string(),
                    image_tag: "5.6-fpm".to_string(),
                    description: "PHP 5.6 (Legacy)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "7.4".to_string(),
                    image_tag: "7.4-fpm".to_string(),
                    description: "PHP 7.4 (LTS)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.0".to_string(),
                    image_tag: "8.0-fpm".to_string(),
                    description: "PHP 8.0".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.1".to_string(),
                    image_tag: "8.1-fpm".to_string(),
                    description: "PHP 8.1".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.2".to_string(),
                    image_tag: "8.2-fpm".to_string(),
                    description: "PHP 8.2 (Recommended)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.3".to_string(),
                    image_tag: "8.3-fpm".to_string(),
                    description: "PHP 8.3 (Latest)".to_string(),
                    is_stable: true,
                },
            ]),
            SoftwareType::MySQL => Ok(vec![
                SoftwareVersion {
                    version: "5.7".to_string(),
                    image_tag: "5.7".to_string(),
                    description: "MySQL 5.7 (LTS)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.0".to_string(),
                    image_tag: "8.0".to_string(),
                    description: "MySQL 8.0 (Recommended)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "8.4".to_string(),
                    image_tag: "8.4".to_string(),
                    description: "MySQL 8.4 (Latest)".to_string(),
                    is_stable: true,
                },
            ]),
            SoftwareType::Redis => Ok(vec![
                SoftwareVersion {
                    version: "6.2".to_string(),
                    image_tag: "6.2-alpine".to_string(),
                    description: "Redis 6.2 (LTS)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "7.0".to_string(),
                    image_tag: "7.0-alpine".to_string(),
                    description: "Redis 7.0 (Recommended)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "7.2".to_string(),
                    image_tag: "7.2-alpine".to_string(),
                    description: "Redis 7.2 (Latest)".to_string(),
                    is_stable: true,
                },
            ]),
            SoftwareType::Nginx => Ok(vec![
                SoftwareVersion {
                    version: "1.24".to_string(),
                    image_tag: "1.24-alpine".to_string(),
                    description: "Nginx 1.24 (Stable)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "1.25".to_string(),
                    image_tag: "1.25-alpine".to_string(),
                    description: "Nginx 1.25 (Mainline)".to_string(),
                    is_stable: true,
                },
            ]),
            SoftwareType::MongoDB => Ok(vec![
                SoftwareVersion {
                    version: "5.0".to_string(),
                    image_tag: "5.0".to_string(),
                    description: "MongoDB 5.0".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "6.0".to_string(),
                    image_tag: "6.0".to_string(),
                    description: "MongoDB 6.0 (Recommended)".to_string(),
                    is_stable: true,
                },
                SoftwareVersion {
                    version: "7.0".to_string(),
                    image_tag: "7.0".to_string(),
                    description: "MongoDB 7.0 (Latest)".to_string(),
                    is_stable: true,
                },
            ]),
        }
    }

    /// 安装软件（创建并启动容器）
    pub async fn install_software(
        &self,
        spec: SoftwareSpec,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let container_name = format!(
            "ps-{}-{}",
            spec.software_type.as_str(),
            spec.version.replace('.', "-")
        );

        // 检查容器是否已存在
        if self.container_exists(&container_name).await? {
            return Err(format!("容器 {} 已存在", container_name).into());
        }

        // 确定镜像名称
        let image = if let Some(custom) = &spec.custom_image {
            custom.clone()
        } else {
            format!(
                "{}:{}",
                spec.software_type.default_image_prefix(),
                spec.version
            )
        };

        // 拉取镜像（如果本地不存在）
        self.pull_image_if_needed(&image).await?;

        // 构建端口映射
        let port_bindings = self.build_port_bindings(&spec.port_mappings);

        // 构建挂载点
        let mounts = self.build_mounts(&spec);

        // 合并默认环境变量和用户自定义变量
        let mut env_vars = self.get_default_env_vars(&spec.software_type, &spec);
        for (k, v) in &spec.env_vars {
            env_vars.insert(k.clone(), v.clone());
        }

        let env: Vec<String> = env_vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // 创建容器配置
        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            mounts: Some(mounts),
            ..Default::default()
        };

        // 使用 HashMap 构建容器配置
        use serde_json::json;
        let config_json = json!({
            "Image": image,
            "Env": env,
            "HostConfig": host_config
        });

        let options = CreateContainerOptions {
            name: Some(container_name.clone()),
            platform: String::new(),
        };

        // 创建容器
        self.docker
            .create_container(
                Some(options),
                serde_json::from_value(config_json).unwrap()
            )
            .await?;

        // 启动容器
        self.docker
            .start_container(&container_name, None)
            .await?;

        // 将新容器加入统一网络
        let alias = self.network_manager.extract_service_alias(&container_name);
        self.network_manager.connect_container(&container_name, &alias).await?;

        Ok(container_name)
    }

    /// 卸载软件（停止并删除容器）
    pub async fn uninstall_software(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 先停止容器
        match self.docker.stop_container(name, None).await {
            Ok(_) => {}
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 304,
                ..
            }) => {
                // 容器已经停止，忽略
            }
            Err(e) => return Err(Box::new(e)),
        }

        // 删除容器
        self.docker
            .remove_container(name, None)
            .await?;

        Ok(())
    }

    /// 获取已安装的软件列表
    pub async fn list_installed_software(
        &self,
    ) -> Result<Vec<InstalledSoftware>, Box<dyn std::error::Error>> {
        use crate::docker::manager::DockerManager;
        let manager = DockerManager::new()?;
        let containers = manager.list_ps_containers().await?;

        let installed = containers
            .into_iter()
            .map(|c| {
                // 解析容器名称获取软件类型和版本
                let parts: Vec<&str> = c.name.split('-').collect();
                let software_type = if parts.len() >= 2 {
                    parts[1]
                } else {
                    "unknown"
                };

                let version = if parts.len() >= 3 {
                    parts[2..].join("-").replace('-', ".")
                } else {
                    "unknown".to_string()
                };

                InstalledSoftware {
                    id: c.id,
                    name: c.name.clone(),
                    spec: SoftwareSpec {
                        software_type: match software_type {
                            "php" => SoftwareType::PHP,
                            "mysql" => SoftwareType::MySQL,
                            "redis" => SoftwareType::Redis,
                            "nginx" => SoftwareType::Nginx,
                            "mongodb" => SoftwareType::MongoDB,
                            _ => SoftwareType::PHP, // 默认
                        },
                        version,
                        custom_image: None,
                        port_mappings: HashMap::new(),
                        volume_path: None,
                        env_vars: HashMap::new(),
                        extra_args: Vec::new(),
                    },
                    status: c.status,
                    created_at: chrono::Local::now().to_rfc3339(),
                }
            })
            .collect();

        Ok(installed)
    }

    // ==================== 私有辅助方法 ====================

    /// 检查容器是否存在
    async fn container_exists(&self, name: &str) -> Result<bool, Box<dyn std::error::Error>> {
        use bollard::query_parameters::ListContainersOptions;
        use std::collections::HashMap;

        let mut filters = HashMap::new();
        filters.insert("name".to_string(), vec![name.to_string()]);

        let options = Some(ListContainersOptions {
            all: true,
            filters: Some(filters),
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;
        Ok(!containers.is_empty())
    }

    /// 拉取镜像（如果本地不存在）
    async fn pull_image_if_needed(&self, image: &str) -> Result<(), Box<dyn std::error::Error>> {
        use futures_util::stream::StreamExt;

        // 先检查镜像是否存在
        match self.docker.inspect_image(image).await {
            Ok(_) => return Ok(()), // 镜像已存在
            Err(_) => {} // 镜像不存在，需要拉取
        }

        println!("正在拉取镜像: {}", image);

        let options = Some(CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        });

        let mut stream = self.docker.create_image(options, None, None);
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        println!("拉取进度: {}", status);
                    }
                }
                Err(e) => return Err(Box::new(e)),
            }
        }

        Ok(())
    }

    /// 构建端口绑定配置
    fn build_port_bindings(
        &self,
        port_mappings: &HashMap<u16, u16>,
    ) -> HashMap<String, Option<Vec<PortBinding>>> {
        let mut bindings = HashMap::new();

        for (container_port, host_port) in port_mappings {
            let key = format!("{}/tcp", container_port);
            bindings.insert(
                key,
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(host_port.to_string()),
                }]),
            );
        }

        bindings
    }

    /// 构建挂载点配置
    fn build_mounts(&self, spec: &SoftwareSpec) -> Vec<Mount> {
        let mut mounts = Vec::new();

        // 如果有数据卷路径，添加挂载
        if let Some(volume_path) = &spec.volume_path {
            if let Some(container_path) = spec.software_type.default_volume_path() {
                mounts.push(Mount {
                    target: Some(container_path.to_string()),
                    source: Some(volume_path.clone()),
                    typ: Some(MountTypeEnum::BIND),
                    ..Default::default()
                });
            }
        }

        mounts
    }

    /// 获取默认环境变量
    fn get_default_env_vars(
        &self,
        software_type: &SoftwareType,
        _spec: &SoftwareSpec,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();

        match software_type {
            SoftwareType::MySQL => {
                env.insert("MYSQL_ROOT_PASSWORD".to_string(), "root".to_string());
                env.insert("MYSQL_DATABASE".to_string(), "default_db".to_string());
            }
            SoftwareType::MongoDB => {
                env.insert("MONGO_INITDB_ROOT_USERNAME".to_string(), "admin".to_string());
                env.insert("MONGO_INITDB_ROOT_PASSWORD".to_string(), "admin".to_string());
            }
            _ => {}
        }

        env
    }

    // TODO: Docker Compose 集成 - 待完善
    // 详见 docs/v1.1-docker-compose-integration-plan.md

    /// 公开方法：迁移容器到统一网络
    pub async fn migrate_container_to_network(&self, container_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let alias = self.network_manager.extract_service_alias(container_name);
        self.network_manager.connect_container(container_name, &alias).await
    }
}

/// 端口分配器 - 自动查找可用端口
pub struct PortAllocator;

impl PortAllocator {
    /// 检测端口是否可用
    pub fn is_port_available(port: u16) -> bool {
        TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// 从指定范围查找第一个可用端口
    pub fn find_available_port(start: u16, end: u16) -> Option<u16> {
        for port in start..=end {
            if Self::is_port_available(port) {
                return Some(port);
            }
        }
        None
    }

    /// 为软件分配端口（避免冲突）
    pub fn allocate_ports(
        software_type: &SoftwareType,
        preferred_ports: &[u16],
    ) -> HashMap<u16, u16> {
        let default_ports = software_type.default_ports();
        let mut mappings = HashMap::new();

        for (idx, &container_port) in default_ports.iter().enumerate() {
            let preferred = if idx < preferred_ports.len() {
                preferred_ports[idx]
            } else {
                container_port
            };

            // 如果首选端口可用，使用它
            let host_port = if Self::is_port_available(preferred) {
                preferred
            } else {
                // 否则在范围内查找可用端口
                Self::find_available_port(preferred, preferred + 100).unwrap_or(container_port)
            };

            mappings.insert(container_port, host_port);
        }

        mappings
    }
}
