use bollard::Docker;
use bollard::query_parameters::{
    ListContainersOptions, StartContainerOptions, StopContainerOptions, RestartContainerOptions,
};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct PsContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: Vec<i32>,
}

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker })
    }

    pub async fn list_ps_containers(&self) -> Result<Vec<PsContainer>, Box<dyn std::error::Error>> {
        let mut filters = HashMap::new();
        filters.insert("name".to_string(), vec!["ps-".to_string()]);
        
        let options = Some(ListContainersOptions {
            all: true,
            filters: Some(filters),
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;
        
        let ps_containers = containers.into_iter().map(|c| {
            let name = c.names.clone().unwrap_or_default().get(0)
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| "unknown".to_string());

            PsContainer {
                id: c.id.unwrap_or_default(),
                name,
                image: c.image.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                state: format!("{:?}", c.state),
                ports: c.ports.unwrap_or_default().into_iter()
                    .filter_map(|p| p.public_port.map(|port| port as i32))
                    .collect(),
            }
        }).collect();

        Ok(ps_containers)
    }

    pub async fn start_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker.start_container(name, None::<StartContainerOptions>).await?;
        Ok(())
    }

    pub async fn stop_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker.stop_container(name, None::<StopContainerOptions>).await?;
        Ok(())
    }

    pub async fn restart_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker.restart_container(name, None::<RestartContainerOptions>).await?;
        Ok(())
    }
}
