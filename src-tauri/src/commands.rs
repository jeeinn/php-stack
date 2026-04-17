use crate::docker::manager::{DockerManager, PsContainer};
use crate::docker::mirror::MirrorManager;
use crate::engine::export::{ExportEngine, ExportOptions};
use crate::engine::software_manager::{
    SoftwareManager, SoftwareSpec, SoftwareType, InstalledSoftware, SoftwareVersion, PortAllocator,
};
use crate::engine::config_generator::{ConfigGenerator, EnvConfig};
use crate::engine::mirror_manager::{MirrorManager as UnifiedMirrorManager, MirrorPreset};
use crate::engine::backup_engine::BackupEngine;
use crate::engine::backup_manifest::BackupOptions;
use crate::engine::restore_engine::{RestoreEngine, RestorePreview};

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

/// 迁移已有容器到统一网络
#[tauri::command]
pub async fn migrate_containers_to_network() -> Result<String, String> {
    use crate::engine::software_manager::SoftwareManager;
    
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    let mut migrated = 0;
    let mut errors = Vec::new();
    
    for container in containers {
        match manager.migrate_container_to_network(&container.name).await {
            Ok(_) => migrated += 1,
            Err(e) => errors.push(format!("{}: {}", container.name, e)),
        }
    }
    
    if errors.is_empty() {
        Ok(format!("✅ 成功迁移 {} 个容器到统一网络", migrated))
    } else {
        Ok(format!(
            "⚠️ 迁移完成：{} 个成功，{} 个失败\n失败详情:\n{}",
            migrated,
            errors.len(),
            errors.join("\n")
        ))
    }
}

/// 【调试命令】手动重建 docker-compose.yml 文件
#[tauri::command]
pub async fn rebuild_compose_file() -> Result<String, String> {
    use crate::engine::software_manager::SoftwareManager;
    
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    log::info!("🔧 手动触发 docker-compose.yml 重建，当前容器数: {}", containers.len());
    
    // 获取 ComposeManager 并重建文件
    let compose_manager = manager.get_compose_manager();
    compose_manager.rebuild_from_containers(&containers).await
        .map_err(|e| format!("重建失败: {}", e))?;
    
    let compose_path = compose_manager.get_compose_path();
    Ok(format!("✅ docker-compose.yml 已重建: {}", compose_path))
}

/// 【Phase 3】分析服务修改的影响范围
#[tauri::command]
pub async fn analyze_restart_impact(
    service_name: String,
) -> Result<RestartImpactResult, String> {
    use crate::engine::software_manager::SoftwareManager;
    
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    let compose_manager = manager.get_compose_manager();
    let impact = compose_manager.analyze_restart_impact(&service_name, &containers);
    
    // 转换为可序列化的结果
    Ok(RestartImpactResult {
        services_to_restart: impact.services_to_restart,
        dependency_chain: impact.dependency_chain,
        total_affected: impact.total_affected,
    })
}

/// 【Phase 3】执行智能重启
#[tauri::command]
pub async fn smart_restart_service(
    service_name: String,
) -> Result<RestartImpactResult, String> {
    use crate::engine::software_manager::SoftwareManager;
    
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    let compose_manager = manager.get_compose_manager();
    let impact = compose_manager.smart_restart_with_analysis(&service_name, &containers).await
        .map_err(|e| format!("智能重启失败: {}", e))?;
    
    Ok(RestartImpactResult {
        services_to_restart: impact.services_to_restart,
        dependency_chain: impact.dependency_chain,
        total_affected: impact.total_affected,
    })
}

/// 重启影响分析结果（可序列化版本）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RestartImpactResult {
    pub services_to_restart: Vec<String>,
    pub dependency_chain: Vec<String>,
    pub total_affected: usize,
}

