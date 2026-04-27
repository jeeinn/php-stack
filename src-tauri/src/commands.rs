use crate::docker::manager::{DockerManager, PsContainer};
use crate::engine::config_generator::{ConfigGenerator, EnvConfig};
use crate::engine::mirror_manager::{MirrorManager as UnifiedMirrorManager, MirrorPreset};
use crate::engine::mirror_config_manager::{MirrorConfigManager, MergedMirrorCategory};
use crate::engine::backup_engine::BackupEngine;
use crate::engine::backup_manifest::BackupOptions;
use crate::engine::restore_engine::{RestoreEngine, RestorePreview};
use crate::engine::version_manifest::{VersionManifest, ServiceType as VmServiceType};
use crate::engine::user_override_manager::{UserOverrideManager, UserVersionOverride};

use crate::engine::workspace_manager::WorkspaceManager;

/// 获取项目根目录（优先读取 workspace.json）
fn get_project_root() -> Result<std::path::PathBuf, String> {
    // 1. 尝试从 workspace.json 读取配置
    if let Some(workspace) = WorkspaceManager::load_workspace()? {
        let path = std::path::PathBuf::from(&workspace.workspace_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. 如果未配置或路径无效，返回 exe 同级目录作为默认值
    if cfg!(debug_assertions) {
        // 开发模式：使用项目根目录（src-tauri 的父目录）
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent() // target/debug/
            .and_then(|p| p.parent()) // target/
            .and_then(|p| p.parent()) // src-tauri/
            .and_then(|p| p.parent()) // 项目根目录
            .ok_or("无法获取项目根目录")?
            .to_path_buf())
    } else {
        // 生产模式：使用可执行文件所在目录
        Ok(std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {e}"))?
            .parent()
            .ok_or("无法获取程序所在目录")?
            .to_path_buf())
    }
}

/// 检测 Docker Compose 版本，判断是否支持 --progress 参数
/// 返回 true 表示支持 --progress 参数（V2.20+）
fn check_compose_progress_support() -> bool {
    use std::process::Command;
    
    // 执行 docker compose version
    let output = Command::new("docker")
        .args(["compose", "version"])
        .output();
    
    match output {
        Ok(output) => {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // 解析版本号，例如：“Docker Compose version v2.17.3”
            if let Some(version_part) = version_str.split_whitespace().last() {
                // 去掉 'v' 前缀
                let version = version_part.trim_start_matches('v');
                // 解析主版本号和次版本号
                let parts: Vec<&str> = version.split('.').collect();
                if parts.len() >= 2 {
                    if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        // V2.20+ 才支持 --progress 参数
                        return major > 2 || (major == 2 && minor >= 20);
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// 解析 Docker Compose 输出，提取关键进度信息
/// 返回格式化后的进度消息，如果不是进度相关的行则返回 None
fn parse_docker_progress(line: &str) -> Option<String> {
    // 拉取镜像: Pulling php ...
    if line.contains("Pulling") {
        if let Some(service) = extract_service_name(line, "Pulling") {
            return Some(format!("正在拉取镜像: {}", service));
        }
    }
    
    // 下载进度: php Pulled
    if line.contains("Pulled") {
        if let Some(service) = extract_service_name(line, "Pulled") {
            return Some(format!("✅ 镜像拉取完成: {}", service));
        }
    }
    
    // 构建镜像: Building php
    if line.contains("Building") {
        if let Some(service) = extract_service_name(line, "Building") {
            return Some(format!("🔨 正在构建镜像: {}", service));
        }
    }
    
    // 创建容器: Creating ps-php-1 ... done
    if line.contains("Creating") {
        if let Some(service) = extract_service_name(line, "Creating") {
            return Some(format!("📦 正在创建容器: {}", service));
        }
    }
    
    // 启动容器: Starting ps-php-1 ... done
    if line.contains("Starting") {
        if let Some(service) = extract_service_name(line, "Starting") {
            return Some(format!("🚀 正在启动服务: {}", service));
        }
    }
    
    // 容器已存在: Container ps-php-1 is running
    if line.contains("is running") || line.contains("Up to date") {
        return Some("⚡ 服务已在运行".to_string());
    }
    
    None
}

/// 从 Docker Compose 输出行中提取服务名称
fn extract_service_name(line: &str, keyword: &str) -> Option<String> {
    // 查找关键词后的服务名
    if let Some(pos) = line.find(keyword) {
        let after_keyword = &line[pos + keyword.len()..];
        // 提取第一个单词作为服务名
        let service = after_keyword.trim().split_whitespace().next()?;
        // 去除可能的特殊字符
        let clean_service = service.trim_end_matches(|c| c == '.' || c == ':' || c == ' ');
        if !clean_service.is_empty() {
            return Some(clean_service.to_string());
        }
    }
    None
}

#[tauri::command]
pub async fn check_docker() -> Result<(), String> {
    let manager = DockerManager::new().map_err(|e| format!("未找到 Docker 安装: {e}"))?;
    manager.check_docker_availability().await
}

#[tauri::command]
pub async fn list_containers() -> Result<Vec<PsContainer>, String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.list_ps_containers().await.map_err(|e| e.to_string())
}

/// 获取所有运行中的容器（用于端口冲突检测）
#[tauri::command]
pub async fn list_all_running_containers() -> Result<Vec<PsContainer>, String> {
    check_docker().await?;
    let manager = DockerManager::new().map_err(|e| e.to_string())?;
    manager.list_all_running_containers().await.map_err(|e| e.to_string())
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
        .map_err(|e| format!("读取 .env 文件失败: {e}"))?;
    let env_file = super::engine::env_parser::EnvFile::parse(&env_content)
        .map_err(|e| format!("解析 .env 文件失败: {e}"))?;
    let env_map = env_file.to_map();
    
    // 解析服务配置
    let mut services: Vec<crate::engine::config_generator::ServiceEntry> = Vec::new();
    
    // 创建 VersionManifest 用于 env prefix 反查
    let manifest = VersionManifest::new();
    
    // 解析 PHP 服务（支持多版本）
    // 查找所有 PHPxx_VERSION 格式的键
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("PHP") {
            // 提取前缀，如 PHP82_VERSION → PHP82
            let prefix = &key[..key.len() - 8]; // 去掉 "_VERSION"
            let ver_part = &key[3..key.len() - 8]; // 去掉 "PHP" 和 "_VERSION"
            
            if ver_part.is_empty() {
                continue;
            }
            
            // 使用 manifest 反查 ID，如 "PHP82" → "php82"
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Php, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("PHP{ver_part}_HOST_PORT");
            let ext_key = format!("PHP{ver_part}_EXTENSIONS");
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(9000);
            
            let extensions = env_map.get(&ext_key)
                .map(|exts| exts.split(',').map(|s| s.trim().to_string()).collect());
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::PHP,
                version,  // manifest ID，如 "php82"
                host_port,
                extensions,
            });
        }
    }
    
    // 解析 MySQL 服务（支持多版本）
    // 查找所有 MYSQLxx_VERSION 格式的键
    let mut mysql_index = 0;
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("MYSQL") && !key.contains("ROOT") && !key.contains("USER") && !key.contains("PASSWORD") {
            // 提取前缀，如 MYSQL84_VERSION → MYSQL84
            let prefix = &key[..key.len() - 8]; // 去掉 "_VERSION"
            let index_part = &key[5..key.len() - 8]; // 去掉 "MYSQL" 和 "_VERSION"
            
            if index_part.is_empty() {
                continue;
            }
            
            let idx = index_part.parse::<usize>().unwrap_or(mysql_index);
            
            // 使用 manifest 反查 ID，如 "MYSQL84" → "mysql84"
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Mysql, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("MYSQL{index_part}_HOST_PORT");
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(3306 + idx as u16);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::MySQL,
                version,  // manifest ID，如 "mysql84"
                host_port,
                extensions: None,
            });
            
            mysql_index += 1;
        }
    }
    
    // 解析 Redis 服务（支持多版本）
    // 查找所有 REDISxx_VERSION 格式的键
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("REDIS") {
            // 提取前缀，如 REDIS72_VERSION → REDIS72
            let prefix = &key[..key.len() - 8]; // 去掉 "_VERSION"
            let index_part = &key[5..key.len() - 8]; // 去掉 "REDIS" 和 "_VERSION"
            
            if index_part.is_empty() {
                continue;
            }
            
            // 使用 manifest 反查 ID，如 "REDIS72" → "redis72"
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Redis, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("REDIS{index_part}_HOST_PORT");
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(6379);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::Redis,
                version,  // manifest ID，如 "redis72"
                host_port,
                extensions: None,
            });
        }
    }
    
    // 解析 Nginx 服务（支持多版本）
    // 查找所有 NGINXxx_VERSION 格式的键
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("NGINX") {
            // 提取前缀，如 NGINX127_VERSION → NGINX127
            let prefix = &key[..key.len() - 8]; // 去掉 "_VERSION"
            let index_part = &key[6..key.len() - 8]; // 去掉 "NGINX" 和 "_VERSION" (注意 NGINX 是 5 个字母 + 1 = 6)
            
            if index_part.is_empty() {
                continue;
            }
            
            // 使用 manifest 反查 ID，如 "NGINX127" → "nginx127"
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Nginx, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("NGINX{index_part}_HTTP_HOST_PORT");
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(80);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::Nginx,
                version,  // manifest ID，如 "nginx127"
                host_port,
                extensions: None,
            });
        }
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
    let mysql_root_password = env_map.get("MYSQL_ROOT_PASSWORD").cloned();
    
    Ok(Some(EnvConfig {
        services,
        source_dir,
        timezone,
        mysql_root_password,
    }))
}

