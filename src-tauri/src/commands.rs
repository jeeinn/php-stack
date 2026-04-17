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

/// 读取现有配置文件并解析为 EnvConfig
#[tauri::command]
pub fn load_existing_config() -> Result<Option<EnvConfig>, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    let compose_path = project_root.join("docker-compose.yml");
    
    // 如果两个文件都不存在，返回 None
    if !env_path.exists() || !compose_path.exists() {
        return Ok(None);
    }
    
    // 读取 .env 文件
    let env_content = std::fs::read_to_string(&env_path)
        .map_err(|e| format!("读取 .env 文件失败: {}", e))?;
    let env_file = super::engine::env_parser::EnvFile::parse(&env_content)
        .map_err(|e| format!("解析 .env 文件失败: {}", e))?;
    let env_map = env_file.to_map();
    
    // 解析服务配置
    let mut services: Vec<crate::engine::config_generator::ServiceEntry> = Vec::new();
    
    // 解析 PHP 服务（支持多版本）
    // 查找所有 PHPxx_VERSION 格式的键
    for (key, value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("PHP") {
            // 提取版本号部分，如 PHP56_VERSION -> 56
            let ver_part = &key[3..key.len() - 8]; // 去掉 "PHP" 和 "_VERSION"
            
            // 跳过纯数字的（这些是版本号的一部分，如 PHP56）
            if ver_part.is_empty() {
                continue;
            }
            
            let version = value.clone();
            let port_key = format!("PHP{}_HOST_PORT", ver_part);
            let ext_key = format!("PHP{}_EXTENSIONS", ver_part);
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(9000);
            
            let extensions = env_map.get(&ext_key)
                .map(|exts| exts.split(',').map(|s| s.trim().to_string()).collect());
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::PHP,
                version,
                host_port,
                extensions,
            });
        }
    }
    
    // 解析 MySQL 服务（支持多版本）
    // 查找所有 MYSQLxx_VERSION 或 MYSQL_VERSION 格式的键
    let mut mysql_index = 0;
    for (key, value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("MYSQL") && !key.contains("ROOT") {
            let version = value.clone();
            
            // 提取索引部分，如 MYSQL1_VERSION -> 1, MYSQL_VERSION -> 0
            let index_part = &key[5..key.len() - 8]; // 去掉 "MYSQL" 和 "_VERSION"
            let idx = if index_part.is_empty() {
                0
            } else {
                index_part.parse::<usize>().unwrap_or(mysql_index)
            };
            
            let port_key = if idx == 0 {
                "MYSQL_HOST_PORT".to_string()
            } else {
                format!("MYSQL{}_HOST_PORT", idx)
            };
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(3306 + idx as u16);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::MySQL,
                version,
                host_port,
                extensions: None,
            });
            
            mysql_index += 1;
        }
    }
    
    // 解析 Redis 服务
    if let Some(version) = env_map.get("REDIS_VERSION") {
        let host_port = env_map.get("REDIS_HOST_PORT")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(6379);
        
        services.push(crate::engine::config_generator::ServiceEntry {
            service_type: crate::engine::config_generator::ServiceType::Redis,
            version: version.clone(),
            host_port,
            extensions: None,
        });
    }
    
    // 解析 Nginx 服务
    if let Some(version) = env_map.get("NGINX_VERSION") {
        let host_port = env_map.get("NGINX_HTTP_HOST_PORT")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(80);
        
        services.push(crate::engine::config_generator::ServiceEntry {
            service_type: crate::engine::config_generator::ServiceType::Nginx,
            version: version.clone(),
            host_port,
            extensions: None,
        });
    }
    
    // 如果没有解析到任何服务，返回 None
    if services.is_empty() {
        return Ok(None);
    }
    
    let source_dir = env_map.get("SOURCE_DIR")
        .cloned()
        .unwrap_or_else(|| "./www".to_string());
    let timezone = env_map.get("TZ")
        .cloned()
        .unwrap_or_else(|| "Asia/Shanghai".to_string());
    
    Ok(Some(EnvConfig {
        services,
        source_dir,
        timezone,
    }))
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
pub async fn apply_env_config(config: EnvConfig, app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    
    // 辅助函数：发送日志到前端并打印到终端
    let emit_log = |msg: &str| {
        eprintln!("[APPLY_CONFIG] {}", msg); // 终端输出
        let _ = app_handle.emit("env-log", msg); // 前端UI显示
    };
    
    emit_log("📝 开始应用配置...");
    
    let project_root = get_project_root()?;
    emit_log(&format!("📁 目标目录: {:?}", project_root));
    
    emit_log("🔧 生成配置文件和目录结构...");
    
    match ConfigGenerator::apply(&config, &project_root).await {
        Ok(()) => {
            emit_log("✅ 配置应用成功！");
            Ok(())
        }
        Err(e) => {
            emit_log(&format!("❌ 配置应用失败: {}", e));
            Err(e)
        }
    }
}

/// 一键启动环境（docker compose up -d）
#[tauri::command]
pub async fn start_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;
    use tauri::Emitter;
    
    // 辅助函数：发送日志到前端并打印到终端
    let emit_log = |msg: &str| {
        eprintln!("[START_ENV] {}", msg); // 终端输出
        let _ = app_handle.emit("env-log", msg); // 前端UI显示
    };
    
    emit_log("🚀 开始启动环境...");
    
    let project_root = get_project_root()?;
    emit_log(&format!("📁 项目根目录: {:?}", project_root));
    
    let compose_file = project_root.join("docker-compose.yml");
    
    if !compose_file.exists() {
        emit_log("❌ docker-compose.yml 文件不存在");
        return Err("docker-compose.yml 文件不存在，请先应用配置".to_string());
    }
    
    emit_log("✅ docker-compose.yml 存在");
    emit_log("🔧 执行: docker compose up -d");
    emit_log("⏳ 这可能需要几分钟时间（首次构建需下载镜像和安装扩展）...");
    
    // 执行 docker compose up -d
    let output = Command::new("docker")
        .args(&["compose", "up", "-d", "--build"])
        .current_dir(&project_root)
        .output()
        .map_err(|e| {
            let err_msg = format!("执行 docker compose 失败: {}", e);
            emit_log(&format!("❌ {}", err_msg));
            err_msg
        })?;
    
    // 输出 stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        emit_log("📤 Docker Compose 输出:");
        for line in stdout.lines() {
            emit_log(&format!("   {}", line));
        }
    }
    
    // 输出 stderr（警告信息）
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        emit_log("⚠️ Docker Compose 警告/错误:");
        for line in stderr.lines() {
            emit_log(&format!("   {}", line));
        }
    }
    
    if output.status.success() {
        emit_log("✅ 环境启动成功！");
        Ok(stdout.to_string())
    } else {
        let err_msg = format!("Docker Compose 启动失败:\n{}", stderr);
        emit_log(&format!("❌ {}", err_msg));
        Err(err_msg)
    }
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
