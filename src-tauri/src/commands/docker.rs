use crate::app_log;
use crate::docker::host_status::{self, DockerHostReport};
use crate::docker::manager::{DockerManager, PsContainer};

#[tauri::command]
pub async fn check_docker() -> Result<(), String> {
    let manager =
        DockerManager::new().map_err(|e| format!("Docker installation not found: {e}"))?;
    manager.check_docker_availability().await
}

/// 仪表盘用这一次调用区分引擎就绪、未安装、未启动、权限和自定义地址。
/// 不可用不是命令失败：状态在返回值里，避免失败路径再打一次 ping。
#[tauri::command]
pub async fn inspect_docker_host() -> DockerHostReport {
    // `new()` 的错误是 `Box<dyn Error>`，不能留在 await 两侧，先收成 String。
    let manager = match DockerManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            return host_status::report_from_ping(Some(format!(
                "Docker installation not found: {e}"
            )));
        }
    };
    let ping_error = manager.check_docker_availability().await.err();
    host_status::report_from_ping(ping_error)
}

/// 启动本机已安装的 Docker Desktop。不代为安装，也不拉起 Linux 上的 dockerd。
#[tauri::command]
pub fn open_docker_desktop() -> Result<(), String> {
    match host_status::launch_docker_desktop() {
        Ok(()) => {
            app_log!(info, "docker", "Docker Desktop launch requested");
            Ok(())
        }
        Err(e) => {
            app_log!(warn, "docker", "failed to open Docker Desktop: {e}");
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn list_containers() -> Result<Vec<PsContainer>, String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager
        .list_ps_containers()
        .await
        .map_err(|e| e.to_string())
}

/// 获取所有运行中的容器（用于端口冲突检测）
#[tauri::command]
pub async fn list_all_running_containers() -> Result<Vec<PsContainer>, String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager
        .list_all_running_containers()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager
        .start_container(&name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager
        .stop_container(&name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager
        .restart_container(&name)
        .await
        .map_err(|e| e.to_string())
}