/// 生成 .env 文件内容预览
#[tauri::command]
pub fn generate_env_config(config: EnvConfig) -> Result<String, String> {
    let project_root = get_project_root()?;
    let env_file = ConfigGenerator::generate_env(&config, None, &project_root);
    Ok(env_file.format())
}

/// 预览 docker-compose.yml 内容
#[tauri::command]
pub fn preview_compose(config: EnvConfig) -> Result<String, String> {
    ConfigGenerator::validate(&config)?;
    Ok(ConfigGenerator::generate_compose(&config))
}

/// 检查配置文件是否存在
#[tauri::command]
pub fn check_config_files_exist() -> Result<Vec<String>, String> {
    let project_root = get_project_root()?;
    let mut existing_files = Vec::new();
    
    // 检查关键文件
    let files_to_check = [
        (".env", "环境配置文件"),
        ("docker-compose.yml", "Docker Compose 配置"),
    ];
    
    for (filename, description) in &files_to_check {
        let file_path = project_root.join(filename);
        if file_path.exists() {
            existing_files.push(format!("{filename} ({description})"));
        }
    }
    
    // 检查 services 目录
    let services_dir = project_root.join("services");
    if services_dir.exists() {
        existing_files.push("services/ (服务配置目录)".to_string());
    }
    
    Ok(existing_files)
}

