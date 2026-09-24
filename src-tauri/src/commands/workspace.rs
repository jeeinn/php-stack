use crate::app_log;
use crate::engine::service_catalog::{
    self, build_custom_descriptor, default_custom_version_id, normalize_service_kind,
    CustomServiceForm, ServiceCatalog, ServiceDescriptor,
};
use crate::engine::user_override_manager::UserOverrideManager;
use crate::engine::version_manifest::{VersionEntry, VersionManifest};
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
    use std::collections::{HashMap, HashSet};

    let project_root = get_project_root()?;
    let override_manager = UserOverrideManager::new(&project_root);
    let manifest = VersionManifest::new();
    let catalog = ServiceCatalog::merged(&project_root);

    let mut kinds: HashSet<String> = HashSet::new();
    for k in catalog.kinds() {
        kinds.insert(k);
    }
    for k in manifest.service_kinds() {
        kinds.insert(k);
    }
    for k in override_manager.service_kinds() {
        kinds.insert(k);
    }

    let mut kind_list: Vec<String> = kinds.into_iter().collect();
    kind_list.sort();

    let mut result = HashMap::new();
    for key in kind_list {
        let mut versions = Vec::new();
        for item in override_manager.list_merged_entries(&key) {
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
        result.insert(key, serde_json::Value::Array(versions));
    }

    serde_json::to_value(result).map_err(|e| format!("serialize failed: {e}"))
}

/// 验证指定的版本是否存在（清单或自定义）
#[tauri::command]
pub fn validate_version(service_type: String, version: String) -> Result<bool, String> {
    let project_root = get_project_root()?;
    let manager = UserOverrideManager::new(&project_root);
    let kind = normalize_service_kind(&service_type);
    Ok(manager.is_id_valid(&kind, &version))
}

/// 获取推荐版本
#[tauri::command]
pub fn get_recommended_version(service_type: String) -> Result<Option<String>, String> {
    let manifest = VersionManifest::new();
    let kind = normalize_service_kind(&service_type);
    Ok(manifest
        .get_recommended_entry(&kind)
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
    manager.save_user_override(&project_root, &service_type, id, image_tag, description)
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

    manager.add_custom_version(
        &project_root,
        &service_type,
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
    manager.remove_user_override(&project_root, &service_type, &id)
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

/// 返回合并后的服务目录（内置 + 工作区自定义）
#[tauri::command]
pub fn get_service_catalog() -> Result<Vec<ServiceDescriptor>, String> {
    let project_root = get_project_root()?;
    let catalog = ServiceCatalog::merged(&project_root);
    Ok(catalog.list().to_vec())
}

fn ensure_custom_version_for_service(
    project_root: &std::path::Path,
    descriptor: &ServiceDescriptor,
    image_tag: &str,
    version_id: Option<&str>,
    host_port: u16,
) -> Result<String, String> {
    let vid = default_custom_version_id(&descriptor.id, version_id);
    // 与服务 id 同一字符集（含 `-`）
    service_catalog::validate_custom_id(&vid)?;

    let mut manager = UserOverrideManager::new(project_root);

    // 已有 custom：更新 image_tag / default_port 并持久化
    if manager.is_custom_entry(&descriptor.id, &vid) {
        manager.upsert_custom_version_fields(
            project_root,
            &descriptor.id,
            &vid,
            image_tag,
            host_port,
            None,
        )?;
        return Ok(vid);
    }

    // 仅存在于清单 / override：不覆盖内置，原样返回
    if manager.is_id_valid(&descriptor.id, &vid) {
        return Ok(vid);
    }

    manager.add_custom_version(
        project_root,
        &descriptor.id,
        vid.clone(),
        VersionEntry {
            display_name: format!("{} ({})", descriptor.display_name, vid),
            image_tag: image_tag.to_string(),
            service_dir: vid.clone(),
            default_port: host_port,
            show_port: true,
            eol: false,
            description: Some(format!("Auto-created for custom service {}", descriptor.id)),
        },
    )?;
    Ok(vid)
}

fn upsert_custom_service(
    form: CustomServiceForm,
    require_existing: bool,
) -> Result<ServiceDescriptor, String> {
    let project_root = get_project_root()?;
    let host_port = form.host_port;
    let image_tag = form.image_tag.clone();
    let version_id = form.version_id.clone();
    let mut descriptor = build_custom_descriptor(&form)?;

    let mut catalog = ServiceCatalog::merged(&project_root);
    if require_existing {
        match catalog.get(&descriptor.id) {
            Some(existing) if existing.builtin => {
                return Err(format!(
                    "cannot update builtin service '{}'",
                    descriptor.id
                ));
            }
            Some(_) => {}
            None => {
                return Err(format!(
                    "custom service '{}' not found; use save_custom_service",
                    descriptor.id
                ));
            }
        }
    } else if let Some(existing) = catalog.get(&descriptor.id) {
        if existing.builtin {
            return Err(format!(
                "service id '{}' conflicts with builtin",
                descriptor.id
            ));
        }
        // 已有自定义：走更新语义
    }

    catalog.insert_custom(descriptor.clone())?;
    service_catalog::persist_custom_from_catalog(&project_root, &catalog)?;

    let vid = ensure_custom_version_for_service(
        &project_root,
        &descriptor,
        &image_tag,
        version_id.as_deref(),
        host_port,
    )?;
    app_log!(
        info,
        "commands::custom_service",
        "Saved custom service {} (version {})",
        descriptor.id,
        vid
    );
    // 回读合并结果，保证返回与磁盘一致
    descriptor = ServiceCatalog::merged(&project_root)
        .get(&descriptor.id)
        .cloned()
        .ok_or_else(|| "failed to reload saved custom service".to_string())?;
    Ok(descriptor)
}

/// 新建自定义服务（写入 custom_services.json，并确保有默认版本映射）
#[tauri::command]
pub fn save_custom_service(form: CustomServiceForm) -> Result<ServiceDescriptor, String> {
    upsert_custom_service(form, false)
}

/// 更新已有非内置自定义服务
#[tauri::command]
pub fn update_custom_service(form: CustomServiceForm) -> Result<ServiceDescriptor, String> {
    upsert_custom_service(form, true)
}

/// 删除自定义服务（仅从 catalog 移除；不删 data/services 目录）
#[tauri::command]
pub fn remove_custom_service(id: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut catalog = ServiceCatalog::merged(&project_root);
    catalog.remove_custom(&id)?;
    service_catalog::persist_custom_from_catalog(&project_root, &catalog)?;
    // 同步清除该 kind 的 version_overrides，避免 ghost kinds
    let mut override_manager = UserOverrideManager::new(&project_root);
    override_manager.remove_kind(&project_root, &id)?;
    app_log!(
        info,
        "commands::custom_service",
        "Removed custom service {id} (data dirs kept)"
    );
    Ok(())
}
