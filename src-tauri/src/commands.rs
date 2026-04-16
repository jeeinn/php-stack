use crate::docker::manager::{DockerManager, PsContainer};
use crate::docker::mirror::MirrorManager;
use crate::engine::export::{ExportEngine, ExportOptions};
use crate::engine::software_manager::{
    SoftwareManager, SoftwareSpec, SoftwareType, InstalledSoftware, SoftwareVersion, PortAllocator,
};

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

// ==================== 软件管理中心命令 ====================

/// 获取可用软件版本清单
#[tauri::command]
pub fn get_available_versions(software_type_str: String) -> Result<Vec<SoftwareVersion>, String> {
    let software_type = match software_type_str.as_str() {
        "php" => SoftwareType::PHP,
        "mysql" => SoftwareType::MySQL,
        "redis" => SoftwareType::Redis,
        "nginx" => SoftwareType::Nginx,
        "mongodb" => SoftwareType::MongoDB,
        _ => return Err(format!("不支持的软件类型: {}", software_type_str)),
    };

    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    manager.get_available_versions(&software_type)
}

/// 安装软件
#[tauri::command]
pub async fn install_software(spec: SoftwareSpec) -> Result<String, String> {
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    manager
        .install_software(spec)
        .await
        .map_err(|e| format!("安装失败: {}", e))
}

/// 卸载软件
#[tauri::command]
pub async fn uninstall_software(name: String) -> Result<(), String> {
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    manager
        .uninstall_software(&name)
        .await
        .map_err(|e| format!("卸载失败: {}", e))
}

/// 获取已安装的软件列表
#[tauri::command]
pub async fn list_installed_software() -> Result<Vec<InstalledSoftware>, String> {
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    manager
        .list_installed_software()
        .await
        .map_err(|e| format!("获取列表失败: {}", e))
}

/// 检测端口是否可用
#[tauri::command]
pub fn check_port_available(port: u16) -> Result<bool, String> {
    Ok(PortAllocator::is_port_available(port))
}

/// 自动分配端口（避免冲突）
#[tauri::command]
pub fn allocate_ports(
    software_type_str: String,
    preferred_ports: Vec<u16>,
) -> Result<std::collections::HashMap<u16, u16>, String> {
    let software_type = match software_type_str.as_str() {
        "php" => SoftwareType::PHP,
        "mysql" => SoftwareType::MySQL,
        "redis" => SoftwareType::Redis,
        "nginx" => SoftwareType::Nginx,
        "mongodb" => SoftwareType::MongoDB,
        _ => return Err(format!("不支持的软件类型: {}", software_type_str)),
    };

    Ok(PortAllocator::allocate_ports(
        &software_type,
        &preferred_ports,
    ))
}