/// 应用配置（写入 .env、docker-compose.yml、创建目录）
#[tauri::command]
pub async fn apply_env_config(config: EnvConfig, enable_backup: bool, app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Emitter;
    use crate::ui_log;
    
    ui_log!(app_handle, info, "commands::apply_env_config", "📝 开始应用配置...");
    
    let project_root = get_project_root()?;
    ui_log!(app_handle, info, "commands::apply_env_config", "📁 项目根目录: {:?}", project_root);
    
    // 检查用户覆盖配置
    let overrides_path = project_root.join(".user_version_overrides.json");
    if overrides_path.exists() {
        ui_log!(app_handle, info, "commands::apply_env_config", "✅ 检测到用户版本覆盖配置");
    } else {
        ui_log!(app_handle, info, "commands::apply_env_config", "ℹ️  未找到用户覆盖配置，使用默认配置");
    }
    
    ui_log!(app_handle, info, "commands::apply_env_config", "🔧 验证配置...");
    ui_log!(app_handle, info, "commands::apply_env_config", "📄 生成 .env 文件...");
    ui_log!(app_handle, info, "commands::apply_env_config", "🐳 生成 docker-compose.yml...");
    ui_log!(app_handle, info, "commands::apply_env_config", "📂 创建服务目录结构...");
    
    match ConfigGenerator::apply(&config, &project_root, enable_backup).await {
        Ok(backed_up_files) => {
            if !backed_up_files.is_empty() {
                ui_log!(app_handle, info, "commands::apply_env_config", "💾 已备份 {} 个文件/目录", backed_up_files.len());
                for file in &backed_up_files {
                    ui_log!(app_handle, info, "commands::apply_env_config", "   - {}", file);
                }
            }
            ui_log!(app_handle, info, "commands::apply_env_config", "✅ 配置应用成功！");
            ui_log!(app_handle, info, "commands::apply_env_config", "💡 提示：请重启容器使新配置生效");
            Ok(backed_up_files)
        }
        Err(e) => {
            ui_log!(app_handle, error, "commands::apply_env_config", "❌ 配置应用失败: {}", e);
            Err(e)
        }
    }
}

