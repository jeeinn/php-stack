use bollard::models::ContainerSummaryStateEnum;
use bollard::query_parameters::{
    ListContainersOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// 容器运行状态。
///
/// 前后端契约：序列化为小写字符串（`running` / `exited` / ...）。
/// 此前后端用 `format!("{:?}", c.state)` 产出 `"Some(RUNNING)"`，前端靠
/// `includes('running')` 猜测解析——Docker 或 bollard 升级即碎。枚举化后
/// 契约由 serde 保证，两端都不再做字符串猜测。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Exited,
    Removing,
    Dead,
    /// Docker 未返回状态（字段缺失或为空）。保留兜底变体，避免反序列化失败。
    Unknown,
}

impl ContainerState {
    /// 仅 `running` 视为运行中；`restarting` 尚未真正提供服务，不算。
    pub fn is_running(&self) -> bool {
        matches!(self, ContainerState::Running)
    }
}

impl From<Option<ContainerSummaryStateEnum>> for ContainerState {
    fn from(state: Option<ContainerSummaryStateEnum>) -> Self {
        match state {
            Some(ContainerSummaryStateEnum::CREATED) => ContainerState::Created,
            Some(ContainerSummaryStateEnum::RUNNING) => ContainerState::Running,
            Some(ContainerSummaryStateEnum::PAUSED) => ContainerState::Paused,
            Some(ContainerSummaryStateEnum::RESTARTING) => ContainerState::Restarting,
            Some(ContainerSummaryStateEnum::EXITED) => ContainerState::Exited,
            Some(ContainerSummaryStateEnum::REMOVING) => ContainerState::Removing,
            Some(ContainerSummaryStateEnum::DEAD) => ContainerState::Dead,
            // 空字符串变体与字段缺失都归为 Unknown
            Some(ContainerSummaryStateEnum::EMPTY) | None => ContainerState::Unknown,
        }
    }
}

impl fmt::Display for ContainerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ContainerState::Created => "created",
            ContainerState::Running => "running",
            ContainerState::Paused => "paused",
            ContainerState::Restarting => "restarting",
            ContainerState::Exited => "exited",
            ContainerState::Removing => "removing",
            ContainerState::Dead => "dead",
            ContainerState::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PsContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: ContainerState,
    pub ports: Vec<i32>,
}

/// 把 bollard 的容器摘要映射为前端契约结构。
///
/// `list_ps_containers` 与 `list_all_running_containers` 此前各抄了一遍，
/// 两处必须同步修改才能保持一致——收敛为单一实现。
fn to_ps_container(c: bollard::models::ContainerSummary) -> PsContainer {
    let name = c
        .names
        .clone()
        .unwrap_or_default()
        .first()
        .map(|n| n.trim_start_matches('/').to_string())
        .unwrap_or_else(|| "unknown".to_string());

    PsContainer {
        id: c.id.unwrap_or_default(),
        name,
        image: c.image.unwrap_or_default(),
        status: c.status.unwrap_or_default(),
        state: ContainerState::from(c.state),
        ports: c
            .ports
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.public_port.map(|port| port as i32))
            .collect(),
    }
}

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker })
    }

    pub async fn check_docker_availability(&self) -> Result<(), String> {
        match self.docker.ping().await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Docker service unavailable or not running: {e}")),
        }
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

        Ok(containers.into_iter().map(to_ps_container).collect())
    }

    /// 获取所有运行中的容器（用于端口冲突检测）
    pub async fn list_all_running_containers(
        &self,
    ) -> Result<Vec<PsContainer>, Box<dyn std::error::Error>> {
        let options = Some(ListContainersOptions {
            all: false, // 只返回运行中的容器
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;

        Ok(containers.into_iter().map(to_ps_container).collect())
    }

    pub async fn start_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker
            .start_container(name, None::<StartContainerOptions>)
            .await?;
        Ok(())
    }

    pub async fn stop_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker
            .stop_container(name, None::<StopContainerOptions>)
            .await?;
        Ok(())
    }

    pub async fn restart_container(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.docker
            .restart_container(name, None::<RestartContainerOptions>)
            .await?;
        Ok(())
    }

    /// 检查所有 ps- 前缀的容器是否都处于 running 状态
    pub async fn check_all_ps_containers_running(&self) -> Result<bool, String> {
        let containers = self
            .list_ps_containers()
            .await
            .map_err(|e| format!("failed to list containers: {e}"))?;

        if containers.is_empty() {
            return Ok(false);
        }

        // 检查所有容器是否都是 running 状态
        for container in &containers {
            if !container.state.is_running() {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_state_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&ContainerState::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&ContainerState::Exited).unwrap(),
            "\"exited\""
        );
        assert_eq!(
            serde_json::to_string(&ContainerState::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    #[test]
    fn test_container_state_roundtrip() {
        for state in [
            ContainerState::Created,
            ContainerState::Running,
            ContainerState::Paused,
            ContainerState::Restarting,
            ContainerState::Exited,
            ContainerState::Removing,
            ContainerState::Dead,
            ContainerState::Unknown,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            let back: ContainerState = serde_json::from_str(&json).unwrap();
            assert_eq!(back, state, "往返失败: {json}");
        }
    }

    #[test]
    fn test_container_state_from_bollard_enum() {
        assert_eq!(
            ContainerState::from(Some(ContainerSummaryStateEnum::RUNNING)),
            ContainerState::Running
        );
        assert_eq!(
            ContainerState::from(Some(ContainerSummaryStateEnum::EXITED)),
            ContainerState::Exited
        );
        // 空字符串变体与字段缺失都不能让反序列化崩掉
        assert_eq!(
            ContainerState::from(Some(ContainerSummaryStateEnum::EMPTY)),
            ContainerState::Unknown
        );
        assert_eq!(ContainerState::from(None), ContainerState::Unknown);
    }

    #[test]
    fn test_is_running_only_for_running() {
        assert!(ContainerState::Running.is_running());
        // restarting 尚未真正提供服务，不当作运行中
        assert!(!ContainerState::Restarting.is_running());
        assert!(!ContainerState::Exited.is_running());
        assert!(!ContainerState::Unknown.is_running());
    }

    #[test]
    fn test_container_state_display() {
        assert_eq!(ContainerState::Running.to_string(), "running");
        assert_eq!(ContainerState::Dead.to_string(), "dead");
    }
}
