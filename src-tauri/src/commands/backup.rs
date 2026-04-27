use crate::engine::backup_engine::BackupEngine;
use crate::engine::backup_manifest::BackupOptions;
use crate::engine::restore_engine::{RestoreEngine, RestorePreview};

use super::get_project_root;

// ==================== 备份命令 ====================

/// 创建环境备份
#[tauri::command]
pub async fn create_backup(
    save_path: String,
    options: BackupOptions,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Clone values for the spawned task
    let save_path_clone = save_path.clone();
    let options_clone = options.clone();
    let app_handle_clone = app_handle.clone();
    let project_root = get_project_root()?;

    // Use spawn to handle the non-Send future from BackupEngine
    let handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            BackupEngine::create_backup(
                &save_path_clone,
                options_clone,
                &project_root,
                Some(&app_handle_clone),
            )
            .await
        })
    });

    handle.await.map_err(|e| format!("备份任务执行失败: {e}"))?
}

// ==================== 恢复命令 ====================

/// 预览备份包内容
#[tauri::command]
pub fn preview_restore(zip_path: String) -> Result<RestorePreview, String> {
    RestoreEngine::preview(&zip_path)
}

/// 验证备份包完整性
#[tauri::command]
pub fn verify_backup(zip_path: String) -> Result<bool, String> {
    RestoreEngine::verify_integrity(&zip_path)
}

/// 执行环境恢复
#[tauri::command]
pub async fn execute_restore(
    zip_path: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    let result = RestoreEngine::restore(
        &zip_path,
        &project_root,
        Some(&app_handle),
    )
    .await?;

    if result.success {
        Ok(())
    } else {
        Err(format!(
            "恢复完成但存在错误:\n{}",
            result.errors.join("\n")
        ))
    }
}

/// 选择项目文件夹并转换为相对路径
#[tauri::command]
pub fn select_project_folder() -> Result<Option<String>, String> {
    // 实际的文件选择逻辑应该在前端通过 @tauri-apps/plugin-dialog 实现
    // 这里仅作为占位符
    Ok(None)
}

/// 将绝对路径转换为相对于项目根目录的路径
#[tauri::command]
pub fn convert_to_relative_path(absolute_path: String, is_directory: bool) -> Result<String, String> {
    let project_root = get_project_root()?;
    let abs_path = std::path::PathBuf::from(&absolute_path);
    
    // 使用 pathdiff 计算相对路径，它会自动处理跨平台差异（如 Windows 盘符）
    match pathdiff::diff_paths(&abs_path, &project_root) {
        Some(relative) if relative.as_os_str().is_empty() || relative == std::path::PathBuf::from(".") => {
            Err("不能选择项目根目录本身，请选择其子文件或子文件夹".to_string())
        }
        Some(relative) => {
            // 检查是否包含 ".." (即不在项目目录下)
            let rel_str = relative.to_string_lossy();
            if rel_str.starts_with("..") || rel_str.contains("/..") || rel_str.contains("\\..") {
                return Err(format!(
                    "所选路径不在项目根目录下。\n为了确保证跨平台恢复成功，建议您将配置文件移动到项目目录（如 www/ 或 configs/）下再进行备份。\n\n当前项目根目录: {}",
                    project_root.display()
                ));
            }
            
            // 统一转换为正斜杠
            let normalized = rel_str.replace('\\', "/");
            if is_directory {
                Ok(format!("{normalized}/**"))
            } else {
                Ok(normalized)
            }
        }
        None => Err("无法计算相对路径，请确保文件位于项目目录内".to_string()),
    }
}