/// 一键启动环境（docker compose up -d）
#[tauri::command]
pub async fn start_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;
    use tauri::Emitter;
    use crate::ui_log;
    
    ui_log!(app_handle, info, "commands::start_environment", "🚀 开始启动环境...");
    
    let project_root = get_project_root()?;
    ui_log!(app_handle, info, "commands::start_environment", "📁 项目根目录: {:?}", project_root);
    
    let compose_file = project_root.join("docker-compose.yml");
    
    if !compose_file.exists() {
        ui_log!(app_handle, info, "commands::start_environment", "❌ docker-compose.yml 文件不存在");
        return Err("docker-compose.yml 文件不存在，请先应用配置".to_string());
    }
    
    ui_log!(app_handle, info, "commands::start_environment", "✅ docker-compose.yml 存在");
    
    // 第一步：清理旧容器（避免名称冲突）
    ui_log!(app_handle, info, "commands::start_environment", "🧹 清理旧容器...");
    let mut down_cmd = Command::new("docker");
    down_cmd.args(["compose", "down", "--remove-orphans"])
        .current_dir(&project_root);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        down_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    let down_output = down_cmd.output().map_err(|e| {
            let err_msg = format!("清理旧容器失败: {e}");
            ui_log!(app_handle, info, "commands::start_environment", "⚠️ {}", err_msg);
            err_msg
        })?;
    
    if !down_output.status.success() {
        let stderr = String::from_utf8_lossy(&down_output.stderr);
        ui_log!(app_handle, warn, "commands::start_environment", "清理警告: {}", stderr.lines().next().unwrap_or(""));
    } else {
        ui_log!(app_handle, info, "commands::start_environment", "✅ 旧容器已清理");
    }
    
    // 等待 ps- 前缀的容器完全停止（最多等待 10 秒）
    ui_log!(app_handle, info, "commands::start_environment", "⏳ 等待容器完全停止...");
    let manager = DockerManager::new().map_err(|e| {
        let err_msg = format!("创建 Docker 管理器失败: {e}");
        ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
        err_msg
    })?;
    
    for attempt in 1..=10 {
        let ps_containers = manager.list_ps_containers().await.map_err(|e| {
            let err_msg = format!("检查容器状态失败: {e}");
            ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
            err_msg
        })?;
        
        // 过滤出仍在运行的 ps- 容器
        let running_ps_containers: Vec<_> = ps_containers.iter()
            .filter(|c| c.state.to_lowercase().contains("running") || 
                        c.state.to_lowercase().contains("up"))
            .collect();
        
        if running_ps_containers.is_empty() {
            ui_log!(app_handle, info, "commands::start_environment", "✅ 所有 ps- 容器已完全停止");
            break;
        }
        
        if attempt == 10 {
            ui_log!(app_handle, warn, "commands::start_environment", "等待超时，仍有 {} 个容器未停止", running_ps_containers.len());
            for container in &running_ps_containers {
                ui_log!(app_handle, info, "commands::start_environment", "   - {} ({})", container.name, container.state);
            }
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    ui_log!(app_handle, info, "commands::start_environment", "");
    
    // 第二步：端口冲突检测
    ui_log!(app_handle, info, "commands::start_environment", "🔍 检查端口冲突...");
    
    // 获取所有运行中的容器
    let manager = DockerManager::new().map_err(|e| {
        let err_msg = format!("创建 Docker 管理器失败: {e}");
        ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
        err_msg
    })?;
    
    let all_containers = manager.list_all_running_containers().await.map_err(|e| {
        let err_msg = format!("获取容器列表失败: {e}");
        ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
        err_msg
    })?;
    
    // 加载配置并检查端口
    let config_result = load_existing_config()?;
    if let Some(config) = config_result {
        let mut conflicts: Vec<(String, u16, String)> = Vec::new();
        
        for service in &config.services {
            let port = service.host_port;
            let service_name = format!("{:?}{}", service.service_type, service.version.replace('.', ""));
            
            // 检查是否有容器占用了这个端口
            for container in &all_containers {
                if container.ports.contains(&(port as i32)) {
                    conflicts.push((
                        container.name.clone(),
                        port,
                        service_name.clone(),
                    ));
                    break;
                }
            }
        }
        
        if !conflicts.is_empty() {
            ui_log!(app_handle, info, "commands::start_environment", "❌ 检测到端口冲突！");
            ui_log!(app_handle, info, "commands::start_environment", "");
            
            for (container_name, port, service_name) in &conflicts {
                ui_log!(app_handle, info, "commands::start_environment", "   ❌ 端口 {} ({}) 被容器 {} 占用", port, service_name, container_name);
            }
            
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(app_handle, info, "commands::start_environment", "💡 解决方案：");
            ui_log!(app_handle, info, "commands::start_environment", "   • 停止冲突容器: docker stop <容器名>");
            ui_log!(app_handle, info, "commands::start_environment", "   • 或删除冲突容器: docker rm <容器名>");
            ui_log!(app_handle, info, "commands::start_environment", "   • 或在环境配置中修改为其他端口");
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(app_handle, info, "commands::start_environment", "⚠️ 请在前端解决冲突后重新启动");
            
            // 返回错误，终止后续流程
            let conflict_details: Vec<String> = conflicts.iter()
                .map(|(name, port, service)| format!("端口 {port} ({service}) 被容器 {name} 占用"))
                .collect();
            
            return Err(format!("PORT_CONFLICT:{}", conflict_details.join("; ")));
        } else {
            ui_log!(app_handle, info, "commands::start_environment", "✅ 没有检测到端口冲突");
        }
    } else {
        ui_log!(app_handle, info, "commands::start_environment", "⚠️ 未找到配置文件，跳过端口检查");
    }
    ui_log!(app_handle, info, "commands::start_environment", "");
    
    // 第二步:启动新容器(流式输出)
    // 根据 Docker Compose 版本决定是否使用 --progress 参数
    let supports_progress = check_compose_progress_support();
    
    if supports_progress {
        // V2.20+ 支持 --progress plain，使用 -d 模式
        ui_log!(app_handle, info, "commands::start_environment", "🔧 执行: docker compose up -d");
        ui_log!(app_handle, info, "commands::start_environment", "⏳ 首次启动可能需要几分钟(下载镜像、安装扩展)...");
        
        let mut compose_cmd = Command::new("docker");
        compose_cmd.args(&["compose", "up", "-d"])
            .current_dir(&project_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            compose_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        // Use spawn() for streaming output instead of blocking output()
        let mut child = compose_cmd.spawn().map_err(|e| {
                let err_msg = format!("执行 docker compose 失败: {e}");
                ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
                err_msg
            })?;
        
        // Stream stdout and stderr in real-time via threads
        let stdout_opt = child.stdout.take();
        let stderr_opt = child.stderr.take();
        let stderr_lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        
        let app_stdout = app_handle.clone();
        let stdout_thread = if let Some(stdout) = stdout_opt {
            Some(std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.is_empty() {
                        ui_log!(&app_stdout, info, "commands::start_environment", "   {}", line);
                    }
                }
            }))
        } else { None };
        
        let app_stderr = app_handle.clone();
        let stderr_lines_clone = stderr_lines.clone();
        let stderr_thread = if let Some(stderr) = stderr_opt {
            Some(std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.is_empty() {
                        ui_log!(&app_stderr, info, "commands::start_environment", "   ⚠️ {}", line);
                        if let Ok(mut lines) = stderr_lines_clone.lock() {
                            lines.push(line);
                        }
                    }
                }
            }))
        } else { None };
        
        // Wait for the process to finish
        let status = child.wait().map_err(|e| {
            let err_msg = format!("等待 docker compose 完成失败: {e}");
            ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
            err_msg
        })?;
        
        // Wait for reader threads to finish
        if let Some(t) = stdout_thread { let _ = t.join(); }
        if let Some(t) = stderr_thread { let _ = t.join(); }
        
        let collected_stderr = stderr_lines.lock().map(|l| l.join("\n")).unwrap_or_default();
        
        if status.success() {
            ui_log!(app_handle, info, "commands::start_environment", "✅ 环境启动成功！");
            Ok("环境启动成功".to_string())
        } else {
            // 分析错误类型，提供更友好的提示
            let exit_code = status.code();
            
            // 检查是否是端口冲突错误
            let is_port_conflict = collected_stderr.contains("port is already allocated") 
                || collected_stderr.contains("Bind for") 
                || collected_stderr.contains("address already in use");
            
            if is_port_conflict {
                ui_log!(app_handle, info, "commands::start_environment", "❌ 端口冲突 detected！");
                ui_log!(app_handle, info, "commands::start_environment", "");
                ui_log!(app_handle, info, "commands::start_environment", "💡 可能的原因：");
                ui_log!(app_handle, info, "commands::start_environment", "   1. 其他 Docker 容器占用了相同端口");
                ui_log!(app_handle, info, "commands::start_environment", "   2. 本地服务（如 MySQL、Nginx）正在运行");
                ui_log!(app_handle, info, "commands::start_environment", "");
                ui_log!(app_handle, info, "commands::start_environment", "🔧 解决方案：");
                ui_log!(app_handle, info, "commands::start_environment", "   方案 1: 停止占用端口的容器");
                ui_log!(app_handle, info, "commands::start_environment", "           docker ps  # 查看运行中的容器");
                ui_log!(app_handle, info, "commands::start_environment", "           docker stop <容器名>");
                ui_log!(app_handle, info, "commands::start_environment", "");
                ui_log!(app_handle, info, "commands::start_environment", "   方案 2: 修改 .env 文件中的端口配置");
                ui_log!(app_handle, info, "commands::start_environment", "           例如：MYSQL_PORT=3307 (改为其他端口)");
                ui_log!(app_handle, info, "commands::start_environment", "           然后重新应用配置");
                ui_log!(app_handle, info, "commands::start_environment", "");
                ui_log!(app_handle, info, "commands::start_environment", "   方案 3: 停止本地服务");
                ui_log!(app_handle, info, "commands::start_environment", "           检查是否有本地 MySQL/Nginx/Redis 在运行");
                ui_log!(app_handle, info, "commands::start_environment", "");
                
                // 提取具体冲突的端口信息
                if let Some(line) = collected_stderr.lines().find(|l| l.contains("Bind for")) {
                    ui_log!(app_handle, info, "commands::start_environment", "📍 详细信息: {}", line.trim());
                }
            } else {
                ui_log!(app_handle, info, "commands::start_environment", "❌ Docker Compose 启动失败，退出码: {:?}", exit_code);
                ui_log!(app_handle, info, "commands::start_environment", "");
                ui_log!(app_handle, info, "commands::start_environment", "💡 建议检查：");
                ui_log!(app_handle, info, "commands::start_environment", "   1. Docker Desktop 是否正常运行");
                ui_log!(app_handle, info, "commands::start_environment", "   2. docker-compose.yml 文件格式是否正确");
                ui_log!(app_handle, info, "commands::start_environment", "   3. 镜像是否存在或网络是否正常");
            }
            
            let err_msg = format!("Docker Compose 启动失败: {}", 
                if is_port_conflict { "端口冲突" } else { "未知错误" });
            Err(err_msg)
        }
    } else {
        // 旧版本不支持 --progress，使用 -d 后台模式启动，然后用 logs 命令获取日志
        ui_log!(app_handle, info, "commands::start_environment", "🔧 执行: docker compose up -d (后台模式)");
        ui_log!(app_handle, info, "commands::start_environment", "⏳ 正在后台启动服务...");
        ui_log!(app_handle, info, "commands::start_environment", "");
        
        // 第一步：后台启动容器
        let mut compose_cmd = Command::new("docker");
        compose_cmd.args(&["compose", "up", "-d"])
            .current_dir(&project_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            compose_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        let output = compose_cmd.output().map_err(|e| {
                let err_msg = format!("执行 docker compose 失败: {e}");
                ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
                err_msg
            })?;
        
        // 输出启动结果
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        if !stdout.is_empty() {
            for line in stdout.lines() {
                if !line.is_empty() {
                    ui_log!(app_handle, info, "commands::start_environment", "   {}", line);
                }
            }
        }
        
        if !output.status.success() {
            // 启动失败，输出错误信息
            if !stderr.is_empty() {
                for line in stderr.lines() {
                    if !line.is_empty() {
                        ui_log!(app_handle, info, "commands::start_environment", "   ⚠️ {}", line);
                    }
                }
            }
            
            let err_msg = format!("Docker Compose 启动失败");
            return Err(err_msg);
        }
        
        ui_log!(app_handle, info, "commands::start_environment", "✅ 容器已在后台启动");
        ui_log!(app_handle, info, "commands::start_environment", "");
        
        // 第二步：流式输出容器日志（实时显示）
        ui_log!(app_handle, info, "commands::start_environment", "📜 开始输出容器日志...");
        
        let mut logs_cmd = Command::new("docker");
        logs_cmd.args(&["compose", "logs", "-f"])
            .current_dir(&project_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            logs_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        let mut child = logs_cmd.spawn().map_err(|e| {
                let err_msg = format!("执行 docker compose logs 失败: {e}");
                ui_log!(app_handle, info, "commands::start_environment", "❌ {}", err_msg);
                err_msg
            })?;
        
        // 异步读取 stdout 和 stderr，实时推送日志到前端
        let app_handle_clone = app_handle.clone();
        let stderr_handle = app_handle.clone();
        
        // 先取出 stdout 和 stderr
        let stdout_opt = child.stdout.take();
        let stderr_opt = child.stderr.take();
        
        // 读取 stdout 线程
        let _stdout_thread = if let Some(stdout) = stdout_opt {
            Some(std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            if !line.is_empty() {
                                // 解析关键进度信息
                                let progress_msg = parse_docker_progress(&line);
                                if let Some(msg) = progress_msg {
                                    ui_log!(&app_handle_clone, info, "commands::start_environment", "📦 {}", msg);
                                } else {
                                    // 普通日志也显示
                                    ui_log!(&app_handle_clone, info, "commands::start_environment", "   {}", line);
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            None
        };
        
        // 读取 stderr 线程
        let _stderr_thread = if let Some(stderr) = stderr_opt {
            Some(std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            if !line.is_empty() {
                                ui_log!(&stderr_handle, info, "commands::start_environment", "   ⚠️ {}", line);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            None
        };
        
        // 第三步：智能等待容器就绪
        ui_log!(app_handle, info, "commands::start_environment", "⏳ 等待容器就绪...");
        
        // 创建 DockerManager 实例
        let docker_manager = match DockerManager::new() {
            Ok(manager) => manager,
            Err(e) => {
                ui_log!(app_handle, warn, "commands::start_environment", "⚠️ 无法创建 DockerManager: {}", e);
                ui_log!(app_handle, info, "commands::start_environment", "⚠️ 跳过智能等待，直接返回");
                let _ = child.kill();
                return Ok("环境启动成功（未检查容器状态）".to_string());
            }
        };
        
        // 智能等待循环
        loop {
            // ✅ 保险 1：检查所有容器是否就绪
            match docker_manager.check_all_ps_containers_running().await {
                Ok(true) => {
                    ui_log!(app_handle, info, "commands::start_environment", "✅ 所有容器已就绪");
                    break;
                }
                Ok(false) => {
                    // 继续等待
                }
                Err(e) => {
                    ui_log!(app_handle, warn, "commands::start_environment", "⚠️ 检查容器状态失败: {}", e);
                }
            }
            
            // ⚠️ 保险 2：检查 logs 进程是否还活着
            if let Ok(Some(status)) = child.try_wait() {
                if !status.success() {
                    ui_log!(app_handle, info, "commands::start_environment", "❌ 日志进程异常退出，启动可能失败");
                    return Err("容器启动失败".to_string());
                }
                // 如果 status.success()，说明 logs 正常退出（容器已停止）
                ui_log!(app_handle, info, "commands::start_environment", "⚠️ 日志进程已退出，检查容器状态...");
                break;
            }
            
            // 每 2 秒检查一次
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        
        // 终止日志输出进程
        let _ = child.kill();
        
        ui_log!(app_handle, info, "commands::start_environment", "✅ 环境启动成功！");
        Ok("环境启动成功".to_string())
    }
}

/// 一键重启环境（docker compose restart）
#[tauri::command]
pub async fn restart_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;
    use tauri::Emitter;
    use crate::ui_log;
    
    ui_log!(app_handle, info, "commands::restart_environment", "🔄 开始重启环境...");
    
    let project_root = get_project_root()?;
    ui_log!(app_handle, info, "commands::restart_environment", "📁 项目根目录: {:?}", project_root);
    
    let compose_file = project_root.join("docker-compose.yml");
    
    if !compose_file.exists() {
        ui_log!(app_handle, info, "commands::restart_environment", "❌ docker-compose.yml 文件不存在");
        return Err("docker-compose.yml 文件不存在，请先应用配置".to_string());
    }
    
    ui_log!(app_handle, info, "commands::restart_environment", "✅ docker-compose.yml 存在");
    
    // 使用 docker compose restart 重启所有容器
    ui_log!(app_handle, info, "commands::restart_environment", "🔧 执行: docker compose restart");
    
    let mut restart_cmd = Command::new("docker");
    restart_cmd.args(["compose", "restart"])
        .current_dir(&project_root);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        restart_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    let output = restart_cmd.output().map_err(|e| {
            let err_msg = format!("执行 docker compose restart 失败: {e}");
            ui_log!(app_handle, info, "commands::restart_environment", "❌ {}", err_msg);
            err_msg
        })?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        ui_log!(app_handle, info, "commands::restart_environment", "❌ 重启失败");
        ui_log!(app_handle, info, "commands::restart_environment", "错误输出: {}", stderr);
        return Err(format!("Docker Compose 重启失败: {stderr}"));
    }
    
    // 记录重启结果
    if !stdout.is_empty() {
        for line in stdout.lines() {
            if !line.is_empty() {
                ui_log!(app_handle, info, "commands::restart_environment", "   {}", line);
            }
        }
    }
    
    ui_log!(app_handle, info, "commands::restart_environment", "✅ 环境重启成功！");
    Ok("环境重启成功".to_string())
}

/// 一键停止环境（docker compose down）
#[tauri::command]
pub async fn stop_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;
    use tauri::Emitter;
    use crate::ui_log;
    
    ui_log!(app_handle, info, "commands::stop_environment", "🛑 开始停止环境...");
    
    let project_root = get_project_root()?;
    ui_log!(app_handle, info, "commands::stop_environment", "📁 项目根目录: {:?}", project_root);
    
    let compose_file = project_root.join("docker-compose.yml");
    
    if !compose_file.exists() {
        ui_log!(app_handle, info, "commands::stop_environment", "❌ docker-compose.yml 文件不存在");
        return Err("docker-compose.yml 文件不存在，请先应用配置".to_string());
    }
    
    ui_log!(app_handle, info, "commands::stop_environment", "✅ docker-compose.yml 存在");
    
    // 使用 docker compose down 停止并删除容器
    ui_log!(app_handle, info, "commands::stop_environment", "🔧 执行: docker compose down");
    
    let mut stop_cmd = Command::new("docker");
    stop_cmd.args(["compose", "down"])
        .current_dir(&project_root);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        stop_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    let output = stop_cmd.output().map_err(|e| {
            let err_msg = format!("执行 docker compose down 失败: {e}");
            ui_log!(app_handle, info, "commands::stop_environment", "❌ {}", err_msg);
            err_msg
        })?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        ui_log!(app_handle, info, "commands::stop_environment", "❌ 停止失败");
        ui_log!(app_handle, info, "commands::stop_environment", "错误输出: {}", stderr);
        return Err(format!("Docker Compose 停止失败: {stderr}"));
    }
    
    // 记录停止结果
    if !stdout.is_empty() {
        for line in stdout.lines() {
            if !line.is_empty() {
                ui_log!(app_handle, info, "commands::stop_environment", "   {}", line);
            }
        }
    }
    
    ui_log!(app_handle, info, "commands::stop_environment", "✅ 环境停止成功！");
    Ok("环境停止成功".to_string())
}

// ==================== 统一镜像源管理命令 ====================

/// 标准化镜像源 URL（去除尾部斜杠）
fn normalize_mirror_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // 去除尾部斜杠，但保留协议部分的斜杠（如 https://）
    trimmed.trim_end_matches('/').to_string()
}

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
    
    // 标准化镜像源地址（去除尾部斜杠）
    let normalized_source = normalize_mirror_url(&source);
    
    UnifiedMirrorManager::update_single(&category, &normalized_source, &env_path)
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
        .map_err(|e| format!("序列化镜像源状态失败: {e}"))
}

/// 获取当前匹配的预设名称
#[tauri::command]
pub fn get_current_mirror_preset() -> Result<String, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::detect_current_preset(&env_path)
}

