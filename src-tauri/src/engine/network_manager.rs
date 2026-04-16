use bollard::Docker;
use bollard::models::{NetworkCreateRequest, NetworkConnectRequest, EndpointSettings};

pub struct NetworkManager {
    docker: Docker,
    network_name: String,
}

impl NetworkManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
            network_name: "php-stack-network".to_string(),
        })
    }

    /// 确保网络存在，不存在则创建
    pub async fn ensure_network_exists(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.docker.inspect_network(&self.network_name, None).await {
            Ok(_) => {
                log::debug!("网络 {} 已存在", self.network_name);
                Ok(())
            }
            Err(_) => {
                // 创建桥接网络
                let create_request = NetworkCreateRequest {
                    name: self.network_name.clone(),
                    ..Default::default()
                };
                
                self.docker.create_network(create_request).await?;
                log::info!("✅ 创建统一网络: {}", self.network_name);
                Ok(())
            }
        }
    }

    /// 将容器连接到网络，并设置服务别名
    pub async fn connect_container(
        &self,
        container_name: &str,
        alias: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 首先确保网络存在
        self.ensure_network_exists().await?;

        // 构建端点配置，设置别名
        let endpoint_config = EndpointSettings {
            aliases: Some(vec![alias.to_string()]),
            ..Default::default()
        };

        let connect_request = NetworkConnectRequest {
            container: container_name.to_string(),
            endpoint_config: Some(endpoint_config),
            ..Default::default()
        };

        self.docker
            .connect_network(&self.network_name, connect_request)
            .await?;

        log::info!(
            "✅ 容器 {} 已加入网络 {}（别名: {}）",
            container_name,
            self.network_name,
            alias
        );

        Ok(())
    }

    /// 从容器名提取服务别名（ps-php-8-2 -> php）
    pub fn extract_service_alias(&self, container_name: &str) -> String {
        // ps-php-8-2 -> php
        // ps-mysql-5-7 -> mysql
        // ps-nginx-1-24 -> nginx
        container_name
            .trim_start_matches("ps-")
            .split('-')
            .next()
            .unwrap_or(container_name)
            .to_string()
    }
}
