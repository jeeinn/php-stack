use crate::app_log;
use crate::engine::user_override_manager::UserOverrideManager;
use crate::engine::version_manifest::{ServiceType as VmServiceType, VersionManifest};
use crate::engine::workspace_manager::WorkspaceManager;

use super::{get_log_file, get_project_root, paths};

/// 打开指定服务的配置文件目录
#[tauri::command]
pub fn open_service_config(service_name: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let service_dir = project_root.join("services").join(&service_name);

    if !service_dir.exists() {
        return Err(format!(
            "service config dir not found: {}",
            service_dir.display()
        ));
    }

    // 在 Windows 上使用 explorer 打开目录
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("failed to open directory: {e}"))?;
    }

    // 在 macOS 上使用 open 命令
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("failed to open directory: {e}"))?;
    }

    // 在 Linux 上使用 xdg-open 命令
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(service_dir)
            .spawn()
            .map_err(|e| format!("failed to open directory: {e}"))?;
    }

    Ok(())
}

/// 工作区信息。
///
/// `workspace_path` 是用户配置的值，`effective_path` 是**数据真正落到的地方**。
/// 两者不一致（`using_fallback`）时必须让用户看到，否则配置看起来"没生效"。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceInfo {
    /// 用户配置的工作区路径
    pub workspace_path: String,
    /// 实际生效的数据落点
    pub effective_path: String,
    /// 配置路径不可用时为 true，数据正写到 effective_path
    pub using_fallback: bool,
    /// 配置路径不存在，需用户选择：重建 / 选新路径 / 临时回退
    pub path_missing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

/// 获取当前工作目录信息
///
/// 未配置过工作区时返回 `None`，前端据此弹出初始化对话框。
#[tauri::command]
pub fn get_workspace_info() -> Result<Option<WorkspaceInfo>, String> {
    let Some(config) = WorkspaceManager::load_workspace()? else {
        return Ok(None);
    };

    let resolved = paths::resolve_workspace()?;
    Ok(Some(WorkspaceInfo {
        workspace_path: config.workspace_path,
        effective_path: resolved.path.to_string_lossy().to_string(),
        using_fallback: resolved.fell_back,
        path_missing: resolved.path_missing,
        fallback_reason: resolved.reason,
        last_updated: config.last_updated,
    }))
}

/// 设置工作目录路径
#[tauri::command]
pub fn set_workspace_path(path: String) -> Result<(), String> {
    // 验证路径是否存在
    if !std::path::PathBuf::from(&path).exists() {
        return Err("workspace path does not exist".to_string());
    }
    WorkspaceManager::save_workspace(&path)
}

/// 按用户确认重建配置中的工作区目录（不再静默 create_dir_all）
#[tauri::command]
pub fn recreate_workspace_dir() -> Result<WorkspaceInfo, String> {
    let path = paths::recreate_configured_workspace()?;
    app_log!(
        info,
        "commands::recreate_workspace_dir",
        "Recreated workspace: {}",
        path.display()
    );

    get_workspace_info()?.ok_or_else(|| "failed to read workspace info after recreate".to_string())
}

/// 获取所有可用的版本映射配置（清单 + 用户自定义）
#[tauri::command]
pub fn get_version_mappings() -> Result<serde_json::Value, String> {
    use std::collections::HashMap;

    let project_root = get_project_root()?;
    let override_manager = UserOverrideManager::new(&project_root);
    let mut result = HashMap::new();

    let service_types = [
        ("php", VmServiceType::Php),
        ("mysql", VmServiceType::Mysql),
        ("redis", VmServiceType::Redis),
        ("nginx", VmServiceType::Nginx),
    ];

    for (key, service_type) in &service_types {
        let mut versions = Vec::new();
        for item in override_manager.list_merged_entries(service_type) {
            versions.push(serde_json::json!({
                "id": item.id,
                "display_name": item.entry.display_name,
                "image_tag": item.entry.image_tag,
                "service_dir": item.entry.service_dir,
                "default_port": item.entry.default_port,
                "show_port": item.entry.show_port,
                "eol": item.entry.eol,
                "description": item.entry.description,
                "has_user_override": item.has_user_override,
                "is_custom": item.is_custom,
            }));
        }
        result.insert(key.to_string(), serde_json::Value::Array(versions));
    }

    serde_json::to_value(result).map_err(|e| format!("serialize failed: {e}"))
}

/// 验证指定的版本是否存在（清单或自定义）
#[tauri::command]
pub fn validate_version(service_type: String, version: String) -> Result<bool, String> {
    let project_root = get_project_root()?;
    let manager = UserOverrideManager::new(&project_root);
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("unsupported service type: {service_type}")),
    };

    Ok(manager.is_id_valid(&vm_service_type, &version))
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
        _ => return Err(format!("unsupported service type: {service_type}")),
    };

    Ok(manifest
        .get_recommended_entry(&vm_service_type)
        .map(|(id, _)| id.to_string()))
}

/// 保存用户自定义版本覆盖（仅已有清单 ID）
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
        _ => return Err(format!("unsupported service type: {service_type}")),
    };

    manager.save_user_override(
        &project_root,
        vm_service_type,
        id,
        image_tag,
        description,
    )
}

/// 新增完整自定义版本映射
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_custom_version(
    service_type: String,
    id: String,
    display_name: String,
    image_tag: String,
    service_dir: String,
    default_port: u16,
    show_port: bool,
    eol: bool,
    description: Option<String>,
) -> Result<(), String> {
    use crate::engine::version_manifest::VersionEntry;

    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);

    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("unsupported service type: {service_type}")),
    };

    manager.add_custom_version(
        &project_root,
        vm_service_type,
        id,
        VersionEntry {
            display_name,
            image_tag,
            service_dir,
            default_port,
            show_port,
            eol,
            description,
        },
    )
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
        _ => return Err(format!("unsupported service type: {service_type}")),
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

/// 把完整文件日志导出到用户指定位置
///
/// 与 `export_logs`（返回文本内容，供复制）不同，这里直接落盘，
/// 对应日志面板的"导出"动作。
#[tauri::command]
pub fn export_logs_to(dest: String) -> Result<(), String> {
    let log_path = get_log_file()?;

    if !log_path.exists() {
        return Err("log file not found; run some operations first".to_string());
    }

    std::fs::copy(&log_path, &dest).map_err(|e| format!("failed to export log: {e}"))?;

    Ok(())
}

/// 导出当前会话日志
///
/// 日志文件位于应用数据目录（路径由 `paths::log_file()` 统一给出）。
#[tauri::command]
pub fn export_logs() -> Result<String, String> {
    let log_path = get_log_file()?;

    if !log_path.exists() {
        return Err("log file not found; run some operations first".to_string());
    }

    std::fs::read_to_string(&log_path).map_err(|e| format!("failed to read log: {e}"))
}