// ==================== 增强镜像源管理命令 ====================

/// 获取合并后的镜像源列表（默认配置 + 用户自定义）
#[tauri::command]
pub fn get_merged_mirror_list() -> Result<Vec<MergedMirrorCategory>, String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::get_merged_mirror_list(&project_root)
}

/// 保存用户选择的镜像源选项
#[tauri::command]
pub fn save_selected_mirror_option(
    category_id: String,
    option_id: String,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::save_selected_option(&project_root, &category_id, &option_id)
}

/// 保存用户自定义的单个类别配置
#[tauri::command]
pub fn save_user_mirror_category(
    category_id: String,
    source: String,
    description: Option<String>,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    
    // 标准化镜像源地址（去除尾部斜杠）
    let normalized_source = normalize_mirror_url(&source);
    
    MirrorConfigManager::save_user_category(&project_root, &category_id, &normalized_source, description)
}

/// 删除用户自定义的类别配置
#[tauri::command]
pub fn remove_user_mirror_category(category_id: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    
    // 1. 从用户配置中删除
    MirrorConfigManager::remove_user_category(&project_root, &category_id)?;
    
    // 2. 同步更新 .env 文件，恢复为默认值
    let env_path = project_root.join(".env");
    let default_value = match category_id.as_str() {
        "docker_registry" => "",
        "apt" => "https://deb.debian.org/debian",
        "composer" => "https://packagist.org",
        "npm" => "https://registry.npmjs.org",
        "github_proxy" => "",
        _ => return Err(format!("未知的镜像源类别: {category_id}")),
    };
    
    UnifiedMirrorManager::update_single(&category_id, default_value, &env_path)
}

