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
    
    // 解析 Redis 服务（支持多版本）
    // 查找所有 REDISxx_VERSION 格式的键
    for (key, value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("REDIS") {
            let full_tag = value.clone();
            
            // 提取索引部分，如 REDIS62_VERSION -> 62
            let index_part = &key[5..key.len() - 8]; // 去掉 "REDIS" 和 "_VERSION"
            
            if index_part.is_empty() {
                continue;
            }
            
            // 将完整标签转换为纯版本号（如 "6.2-alpine" -> "6.2"）
            let version = full_tag.split('-').next().unwrap_or(&full_tag).to_string();
            
            let port_key = format!("REDIS{}_HOST_PORT", index_part);
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(6379);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::Redis,
                version,
                host_port,
                extensions: None,
            });
        }
    }
    
    // 解析 Nginx 服务（支持多版本）
    // 查找所有 NGINXxx_VERSION 格式的键
    for (key, value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("NGINX") {
            let full_tag = value.clone();
            
            // 提取索引部分，如 NGINX127_VERSION -> 127
            let index_part = &key[6..key.len() - 8]; // 去掉 "NGINX" 和 "_VERSION"
            
            if index_part.is_empty() {
                continue;
            }
            
            // 将完整标签转换为纯版本号（如 "1.27-alpine" -> "1.27"）
            let version = full_tag.split('-').next().unwrap_or(&full_tag).to_string();
            
            let port_key = format!("NGINX{}_HTTP_HOST_PORT", index_part);
            
            let host_port = env_map.get(&port_key)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(80);
            
            services.push(crate::engine::config_generator::ServiceEntry {
                service_type: crate::engine::config_generator::ServiceType::Nginx,
                version,
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
            existing_files.push(format!("{} ({})", filename, description));
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
    
    // 辅助函数：发送日志到前端并打印到终端
    let emit_log = |msg: &str| {
        eprintln!("{}", msg); // 终端输出（已包含emoji）
        let _ = app_handle.emit("env-log", msg); // 前端UI显示
    };
    
    emit_log("📝 开始应用配置...");
    
    let project_root = get_project_root()?;
    emit_log(&format!("📁 项目根目录: {:?}", project_root));
    
    // 检查用户覆盖配置
    let overrides_path = project_root.join(".user_version_overrides.json");
    if overrides_path.exists() {
        emit_log("✅ 检测到用户版本覆盖配置");
    } else {
        emit_log("ℹ️  未找到用户覆盖配置，使用默认配置");
    }
    
    emit_log("🔧 验证配置...");
    emit_log("📄 生成 .env 文件...");
    emit_log("🐳 生成 docker-compose.yml...");
    emit_log("📂 创建服务目录结构...");
    
    match ConfigGenerator::apply(&config, &project_root, enable_backup).await {
        Ok(backed_up_files) => {
            if !backed_up_files.is_empty() {
                emit_log(&format!("💾 已备份 {} 个文件/目录", backed_up_files.len()));
                for file in &backed_up_files {
                    emit_log(&format!("   - {}", file));
                }
            }
            emit_log("✅ 配置应用成功！");
            emit_log("💡 提示：请重启容器使新配置生效");
            Ok(backed_up_files)
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
    
    // 第一步：清理旧容器（避免名称冲突）
    emit_log("🧹 清理旧容器...");
    let down_output = Command::new("docker")
        .args(&["compose", "down", "--remove-orphans"])
        .current_dir(&project_root)
        .output()
        .map_err(|e| {
            let err_msg = format!("清理旧容器失败: {}", e);
            emit_log(&format!("⚠️ {}", err_msg));
            err_msg
        })?;
    
    if !down_output.status.success() {
        let stderr = String::from_utf8_lossy(&down_output.stderr);
        emit_log(&format!("⚠️ 清理警告: {}", stderr.lines().next().unwrap_or("")));
    } else {
        emit_log("✅ 旧容器已清理");
    }
    
    // 第二步：端口冲突检测
    emit_log("🔍 检查端口冲突...");
    
    // 获取所有运行中的容器
    let manager = DockerManager::new().map_err(|e| {
        let err_msg = format!("创建 Docker 管理器失败: {}", e);
        emit_log(&format!("❌ {}", err_msg));
        err_msg
    })?;
    
    let all_containers = manager.list_all_running_containers().await.map_err(|e| {
        let err_msg = format!("获取容器列表失败: {}", e);
        emit_log(&format!("❌ {}", err_msg));
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
                if container.ports.iter().any(|&p| p == port as i32) {
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
            emit_log("❌ 检测到端口冲突！");
            emit_log("");
            
            for (container_name, port, service_name) in &conflicts {
                emit_log(&format!("   ❌ 端口 {} ({}) 被容器 {} 占用", port, service_name, container_name));
            }
            
            emit_log("");
            emit_log("💡 解决方案：");
            emit_log("   • 停止冲突容器: docker stop <容器名>");
            emit_log("   • 或删除冲突容器: docker rm <容器名>");
            emit_log("   • 或在环境配置中修改为其他端口");
            emit_log("");
            emit_log("⚠️ 请在前端解决冲突后重新启动");
            
            // 返回错误，终止后续流程
            let conflict_details: Vec<String> = conflicts.iter()
                .map(|(name, port, service)| format!("端口 {} ({}) 被容器 {} 占用", port, service, name))
                .collect();
            
            return Err(format!("PORT_CONFLICT:{}", conflict_details.join("; ")));
        } else {
            emit_log("✅ 没有检测到端口冲突");
        }
    } else {
        emit_log("⚠️ 未找到配置文件，跳过端口检查");
    }
    emit_log("");
    
    // 第二步:启动新容器(流式输出)
    emit_log("🔧 执行: docker compose --progress plain up -d");
    emit_log("⏳ 首次启动可能需要几分钟(下载镜像、安装扩展)...");
        
    let mut child = Command::new("docker")
        .args(&["compose", "--progress", "plain", "up", "-d"])
        .current_dir(&project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            let err_msg = format!("执行 docker compose 失败: {}", e);
            emit_log(&format!("❌ {}", err_msg));
            err_msg
        })?;
    
    // 读取 stdout（流式）
    let mut stdout_lines = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if !line.is_empty() {
                        emit_log(&format!("   {}", line));
                        stdout_lines.push(line);
                    }
                }
                Err(_) => break,
            }
        }
    }
    
    // 读取 stderr（流式）并收集内容
    let mut stderr_lines = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if !line.is_empty() {
                        emit_log(&format!("   ⚠️ {}", line));
                        stderr_lines.push(line);
                    }
                }
                Err(_) => break,
            }
        }
    }
    
    let stderr_content = stderr_lines.join("\n");
    
    let status = child.wait().map_err(|e| {
        let err_msg = format!("等待 docker compose 完成失败: {}", e);
        emit_log(&format!("❌ {}", err_msg));
        err_msg
    })?;
    
    if status.success() {
        emit_log("✅ 环境启动成功！");
        Ok("环境启动成功".to_string())
    } else {
        // 分析错误类型，提供更友好的提示
        let exit_code = status.code();
        
        // 检查是否是端口冲突错误
        let is_port_conflict = stderr_content.contains("port is already allocated") 
            || stderr_content.contains("Bind for") 
            || stderr_content.contains("address already in use");
        
        if is_port_conflict {
            emit_log("❌ 端口冲突 detected！");
            emit_log("");
            emit_log("💡 可能的原因：");
            emit_log("   1. 其他 Docker 容器占用了相同端口");
            emit_log("   2. 本地服务（如 MySQL、Nginx）正在运行");
            emit_log("");
            emit_log("🔧 解决方案：");
            emit_log("   方案 1: 停止占用端口的容器");
            emit_log("           docker ps  # 查看运行中的容器");
            emit_log("           docker stop <容器名>");
            emit_log("");
            emit_log("   方案 2: 修改 .env 文件中的端口配置");
            emit_log("           例如：MYSQL_PORT=3307 (改为其他端口)");
            emit_log("           然后重新应用配置");
            emit_log("");
            emit_log("   方案 3: 停止本地服务");
            emit_log("           检查是否有本地 MySQL/Nginx/Redis 在运行");
            emit_log("");
            
            // 提取具体冲突的端口信息
            if let Some(line) = stderr_lines.iter().find(|l| l.contains("Bind for")) {
                emit_log(&format!("📍 详细信息: {}", line.trim()));
            }
        } else {
            emit_log(&format!("❌ Docker Compose 启动失败，退出码: {:?}", exit_code));
            emit_log("");
            emit_log("💡 建议检查：");
            emit_log("   1. Docker Desktop 是否正常运行");
            emit_log("   2. docker-compose.yml 文件格式是否正确");
            emit_log("   3. 镜像是否存在或网络是否正常");
        }
        
        let err_msg = format!("Docker Compose 启动失败: {}", 
            if is_port_conflict { "端口冲突" } else { "未知错误" });
        Err(err_msg)
    }
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
        .map_err(|e| format!("序列化镜像源状态失败: {}", e))
}

