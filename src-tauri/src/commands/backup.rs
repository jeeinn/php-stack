use crate::app_log;
use crate::engine::backup_engine::BackupEngine;
use crate::engine::backup_manifest::BackupOptions;
use crate::engine::restore_engine::{RestoreEngine, RestorePreview, RestoreResult};

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
///
/// 始终返回 `RestoreResult`（U2）：成功、部分失败、致命失败（包打不开 / zip-slip 等）
/// 都走 `Ok`，把错误明细与回滚包路径交给前端结果面板。
/// 仅工作区路径等命令前置失败才走 `Err`。
#[tauri::command]
pub async fn execute_restore(
    zip_path: String,
    app_handle: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    let project_root = get_project_root()?;

    // R2: 恢复是逐文件覆盖的破坏性操作，开始前先打一份回滚包
    let rollback_path = create_rollback_bundle(&project_root).await;

    let mut result = match RestoreEngine::restore(&zip_path, &project_root, Some(&app_handle)).await
    {
        Ok(r) => r,
        // 致命错误也结构化返回：前端可渲染错误面板 + 回滚快捷入口，
        // 不再把多行文案塞进 toast。
        Err(e) => RestoreResult {
            success: false,
            restored_files: Vec::new(),
            errors: vec![e],
            rollback_path: None,
        },
    };

    result.rollback_path = rollback_path;
    Ok(result)
}

/// 恢复前把即将被覆盖的配置文件打包成回滚包（R2）
///
/// 写入项目根目录 `.restore_rollback_<时间戳>.zip`，只打包配置文件
/// （不含项目文件与日志），保证体积小、回滚快。
/// 失败不阻断恢复——回滚包是兜底手段，没有它也允许用户继续。
async fn create_rollback_bundle(project_root: &std::path::Path) -> Option<String> {
    // 首次恢复场景：没有任何现有配置，不存在覆盖风险，跳过
    let has_existing = project_root.join(".env").exists()
        || project_root.join("docker-compose.yml").exists()
        || project_root.join("services").exists();
    if !has_existing {
        return None;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let save_path = project_root.join(format!(".restore_rollback_{timestamp}.zip"));
    let options = BackupOptions {
        include_projects: false,
        project_patterns: Vec::new(),
        include_logs: false,
    };

    match BackupEngine::create_backup(&save_path.to_string_lossy(), options, project_root, None)
        .await
    {
        Ok(()) => {
            app_log!(
                info,
                "commands::execute_restore",
                "已生成恢复前回滚包: {}",
                save_path.display()
            );
            Some(save_path.to_string_lossy().to_string())
        }
        Err(e) => {
            app_log!(
                warn,
                "commands::execute_restore",
                "生成回滚包失败（不阻断恢复）: {e}"
            );
            None
        }
    }
}

/// 将绝对路径转换为相对于项目根目录的路径
#[tauri::command]
pub fn convert_to_relative_path(
    absolute_path: String,
    is_directory: bool,
) -> Result<String, String> {
    let project_root = get_project_root()?;
    let abs_path = std::path::PathBuf::from(&absolute_path);

    // 使用 pathdiff 计算相对路径，它会自动处理跨平台差异（如 Windows 盘符）
    match pathdiff::diff_paths(&abs_path, &project_root) {
        Some(relative) if relative.as_os_str().is_empty() || relative.as_os_str() == "." => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// `create_rollback_bundle` 是 async，但内部只做文件 IO，
    /// 用当前线程 runtime block 住即可。
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("创建 tokio runtime")
            .block_on(fut)
    }

    #[test]
    fn test_create_rollback_bundle_skips_empty_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let result = block_on(create_rollback_bundle(dir.path()));
        assert!(result.is_none(), "没有任何现有配置时不应生成回滚包");
    }

    #[test]
    fn test_create_rollback_bundle_creates_valid_zip() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".env"), "PHP82_VERSION=8.2\n").unwrap();

        let result = block_on(create_rollback_bundle(dir.path()));
        let path = result.expect("存在 .env 时应生成回滚包");

        assert!(std::path::Path::new(&path).exists(), "回滚包文件应存在");
        assert!(
            path.contains(".restore_rollback_"),
            "回滚包应带可识别的前缀，实际: {path}"
        );

        // 回滚包必须是合法 ZIP——否则用户真要回滚时才发现打不开
        let file = fs::File::open(&path).expect("回滚包应可读");
        let archive = zip::ZipArchive::new(file).expect("回滚包应是合法 ZIP");
        assert!(!archive.is_empty(), "回滚包不应为空");
    }
}