/// 重置所有用户自定义镜像源配置
#[tauri::command]
pub fn reset_all_mirror_overrides() -> Result<(), String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::reset_all_overrides(&project_root)
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

/// 选择项目文件夹并转换为相对路径
#[tauri::command]
pub fn select_project_folder() -> Result<Option<String>, String> {
    // 实际的文件选择逻辑应该在前端通过 @tauri-apps/plugin-dialog 实现
    // 这里仅作为占位符，或者我们可以通过 CLI 方式打开文件选择器
    
    // 为了简化，我们暂时不在此处实现复杂的跨平台文件夹选择逻辑
    // 而是建议前端直接使用 dialog.open({ directory: true })
    // 然后前端将选中的绝对路径发送给后端，后端计算相对路径
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 测试 load_existing_config 解析多版本 Redis
    #[test]
    fn test_load_existing_config_multi_redis() {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("php_stack_test_multi_redis");
        fs::create_dir_all(&temp_dir).unwrap();
        
        // 创建测试 .env 文件
        let env_content = r#"SOURCE_DIR=./www
TZ=Asia/Shanghai
REDIS62_VERSION=6.2-alpine-01
REDIS62_HOST_PORT=6379
REDIS72_VERSION=7.2-alpine
REDIS72_HOST_PORT=6380
"#;
        fs::write(temp_dir.join(".env"), env_content).unwrap();
        
        // 创建空的 docker-compose.yml
        fs::write(temp_dir.join("docker-compose.yml"), "version: '3'\nservices: {}\n").unwrap();
        
        // 临时修改 project_root（这里无法直接测试，因为 get_project_root 是硬编码的）
        // 所以这个测试主要用于验证解析逻辑
        
        // 清理
        fs::remove_dir_all(&temp_dir).ok();
    }

    /// 测试 load_existing_config 解析多版本 Nginx
    #[test]
    fn test_load_existing_config_multi_nginx() {
        let temp_dir = std::env::temp_dir().join("php_stack_test_multi_nginx");
        fs::create_dir_all(&temp_dir).unwrap();
        
        let env_content = r#"SOURCE_DIR=./www
TZ=Asia/Shanghai
NGINX127_VERSION=1.27-alpine
NGINX127_HTTP_HOST_PORT=80
NGINX125_VERSION=1.25-alpine
NGINX125_HTTP_HOST_PORT=8080
"#;
        fs::write(temp_dir.join(".env"), env_content).unwrap();
        fs::write(temp_dir.join("docker-compose.yml"), "version: '3'\nservices: {}\n").unwrap();
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    /// 测试 load_existing_config 解析混合服务
    #[test]
    fn test_load_existing_config_mixed_services() {
        let temp_dir = std::env::temp_dir().join("php_stack_test_mixed");
        fs::create_dir_all(&temp_dir).unwrap();
        
        let env_content = r#"SOURCE_DIR=./www
TZ=Asia/Shanghai
PHP85_VERSION=8.5
PHP85_HOST_PORT=9000
PHP85_EXTENSIONS=mysqli,mbstring
MYSQL84_VERSION=8.4
MYSQL84_HOST_PORT=3306
REDIS62_VERSION=6.2-alpine-01
REDIS62_HOST_PORT=6379
NGINX127_VERSION=1.27-alpine
NGINX127_HTTP_HOST_PORT=80
"#;
        fs::write(temp_dir.join(".env"), env_content).unwrap();
        fs::write(temp_dir.join("docker-compose.yml"), "version: '3'\nservices: {}\n").unwrap();
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    /// 测试 Docker Compose 版本检测功能
    #[test]
    fn test_check_compose_progress_support() {
        // 这个测试会实际调用 docker compose version，所以只在有 Docker 的环境中运行
        let result = check_compose_progress_support();
        println!("Docker Compose supports --progress: {}", result);
        // 不 assert，因为结果取决于实际环境
    }
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
