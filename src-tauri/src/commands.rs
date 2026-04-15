use crate::docker::manager::{DockerManager, PsContainer};
use crate::docker::mirror::MirrorManager;
use crate::engine::export::{ExportEngine, ExportOptions};

#[tauri::command]
pub async fn check_docker() -> Result<(), String> {
    let manager = DockerManager::new().map_err(|e| format!("未找到 Docker 安装: {}", e))?;
    manager.check_docker_availability().await
}

#[tauri::command]
pub async fn list_containers() -> Result<Vec<PsContainer>, String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.list_ps_containers().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.start_container(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.stop_container(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_container(name: String) -> Result<(), String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.restart_container(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_docker_mirror(url: String) -> Result<(), String> {
    MirrorManager::set_docker_mirror(&url).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_stack(save_path: String, options: ExportOptions) -> Result<String, String> {
    ExportEngine::run_export(&save_path, options).await.map_err(|e| e.to_string())?;
    Ok(format!("导出成功: {}", save_path))
}
