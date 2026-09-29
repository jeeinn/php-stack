//! Docker 镜像 tar 导入导出命令

use crate::engine::image_transfer::{
    ImageImportResult, ImageTransferEngine, WorkspaceImageEntry,
};

use super::env_config::load_existing_config;
use super::get_project_root;

/// 列出当前工作区相关镜像（含本地存在性与角色）
#[tauri::command]
pub fn list_workspace_images() -> Result<Vec<WorkspaceImageEntry>, String> {
    let project_root = get_project_root()?;
    let Some(config) = load_existing_config()? else {
        return Ok(Vec::new());
    };
    ImageTransferEngine::list_workspace_images(&project_root, &config)
}

/// 导出工作区镜像到 .tar（旁路写入同名 .manifest.json）
#[tauri::command]
pub async fn export_workspace_images(
    save_path: String,
    selected_refs: Option<Vec<String>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    let Some(config) = load_existing_config()? else {
        return Err("no workspace config; apply config first".into());
    };

    let handle = tokio::task::spawn_blocking(move || {
        ImageTransferEngine::export_images(
            &project_root,
            &config,
            &save_path,
            selected_refs,
            Some(&app_handle),
        )
    });

    handle
        .await
        .map_err(|e| format!("export images task failed: {e}"))?
}

/// 从 .tar 导入镜像
#[tauri::command]
pub async fn import_workspace_images(
    tar_path: String,
    app_handle: tauri::AppHandle,
) -> Result<ImageImportResult, String> {
    let handle = tokio::task::spawn_blocking(move || {
        ImageTransferEngine::import_images(&tar_path, Some(&app_handle))
    });

    handle
        .await
        .map_err(|e| format!("import images task failed: {e}"))?
}
