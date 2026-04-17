use crate::docker::manager::{DockerManager, PsContainer};
use crate::engine::config_generator::{ConfigGenerator, EnvConfig};
use crate::engine::mirror_manager::{MirrorManager as UnifiedMirrorManager, MirrorPreset};
use crate::engine::backup_engine::BackupEngine;
use crate::engine::backup_manifest::BackupOptions;
use crate::engine::restore_engine::{RestoreEngine, RestorePreview};

/// 获取项目根目录（用于配置文件生成）
fn get_project_root() -> Result<std::path::PathBuf, String> {
    if cfg!(debug_assertions) {
        // 开发模式：使用项目根目录（src-tauri 的父目录）
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {}", e))?
            .parent() // target/debug/
            .and_then(|p| p.parent()) // target/
            .and_then(|p| p.parent()) // src-tauri/
            .and_then(|p| p.parent()) // 项目根目录
            .ok_or("无法获取项目根目录")?
            .to_path_buf())
    } else {
        // 生产模式：使用可执行文件所在目录
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {}", e))?
            .parent()
            .ok_or("无法获取程序所在目录")?
            .to_path_buf())
    }
}

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

// ==================== 可视化配置生成命令 ====================

/// 验证 EnvConfig（端口冲突检测等）
#[tauri::command]
pub fn validate_env_config(config: EnvConfig) -> Result<(), String> {
    ConfigGenerator::validate(&config)
}

/// 生成 .env 文件内容预览
#[tauri::command]
pub fn generate_env_config(config: EnvConfig) -> Result<String, String> {
    let env_file = ConfigGenerator::generate_env(&config, None);
    Ok(env_file.format())
}

/// 预览 docker-compose.yml 内容
#[tauri::command]
pub fn preview_compose(config: EnvConfig) -> Result<String, String> {
    ConfigGenerator::validate(&config)?;
    Ok(ConfigGenerator::generate_compose(&config))
}

/// 应用配置（写入 .env、docker-compose.yml、创建目录）
#[tauri::command]
pub async fn apply_env_config(config: EnvConfig, _app_handle: tauri::AppHandle) -> Result<(), String> {
    let project_root = get_project_root()?;
    ConfigGenerator::apply(&config, &project_root).await
}

// ==================== 统一镜像源管理命令 ====================

/// 获取所有镜像源预设方案
#[tauri::command]
pub fn get_mirror_presets() -> Result<Vec<MirrorPreset>, String> {
    Ok(UnifiedMirrorManager::get_presets())
}

/// 应用镜像源预设方案
#[tauri::command]
pub async fn apply_mirror_preset(preset_name: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::apply_preset(&preset_name, &env_path)
}

/// 更新单个镜像源类别
#[tauri::command]
pub fn update_single_mirror(category: String, source: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::update_single(&category, &source, &env_path)
}

/// 测试镜像源连接（3秒超时）
#[tauri::command]
pub async fn test_mirror(url: String) -> Result<bool, String> {
    UnifiedMirrorManager::test_connection(&url).await
}

/// 获取当前镜像源状态
#[tauri::command]
pub fn get_mirror_status() -> Result<serde_json::Value, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    let status = UnifiedMirrorManager::get_current_status(&env_path)?;
    serde_json::to_value(&status)
        .map_err(|e| format!("序列化镜像源状态失败: {}", e))
}

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

    handle.await.map_err(|e| format!("备份任务执行失败: {}", e))?
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
    port_overrides: std::collections::HashMap<String, u16>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    let result = RestoreEngine::restore(
        &zip_path,
        &project_root,
        port_overrides,
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