/// 【Phase 4】读取 docker-compose.yml 文件内容
#[tauri::command]
pub async fn read_compose_file() -> Result<String, String> {
    use std::fs;
    use crate::engine::software_manager::SoftwareManager;
    
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let compose_manager = manager.get_compose_manager();
    let compose_path = compose_manager.get_compose_path();
    
    let content = fs::read_to_string(compose_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    
    Ok(content)
}

// ==================== V2.0: 环境构建器命令 ====================

use crate::engine::mirror_config::MirrorConfig;
use crate::engine::environment_builder::{EnvironmentSpec, CompatibilityChecker, EnvironmentBuilder};

/// 获取当前镜像源配置
#[tauri::command]
pub async fn get_mirror_config() -> Result<MirrorConfig, String> {
    MirrorConfig::load_from_env()
}

/// 更新镜像源配置
#[tauri::command]
pub async fn update_mirror_config(config: MirrorConfig) -> Result<(), String> {
    config.save_to_env()?;
    log::info!("✅ 镜像源配置已更新");
    Ok(())
}

/// 测试镜像源连接
#[tauri::command]
pub async fn test_mirror_connection(source: crate::engine::mirror_config::MirrorSource) -> Result<bool, String> {
    // 尝试访问镜像源
    let url = source.get_url("apt");
    if url.is_empty() {
        return Ok(true); // Default source
    }
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    let response = client.get(&url).send().await;
    
    Ok(response.is_ok())
}

/// 验证环境规格
#[tauri::command]
pub async fn validate_environment_spec(spec: EnvironmentSpec) -> Result<bool, String> {
    CompatibilityChecker::validate(&spec)?;
    Ok(true)
}

/// 生成 docker-compose 配置（预览，不包含镜像构建）
#[tauri::command]
pub async fn generate_compose_preview(spec: EnvironmentSpec) -> Result<String, String> {
    let builder = EnvironmentBuilder::new();
    let compose = builder.generate_compose(&spec).await?;
    
    // 序列化为 YAML
    let yaml = serde_yaml::to_string(&compose)
        .map_err(|e| format!("序列化 YAML 失败: {}", e))?;
    
    Ok(yaml)
}

/// 生成并应用 docker-compose 配置（包含镜像构建）
#[tauri::command]
pub async fn deploy_environment_with_build(spec: EnvironmentSpec) -> Result<String, String> {
    use crate::engine::compose_manager::ComposeManager;
    
    log::info!("🚀 开始部署环境...");
    
    // 1. 验证环境规格
    CompatibilityChecker::validate(&spec)?;
    log::info!("✅ 环境规格验证通过");
    
    // 2. 生成 compose 配置（包含镜像构建）
    let builder = EnvironmentBuilder::new();
    let compose = builder.generate_compose_with_build(&spec).await?;
    log::info!("✅ Compose 配置生成成功");
    
    // 3. 写入 docker-compose.yml
    let compose_manager = ComposeManager::new(".");
    compose_manager.save_compose_file(&compose)
        .map_err(|e| format!("保存 compose 文件失败: {}", e))?;
    log::info!("✅ docker-compose.yml 已保存");
    
    // 4. 应用配置（启动容器）
    compose_manager.apply_changes().await
        .map_err(|e| format!("应用配置失败: {}", e))?;
    log::info!("✅ 环境部署成功");
    
    Ok("环境部署成功".to_string())
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
pub async fn apply_env_config(config: EnvConfig) -> Result<(), String> {
    ConfigGenerator::apply(&config, &std::path::Path::new(".")).await
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
    let env_path = std::path::Path::new(".env");
    UnifiedMirrorManager::apply_preset(&preset_name, env_path)
}

/// 更新单个镜像源类别
#[tauri::command]
pub fn update_single_mirror(category: String, source: String) -> Result<(), String> {
    let env_path = std::path::Path::new(".env");
    UnifiedMirrorManager::update_single(&category, &source, env_path)
}

/// 测试镜像源连接（3秒超时）
#[tauri::command]
pub async fn test_mirror(url: String) -> Result<bool, String> {
    UnifiedMirrorManager::test_connection(&url).await
}

/// 获取当前镜像源状态
#[tauri::command]
pub fn get_mirror_status() -> Result<serde_json::Value, String> {
    let env_path = std::path::Path::new(".env");
    let status = UnifiedMirrorManager::get_current_status(env_path)?;
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

    // Use spawn to handle the non-Send future from BackupEngine
    let handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            BackupEngine::create_backup(
                &save_path_clone,
                options_clone,
                &std::path::Path::new("."),
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
    let result = RestoreEngine::restore(
        &zip_path,
        &std::path::Path::new("."),
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
