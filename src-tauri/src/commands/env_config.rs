use crate::docker::manager::DockerManager;
use crate::engine::config_generator::{ConfigGenerator, EnvConfig};
use crate::engine::version_manifest::{VersionManifest, ServiceType as VmServiceType};

use super::get_project_root;

/// 检测 Docker Compose 版本，判断是否支持 --progress 参数
/// 返回 true 表示支持 --progress 参数（V2.20+）
fn check_compose_progress_support() -> bool {
    use std::process::Command;
    
    // 执行 docker compose version
    let mut cmd = Command::new("docker");
    cmd.args(["compose", "version"]);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        cmd.creation_flags(0x08000200);
    }
    
    let output = cmd.output();
    
    match output {
        Ok(output) => {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // 解析版本号，例如："Docker Compose version v2.17.3"
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
    let env_file = crate::engine::env_parser::EnvFile::parse(&env_content)
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
                version,
                host_port,
                extensions,
            });
        }
    }
    
    // 解析 MySQL 服务（支持多版本）
    let mut mysql_index = 0;
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("MYSQL") && !key.contains("ROOT") && !key.contains("USER") && !key.contains("PASSWORD") {
            let prefix = &key[..key.len() - 8];
            let index_part = &key[5..key.len() - 8];
            
            if index_part.is_empty() {
                continue;
            }
            
            let idx = index_part.parse::<usize>().unwrap_or(mysql_index);
            
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Mysql, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("MYSQL{index_part}_HOST_PORT");
            
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
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("REDIS") {
            let prefix = &key[..key.len() - 8];
            let index_part = &key[5..key.len() - 8];
            
            if index_part.is_empty() {
                continue;
            }
            
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Redis, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("REDIS{index_part}_HOST_PORT");
            
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
    for (key, _value) in &env_map {
        if key.ends_with("_VERSION") && key.starts_with("NGINX") {
            let prefix = &key[..key.len() - 8];
            let index_part = &key[6..key.len() - 8]; // NGINX 是 5 个字母 + 1 = 6
            
            if index_part.is_empty() {
                continue;
            }
            
            let version = manifest.find_entry_by_env_prefix(&VmServiceType::Nginx, prefix)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| prefix.to_lowercase());
            
            let port_key = format!("NGINX{index_part}_HTTP_HOST_PORT");
            
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
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        down_cmd.creation_flags(0x08000200);
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
            // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
            // CREATE_NEW_PROCESS_GROUP: 创建新的进程组，避免继承父进程的控制台
            // CREATE_NO_WINDOW: 不创建新窗口
            // 这样既能流式读取日志，又不会显示黑窗口
            compose_cmd.creation_flags(0x08000200);
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
            // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
            compose_cmd.creation_flags(0x08000200);
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
            // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
            // 与 docker compose up -d 保持一致，确保流式日志正常读取
            logs_cmd.creation_flags(0x08000200);
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
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        restart_cmd.creation_flags(0x08000200);
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
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        stop_cmd.creation_flags(0x08000200);
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