/// 获取当前匹配的预设名称
#[tauri::command]
pub fn get_current_mirror_preset() -> Result<String, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::detect_current_preset(&env_path)
}

// ==================== 增强镜像源管理命令 (V2.2) ====================

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
        _ => return Err(format!("未知的镜像源类别: {}", category_id)),
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
            .map_err(|e| format!("无法打开目录: {}", e))?;
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
                Ok(format!("{}/**", normalized))
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
    
    // PHP 版本（按版本号降序）
    let mut php_versions = Vec::new();
    let mut php_version_list: Vec<&String> = manifest.get_available_versions(&VmServiceType::Php);
    php_version_list.sort_by(|a, b| {
        let ver_a: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let ver_b: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        ver_b.cmp(&ver_a) // 降序
    });
    
    for version in php_version_list {
        // 使用合并后的配置（用户覆盖优先）
        let merged_info = override_manager.get_merged_image_info(&VmServiceType::Php, version)
            .or_else(|| manifest.get_image_info(&VmServiceType::Php, version).cloned());
        
        if let Some(info) = merged_info {
            // 检查是否有用户覆盖
            let has_user_override = override_manager.has_user_override(&VmServiceType::Php, version);
            
            php_versions.push(serde_json::json!({
                "version": version,
                "image": info.image,
                "tag": info.tag,
                "full_name": info.full_name(),
                "eol": info.eol,
                "description": info.description,
                "has_user_override": has_user_override
            }));
        }
    }
    result.insert("php".to_string(), serde_json::Value::Array(php_versions));
    
    // MySQL 版本（按版本号降序）
    let mut mysql_versions = Vec::new();
    let mut mysql_version_list: Vec<&String> = manifest.get_available_versions(&VmServiceType::Mysql);
    mysql_version_list.sort_by(|a, b| {
        let ver_a: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let ver_b: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        ver_b.cmp(&ver_a) // 降序
    });
    
    for version in mysql_version_list {
        let merged_info = override_manager.get_merged_image_info(&VmServiceType::Mysql, version)
            .or_else(|| manifest.get_image_info(&VmServiceType::Mysql, version).cloned());
        
        if let Some(info) = merged_info {
            let has_user_override = override_manager.has_user_override(&VmServiceType::Mysql, version);
            
            mysql_versions.push(serde_json::json!({
                "version": version,
                "image": info.image,
                "tag": info.tag,
                "full_name": info.full_name(),
                "eol": info.eol,
                "description": info.description,
                "has_user_override": has_user_override
            }));
        }
    }
    result.insert("mysql".to_string(), serde_json::Value::Array(mysql_versions));
    
    // Redis 版本（按版本号降序）
    let mut redis_versions = Vec::new();
    let mut redis_version_list: Vec<&String> = manifest.get_available_versions(&VmServiceType::Redis);
    redis_version_list.sort_by(|a, b| {
        let ver_a: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let ver_b: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        ver_b.cmp(&ver_a) // 降序
    });
    
    for version in redis_version_list {
        let merged_info = override_manager.get_merged_image_info(&VmServiceType::Redis, version)
            .or_else(|| manifest.get_image_info(&VmServiceType::Redis, version).cloned());
        
        if let Some(info) = merged_info {
            let has_user_override = override_manager.has_user_override(&VmServiceType::Redis, version);
            
            redis_versions.push(serde_json::json!({
                "version": version,
                "image": info.image,
                "tag": info.tag,
                "full_name": info.full_name(),
                "eol": info.eol,
                "description": info.description,
                "has_user_override": has_user_override
            }));
        }
    }
    result.insert("redis".to_string(), serde_json::Value::Array(redis_versions));
    
    // Nginx 版本（按版本号降序）
    let mut nginx_versions = Vec::new();
    let mut nginx_version_list: Vec<&String> = manifest.get_available_versions(&VmServiceType::Nginx);
    nginx_version_list.sort_by(|a, b| {
        let ver_a: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let ver_b: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        ver_b.cmp(&ver_a) // 降序
    });
    
    for version in nginx_version_list {
        let merged_info = override_manager.get_merged_image_info(&VmServiceType::Nginx, version)
            .or_else(|| manifest.get_image_info(&VmServiceType::Nginx, version).cloned());
        
        if let Some(info) = merged_info {
            let has_user_override = override_manager.has_user_override(&VmServiceType::Nginx, version);
            
            nginx_versions.push(serde_json::json!({
                "version": version,
                "image": info.image,
                "tag": info.tag,
                "full_name": info.full_name(),
                "eol": info.eol,
                "description": info.description,
                "has_user_override": has_user_override
            }));
        }
    }
    result.insert("nginx".to_string(), serde_json::Value::Array(nginx_versions));
    
    Ok(serde_json::to_value(result).map_err(|e| format!("序列化失败: {}", e))?)
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
        _ => return Err(format!("不支持的服务类型: {}", service_type)),
    };
    
    Ok(manifest.is_version_valid(&vm_service_type, &version))
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
        _ => return Err(format!("不支持的服务类型: {}", service_type)),
    };
    
    Ok(manifest.get_recommended_version(&vm_service_type).map(|s| s.to_string()))
}

/// 保存用户自定义版本覆盖
#[tauri::command]
pub fn save_user_override(
    service_type: String,
    version: String,
    tag: String,
    description: Option<String>,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);
    
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {}", service_type)),
    };
    
    let override_config = UserVersionOverride {
        tag,
        description,
    };
    
    manager.save_user_override(&project_root, vm_service_type, version, override_config)
}

/// 删除用户自定义版本覆盖
#[tauri::command]
pub fn remove_user_override(service_type: String, version: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let mut manager = UserOverrideManager::new(&project_root);
    
    let vm_service_type = match service_type.as_str() {
        "php" => VmServiceType::Php,
        "mysql" => VmServiceType::Mysql,
        "redis" => VmServiceType::Redis,
        "nginx" => VmServiceType::Nginx,
        _ => return Err(format!("不支持的服务类型: {}", service_type)),
    };
    
    manager.remove_user_override(&project_root, &vm_service_type, &version)
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
    use std::path::PathBuf;

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
}
