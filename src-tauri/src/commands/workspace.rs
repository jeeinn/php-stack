use crate::engine::version_manifest::{VersionManifest, ServiceType as VmServiceType};
use crate::engine::user_override_manager::{UserOverrideManager, UserVersionOverride};
use crate::engine::workspace_manager::WorkspaceManager;

use super::get_project_root;

/// 打开指定服务的配置文件目录
#[tauri::command]
pub fn open_service_config(service_name: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let service_dir = project_root.join("services").join(&service_name);
    
    if !service_dir.exists() {
        return Err(format!("服务配置目录不存在: {}", service_dir.display()));
    }
    
    // 在 Windows 上使用 explorer 打开目录
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("无法打开目录: {e}"))?;
    }
    
    // 在 macOS 上使用 open 命令
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("无法打开目录: {}", e))?;
    }
    
    // 在 Linux 上使用 xdg-open 命令
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("无法打开目录: {}", e))?;
    }
    
    Ok(())
}

/// 获取当前工作目录信息
#[tauri::command]
pub fn get_workspace_info() -> Result<Option<crate::engine::workspace_manager::WorkspaceConfig>, String> {
    WorkspaceManager::load_workspace()
}

/// 设置工作目录路径
#[tauri::command]
pub fn set_workspace_path(path: String) -> Result<(), String> {
    // 验证路径是否存在
    if !std::path::PathBuf::from(&path).exists() {
        return Err("指定的工作目录路径不存在".to_string());
    }
    WorkspaceManager::save_workspace(&path)
}

/// 获取所有可用的版本映射配置
#[tauri::command]
pub fn get_version_mappings() -> Result<serde_json::Value, String> {
    use std::collections::HashMap;
    
    let manifest = VersionManifest::new();
    let project_root = get_project_root()?;
    let override_manager = UserOverrideManager::new(&project_root);
    let mut result = HashMap::new();
    
    // 使用辅助函数处理每种服务类型
    let service_types = [
        ("php", VmServiceType::Php),
        ("mysql", VmServiceType::Mysql),
        ("redis", VmServiceType::Redis),
        ("nginx", VmServiceType::Nginx),
    ];
    
    for (key, service_type) in &service_types {
        let mut versions = Vec::new();
        let entries = manifest.get_available_entries(service_type);
        
        for (id, entry) in entries {
            // 使用合并后的配置（用户覆盖优先）
            let merged_entry = override_manager.get_merged_entry(service_type, id)
                .unwrap_or_else(|| entry.clone());
            
            let has_user_override = override_manager.has_user_override(service_type, id);
            
            versions.push(serde_json::json!({
                "id": id,
                "display_name": merged_entry.display_name,
                "image_tag": merged_entry.image_tag,
                "service_dir": merged_entry.service_dir,
                "default_port": merged_entry.default_port,
                "show_port": merged_entry.show_port,
                "eol": merged_entry.eol,
                "description": merged_entry.description,
                "has_user_override": has_user_override
            }));
        }
        result.insert(key.to_string(), serde_json::Value::Array(versions));
    }
    
    serde_json::to_value(result).map_err(|e| format!("序列化失败: {e}"))
}

/// 验证指定的版本是否存在
#[tauri::command]
pub fn validate_version(service_type: String, version: String) -> Result<bool, String> {
    let manifest = VersionManifest::new();
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {service_type}")),
    };
    
    Ok(manifest.is_id_valid(&vm_service_type, &version))
}

/// 获取推荐版本
#[tauri::command]
pub fn get_recommended_version(service_type: String) -> Result<Option<String>, String> {
    let manifest = VersionManifest::new();
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {service_type}")),
    };
    
    Ok(manifest.get_recommended_entry(&vm_service_type).map(|(id, _)| id.to_string()))
}

/// 保存用户自定义版本覆盖
#[tauri::command]
pub fn save_user_override(
    service_type: String,
    id: String,
    image_tag: String,
    description: Option<String>,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);
    
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {service_type}")),
    };
    
    let override_config = UserVersionOverride {
        image_tag,
        description,
    };
    
    manager.save_user_override(&project_root, vm_service_type, id, override_config)
}

/// 删除用户自定义版本覆盖
#[tauri::command]
pub fn remove_user_override(service_type: String, id: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);
    
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {service_type}")),
    };
    
    manager.remove_user_override(&project_root, &vm_service_type, &id)
}

/// 重置所有用户自定义版本覆盖
#[tauri::command]
pub fn reset_all_overrides() -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);
    
    manager.reset_all_overrides(&project_root)
}

/// 导出当前会话日志
#[tauri::command]
pub fn export_logs() -> Result<String, String> {
    // 获取项目根目录（与 get_project_root 逻辑一致）
    let log_dir = if cfg!(debug_assertions) {
        // 开发模式：使用项目根目录
        std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .ok_or("无法获取项目根目录")?
            .to_path_buf()
    } else {
        // 生产模式：使用可执行文件所在目录
        std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent()
            .ok_or("无法获取程序所在目录")?
            .to_path_buf()
    };
    
    let log_path = log_dir.join("php-stack.log");
    
    if !log_path.exists() {
        return Err("日志文件不存在，请先执行一些操作".to_string());
    }
    
    std::fs::read_to_string(&log_path)
        .map_err(|e| format!("读取日志失败: {e}"))
}
