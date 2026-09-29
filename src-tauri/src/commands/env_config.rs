use super::get_project_root;
use crate::docker::manager::DockerManager;
use crate::engine::config_extractor::{ConfigExtractor, ExtractOutcome, ImageStatus};
use crate::engine::config_generator::{ConfigGenerator, EnvConfig};
use crate::engine::service_catalog::{GeneratorKind, ServiceCatalog};
use crate::engine::site_manager;
use crate::engine::user_config;
use crate::engine::user_override_manager::UserOverrideManager;
use crate::engine::version_manifest::{VersionEntry, VersionManifest};

/// 单个镜像拉取结果（前端"待拉取"确认弹窗使用）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PullImageResult {
    pub tag: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 把同步子进程调用挪到阻塞线程池执行（R3）
///
/// Tauri 命令跑在 async 执行线程上，同步的 `Command::output()` / `Child::wait()`
/// 会把整条线程占住，同线程上的其它命令（容器列表轮询、镜像探测等）
/// 会被一起拖慢。这里收敛成统一助手，与 `commands/backup.rs` 的既有做法对齐。
async fn run_blocking_command<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("blocking task failed: {e}"))?
}

/// 获取 Docker Compose 容器的最新日志
///
/// 使用 `docker compose logs --tail 100` 获取最近日志，
/// 通过 skip_lines 跳过已处理的行，只返回新增内容。
///
/// 内部是同步子进程调用，已包进 `run_blocking_command`——本函数在启动流程中
/// 最长 5 分钟内每 2 秒被调用一次，直接在 async 线程上跑会持续占用该线程。
async fn get_compose_logs(
    project_root: &std::path::Path,
    skip_lines: usize,
) -> Result<(Vec<String>, usize), String> {
    let project_root = project_root.to_path_buf();

    run_blocking_command(move || {
        use std::process::Command;

        let mut logs_cmd = Command::new("docker");
        logs_cmd
            .args(["compose", "logs", "--tail", "100"])
            .current_dir(&project_root);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            logs_cmd.creation_flags(0x08000200);
        }

        let output = logs_cmd
            .output()
            .map_err(|e| format!("failed to run docker compose logs: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "docker compose logs exit code: {:?}",
                output.status.code()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let all_lines: Vec<&str> = stdout.lines().collect();
        let total_count = all_lines.len();

        let new_lines: Vec<String> = all_lines
            .iter()
            .skip(skip_lines)
            .map(|s| s.to_string())
            .collect();

        Ok((new_lines, total_count))
    })
    .await
}

/// 解析 Docker Compose 输出，提取关键进度信息
/// 返回格式化后的进度消息，如果不是进度相关的行则返回 None
fn parse_docker_progress(line: &str) -> Option<String> {
    // 拉取镜像: Pulling php ...
    if line.contains("Pulling") {
        if let Some(service) = extract_service_name(line, "Pulling") {
            return Some(format!("Pulling image: {service}"));
        }
    }

    // 下载进度: php Pulled
    if line.contains("Pulled") {
        if let Some(service) = extract_service_name(line, "Pulled") {
            return Some(format!("Image pulled: {service}"));
        }
    }

    // 构建镜像: Building php
    if line.contains("Building") {
        if let Some(service) = extract_service_name(line, "Building") {
            return Some(format!("Building image: {service}"));
        }
    }

    // 创建容器: Creating ps-php-1 ... done
    if line.contains("Creating") {
        if let Some(service) = extract_service_name(line, "Creating") {
            return Some(format!("Creating container: {service}"));
        }
    }

    // 启动容器: Starting ps-php-1 ... done
    if line.contains("Starting") {
        if let Some(service) = extract_service_name(line, "Starting") {
            return Some(format!("Starting service: {service}"));
        }
    }

    // 容器已存在: Container ps-php-1 is running
    if line.contains("is running") || line.contains("Up to date") {
        return Some("Service already running".to_string());
    }

    None
}

/// 从 Docker Compose 输出行中提取服务名称
fn extract_service_name(line: &str, keyword: &str) -> Option<String> {
    // 查找关键词后的服务名
    if let Some(pos) = line.find(keyword) {
        let after_keyword = &line[pos + keyword.len()..];
        // 提取第一个单词作为服务名
        let service = after_keyword.split_whitespace().next()?;
        // 去除可能的特殊字符
        let clean_service = service.trim_end_matches(['.', ':', ' ']);
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
    let project_root = get_project_root().ok();
    ConfigGenerator::validate(&config, project_root.as_deref())
}

/// 从 .env 键解析服务列表（纯函数，便于单测）。
///
/// 按 version_manifest / 用户 custom 的 `service_dir` 生成 `{DIR}_VERSION` 去匹配，
/// **不再**用 `key[3..]` / `key[5..]` / `key[6..]` 这类魔数切片反解前缀（A2）。
///
/// `override_manager` 为 `None` 时仅扫清单；有值时同时扫 custom 条目。
pub fn parse_env_to_services(
    env_map: &std::collections::HashMap<String, String>,
    manifest: &VersionManifest,
    override_manager: Option<&UserOverrideManager>,
    project_root: &std::path::Path,
) -> Vec<crate::engine::config_generator::ServiceEntry> {
    use crate::engine::config_generator::ServiceEntry;
    use std::collections::{BTreeSet, HashSet};

    let catalog = ServiceCatalog::merged(project_root);
    let mut services: Vec<ServiceEntry> = Vec::new();
    let mut seen_kinds: HashSet<String> = HashSet::new();

    for desc in catalog.list() {
        seen_kinds.insert(desc.id.clone());
        let with_extensions = desc.generator == GeneratorKind::Php;
        collect_services_for_kind(
            &mut services,
            env_map,
            manifest,
            override_manager,
            &desc.id,
            &desc.host_port_env_suffix,
            with_extensions,
        );
    }

    // 清单或覆盖中出现、但不在 builtin catalog 的 kind（如仅有 custom 版本）
    let mut extra_kinds: BTreeSet<String> = BTreeSet::new();
    for k in manifest.service_kinds() {
        if !seen_kinds.contains(&k) {
            extra_kinds.insert(k);
        }
    }
    if let Some(mgr) = override_manager {
        for k in mgr.service_kinds() {
            if !seen_kinds.contains(&k) {
                extra_kinds.insert(k);
            }
        }
    }
    for kind in extra_kinds {
        let port_suffix = catalog
            .get(&kind)
            .map(|d| d.host_port_env_suffix.as_str())
            .unwrap_or("HOST_PORT");
        collect_services_for_kind(
            &mut services,
            env_map,
            manifest,
            override_manager,
            &kind,
            port_suffix,
            false,
        );
    }

    services
}

fn collect_services_for_kind(
    services: &mut Vec<crate::engine::config_generator::ServiceEntry>,
    env_map: &std::collections::HashMap<String, String>,
    manifest: &VersionManifest,
    override_manager: Option<&UserOverrideManager>,
    kind: &str,
    port_suffix: &str,
    with_extensions: bool,
) {
    use crate::engine::config_generator::ServiceEntry;

    let entries: Vec<(String, VersionEntry)> = if let Some(mgr) = override_manager {
        mgr.list_merged_entries(kind)
            .into_iter()
            .map(|item| (item.id, item.entry))
            .collect()
    } else {
        manifest
            .get_available_entries(kind)
            .into_iter()
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect()
    };

    for (id, entry) in entries {
        let prefix = entry.service_dir.to_uppercase();
        let version_key = format!("{prefix}_VERSION");
        if !env_map.contains_key(&version_key) {
            continue;
        }

        let port_key = format!("{prefix}_{port_suffix}");
        let host_port = env_map
            .get(&port_key)
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(entry.default_port);

        let extensions = if with_extensions {
            env_map
                .get(&format!("{prefix}_EXTENSIONS"))
                .map(|exts| exts.split(',').map(|s| s.trim().to_string()).collect())
        } else {
            None
        };

        services.push(ServiceEntry {
            service_type: kind.to_string(),
            version: id,
            host_port,
            extensions,
        });
    }
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
    let env_content =
        std::fs::read_to_string(&env_path).map_err(|e| format!("failed to read .env file: {e}"))?;
    let env_file = crate::engine::env_parser::EnvFile::parse(&env_content)
        .map_err(|e| format!("failed to parse .env file: {e}"))?;
    let env_map = env_file.to_map();

    // 创建 VersionManifest / UserOverrideManager 用于 env prefix 反查
    let manifest = VersionManifest::new();
    let override_manager = UserOverrideManager::new(&project_root);
    let services =
        parse_env_to_services(&env_map, &manifest, Some(&override_manager), &project_root);

    // 如果没有解析到任何服务，返回 None
    if services.is_empty() {
        return Ok(None);
    }

    let timezone = env_map
        .get("TZ")
        .cloned()
        .unwrap_or_else(|| "Asia/Shanghai".to_string());
    let mysql_root_password = env_map.get("MYSQL_ROOT_PASSWORD").cloned();

    Ok(Some(EnvConfig {
        services,
        timezone,
        mysql_root_password,
        sites: site_manager::load_sites_with_hosts(&project_root, &env_file),
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
    let project_root = get_project_root()?;
    ConfigGenerator::validate(&config, Some(&project_root))?;
    Ok(ConfigGenerator::generate_compose(&config, &project_root))
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
pub async fn apply_env_config(
    config: EnvConfig,
    enable_backup: bool,
    app_handle: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    use crate::ui_log;
    use tauri::Emitter;

    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Applying config..."
    );

    let project_root = get_project_root()?;
    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Project root: {:?}",
        project_root
    );

    // 检查用户覆盖配置
    let overrides_path = user_config::path(&project_root, user_config::VERSION_OVERRIDES);
    if overrides_path.exists() {
        ui_log!(
            app_handle,
            info,
            "commands::apply_env_config",
            "User version overrides found"
        );
    } else {
        ui_log!(
            app_handle,
            info,
            "commands::apply_env_config",
            "No user overrides, using defaults"
        );
    }

    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Validating config..."
    );
    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Generating .env..."
    );
    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Generating docker-compose.yml..."
    );
    ui_log!(
        app_handle,
        info,
        "commands::apply_env_config",
        "Creating service directories..."
    );

    match ConfigGenerator::apply(&config, &project_root, enable_backup).await {
        Ok(backed_up_files) => {
            if !backed_up_files.is_empty() {
                ui_log!(
                    app_handle,
                    info,
                    "commands::apply_env_config",
                    "Backed up {} files/dirs",
                    backed_up_files.len()
                );
                for file in &backed_up_files {
                    ui_log!(
                        app_handle,
                        info,
                        "commands::apply_env_config",
                        "   - {}",
                        file
                    );
                }
            }
            ui_log!(
                app_handle,
                info,
                "commands::apply_env_config",
                "Config applied"
            );
            ui_log!(
                app_handle,
                info,
                "commands::apply_env_config",
                "Restart containers to apply the new config"
            );
            Ok(backed_up_files)
        }
        Err(e) => {
            ui_log!(
                app_handle,
                error,
                "commands::apply_env_config",
                "Config apply failed: {}",
                e
            );
            Err(e)
        }
    }
}

// ==================== Phase 3: 镜像探测 / 拉取 / 配置提取 ====================

/// 从配置收集待探测镜像 tag（按 tag 去重，避免同镜像多服务重复 docker images / 弹窗重复项）
fn collect_unique_image_tags(
    config: &EnvConfig,
    override_manager: &UserOverrideManager,
    manifest: &VersionManifest,
) -> Vec<String> {
    use crate::engine::config_generator::resolve_service_image_tag;
    use crate::engine::service_catalog::normalize_service_kind;
    use std::collections::BTreeSet;

    let mut tags = BTreeSet::new();
    for service in &config.services {
        let kind = normalize_service_kind(&service.service_type);
        tags.insert(resolve_service_image_tag(
            override_manager,
            manifest,
            &kind,
            &service.version,
        ));
    }
    tags.into_iter().collect()
}

/// 按 EnvConfig 推算所有服务的 image_tag 并批量探测本地存在性
///
/// 前端流程：
/// 1. 用户点击"应用配置" → 先调本接口探测镜像
/// 2. 若有 `Missing` 项 → 弹"待拉取"确认对话框
/// 3. 用户确认 → 调 `pull_service_images` 拉取缺失镜像
/// 4. 拉取完成 → 调 `apply_env_config`（此时镜像已存在，extract 路径生效）
#[tauri::command]
pub fn check_service_images_presence(config: EnvConfig) -> Result<Vec<ImageStatus>, String> {
    let project_root = get_project_root()?;
    let override_manager = UserOverrideManager::new(&project_root);
    let manifest = VersionManifest::new();
    let tags = collect_unique_image_tags(&config, &override_manager, &manifest);
    Ok(ConfigExtractor::check_images_batch(&tags))
}

/// 批量拉取镜像（前端用户确认后调用）
///
/// **一条失败不影响其他镜像**——返回每条的 success/error，前端可选择继续或回滚。
/// `pull_service_images` 是 idempotent：本机已有镜像时 `docker pull` 本身是 no-op + 极快。
#[tauri::command]
pub async fn pull_service_images(image_tags: Vec<String>) -> Result<Vec<PullImageResult>, String> {
    use crate::app_log;

    let mut results: Vec<PullImageResult> = Vec::with_capacity(image_tags.len());
    for tag in image_tags {
        let tag_for_task = tag.clone();
        // docker pull 会长时间阻塞（大镜像下载），必须走阻塞线程池，
        // 否则 Tauri async runtime 的执行线程被占满，前端弹窗无响应。
        let result = run_blocking_command(move || ConfigExtractor::pull_image(&tag_for_task)).await;
        match result {
            Ok(()) => {
                app_log!(
                    info,
                    "commands::pull_service_images",
                    "Image pulled: {}",
                    tag
                );
                results.push(PullImageResult {
                    tag,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                app_log!(
                    warn,
                    "commands::pull_service_images",
                    "Image pull failed: {} ({})",
                    tag,
                    e
                );
                results.push(PullImageResult {
                    tag,
                    success: false,
                    error: Some(e),
                });
            }
        }
    }
    Ok(results)
}

/// 强制从镜像提取配置（覆盖式；用户手动点"重新提取"时调用）
///
/// **谨慎**：destination 存在时返回 `SkippedExists`（保留用户修改）。如需强制覆盖，
/// 前端应先在 UI 提示用户、删除目标文件后再调用。
#[tauri::command]
pub fn extract_service_config(
    service_type: String,
    service_dir: String,
    image_tag: String,
) -> Result<ExtractOutcome, String> {
    use crate::app_log;
    use crate::engine::service_catalog::normalize_service_kind;

    let project_root = get_project_root()?;
    let kind = normalize_service_kind(&service_type);

    app_log!(
        info,
        "commands::extract_service_config",
        "Extracting {} config from {} (dir={})",
        kind,
        image_tag,
        service_dir
    );

    let outcome = ConfigExtractor::extract_config(&kind, &service_dir, &image_tag, &project_root);
    Ok(outcome)
}

/// 一键启动环境（docker compose up -d）
#[tauri::command]
pub async fn start_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use crate::ui_log;
    use std::process::Command;
    use tauri::Emitter;

    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Starting environment..."
    );

    let project_root = get_project_root()?;
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Project root: {:?}",
        project_root
    );

    let compose_file = project_root.join("docker-compose.yml");

    if !compose_file.exists() {
        ui_log!(
            app_handle,
            error,
            "commands::start_environment",
            "docker-compose.yml not found"
        );
        return Err("docker-compose.yml not found; apply config first".to_string());
    }

    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "docker-compose.yml found"
    );

    // 第一步：清理旧容器（避免名称冲突）
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Removing old containers..."
    );
    let mut down_cmd = Command::new("docker");
    down_cmd
        .args(["compose", "down", "--remove-orphans"])
        .current_dir(&project_root);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        down_cmd.creation_flags(0x08000200);
    }

    // R3: docker compose down 是同步阻塞调用，挪到阻塞线程池
    let down_output = run_blocking_command(move || {
        down_cmd
            .output()
            .map_err(|e| format!("failed to remove old containers: {e}"))
    })
    .await
    .map_err(|e| {
        ui_log!(app_handle, error, "commands::start_environment", "{}", e);
        e
    })?;

    if !down_output.status.success() {
        let stderr = String::from_utf8_lossy(&down_output.stderr);
        ui_log!(
            app_handle,
            warn,
            "commands::start_environment",
            "Cleanup warning: {}",
            stderr.lines().next().unwrap_or("")
        );
    } else {
        ui_log!(
            app_handle,
            info,
            "commands::start_environment",
            "Old containers removed"
        );
    }

    // 等待 ps- 前缀的容器完全停止（最多等待 10 秒）
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Waiting for containers to stop..."
    );
    let manager = DockerManager::new().map_err(|e| {
        let err_msg = format!("failed to create Docker manager: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::start_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    for attempt in 1..=10 {
        let ps_containers = manager.list_ps_containers().await.map_err(|e| {
            let err_msg = format!("failed to check container status: {e}");
            ui_log!(
                app_handle,
                error,
                "commands::start_environment",
                "{}",
                err_msg
            );
            err_msg
        })?;

        // 过滤出仍在运行的 ps- 容器
        let running_ps_containers: Vec<_> = ps_containers
            .iter()
            .filter(|c| c.state.is_running())
            .collect();

        if running_ps_containers.is_empty() {
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "All ps- containers stopped"
            );
            break;
        }

        if attempt == 10 {
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "Timed out waiting, {} containers still running",
                running_ps_containers.len()
            );
            for container in &running_ps_containers {
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "   - {} ({})",
                    container.name,
                    container.state
                );
            }
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    ui_log!(app_handle, info, "commands::start_environment", "");

    // 第二步：端口冲突检测
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Checking port conflicts..."
    );

    // 获取所有运行中的容器
    let manager = DockerManager::new().map_err(|e| {
        let err_msg = format!("failed to create Docker manager: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::start_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    let all_containers = manager.list_all_running_containers().await.map_err(|e| {
        let err_msg = format!("failed to list containers: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::start_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    // 加载配置并检查端口
    let config_result = load_existing_config()?;
    if let Some(config) = config_result {
        let mut conflicts: Vec<(String, u16, String)> = Vec::new();

        for service in &config.services {
            let port = service.host_port;
            let service_name = format!(
                "{:?}{}",
                service.service_type,
                service.version.replace('.', "")
            );

            // 检查是否有容器占用了这个端口
            for container in &all_containers {
                if container.ports.contains(&(port as i32)) {
                    conflicts.push((container.name.clone(), port, service_name.clone()));
                    break;
                }
            }
        }

        if !conflicts.is_empty() {
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "Port conflict detected"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");

            for (container_name, port, service_name) in &conflicts {
                ui_log!(
                    app_handle,
                    warn,
                    "commands::start_environment",
                    "Port {} ({}) in use by {}",
                    port,
                    service_name,
                    container_name
                );
            }

            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "How to fix:"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Stop the container: docker stop <name>"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Or remove it: docker rm <name>"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Or change the port in Environment"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "Resolve the conflict, then start again"
            );

            // 返回错误，终止后续流程
            let conflict_details: Vec<String> = conflicts
                .iter()
                .map(|(name, port, service)| format!("port {port} ({service}) in use by {name}"))
                .collect();

            return Err(format!("PORT_CONFLICT:{}", conflict_details.join("; ")));
        } else {
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "No port conflicts"
            );
        }
    } else {
        ui_log!(
            app_handle,
            info,
            "commands::start_environment",
            "No config file, skipping port check"
        );
    }
    ui_log!(app_handle, info, "commands::start_environment", "");

    // 构建型服务：目标指纹镜像缺失时先 compose build --cache-from，再 up
    if let Some(ref config) = load_existing_config()? {
        match crate::engine::image_transfer::ImageTransferEngine::build_plan_for_start(
            &project_root,
            config,
        ) {
            Ok(plan) if !plan.services_needing_build.is_empty() => {
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "Building missing images: {}",
                    plan.services_needing_build.join(", ")
                );
                if !plan.cache_from.is_empty() {
                    ui_log!(
                        app_handle,
                        info,
                        "commands::start_environment",
                        "Using cache-from: {}",
                        plan.cache_from.join(", ")
                    );
                }

                // docker compose build 不支持 CLI --cache-from，改用临时 override 的 build.cache_from
                let cache_override =
                    crate::engine::image_transfer::ImageTransferEngine::write_build_cache_override(
                        &project_root,
                        &plan,
                    )?;

                let mut build_cmd = Command::new("docker");
                build_cmd.arg("compose").arg("-f").arg("docker-compose.yml");
                if let Some(ref override_path) = cache_override {
                    build_cmd.arg("-f").arg(override_path);
                }
                build_cmd.arg("build").current_dir(&project_root);
                for svc in &plan.services_needing_build {
                    build_cmd.arg(svc);
                }
                build_cmd
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    build_cmd.creation_flags(0x08000200);
                }

                let mut build_child = build_cmd.spawn().map_err(|e| {
                    if let Some(p) = &cache_override {
                        let _ = std::fs::remove_file(p);
                    }
                    let err_msg = format!("failed to run docker compose build: {e}");
                    ui_log!(
                        app_handle,
                        error,
                        "commands::start_environment",
                        "{}",
                        err_msg
                    );
                    err_msg
                })?;

                let build_stderr = build_child.stderr.take();
                let app_build = app_handle.clone();
                let build_stderr_thread = build_stderr.map(|stderr| {
                    std::thread::spawn(move || {
                        use std::io::{BufRead, BufReader};
                        let reader = BufReader::new(stderr);
                        for line in reader.lines().map_while(Result::ok) {
                            if let Some(msg) = parse_docker_progress(&line) {
                                ui_log!(&app_build, info, "commands::start_environment", "{}", msg);
                            } else if !line.is_empty() {
                                ui_log!(
                                    &app_build,
                                    info,
                                    "commands::start_environment",
                                    "   {}",
                                    line
                                );
                            }
                        }
                    })
                });

                let build_status = build_child.wait().map_err(|e| {
                    if let Some(p) = &cache_override {
                        let _ = std::fs::remove_file(p);
                    }
                    let err_msg = format!("failed to wait for compose build: {e}");
                    ui_log!(
                        app_handle,
                        error,
                        "commands::start_environment",
                        "{}",
                        err_msg
                    );
                    err_msg
                })?;
                if let Some(t) = build_stderr_thread {
                    let _ = t.join();
                }
                if let Some(p) = &cache_override {
                    let _ = std::fs::remove_file(p);
                }
                if !build_status.success() {
                    let err_msg = format!(
                        "docker compose build failed (exit {:?})",
                        build_status.code()
                    );
                    ui_log!(
                        app_handle,
                        error,
                        "commands::start_environment",
                        "{}",
                        err_msg
                    );
                    return Err(err_msg);
                }
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "Image build finished"
                );
            }
            Ok(_) => {
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "Built images present; skipping compose build"
                );
            }
            Err(e) => {
                ui_log!(
                    app_handle,
                    warn,
                    "commands::start_environment",
                    "Build plan skipped: {}",
                    e
                );
            }
        }
    }

    // 后台启动容器（spawn 模式，实时读取 build/pull 进度）
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Running: docker compose up -d"
    );
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "First start may take a few minutes (images, extensions)..."
    );
    ui_log!(app_handle, info, "commands::start_environment", "");

    let mut compose_cmd = Command::new("docker");
    compose_cmd
        .args(["compose", "up", "-d"])
        .current_dir(&project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        compose_cmd.creation_flags(0x08000200);
    }

    // 使用 spawn 而非 output，这样 build/pull 过程中可以实时读取 stderr 进度
    let mut child = compose_cmd.spawn().map_err(|e| {
        let err_msg = format!("failed to run docker compose: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::start_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    let stdout_opt = child.stdout.take();
    let stderr_opt = child.stderr.take();
    let stderr_lines_shared = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    // stdout 线程：读取容器创建信息
    let app_stdout = app_handle.clone();
    let stdout_thread = stdout_opt.map(|stdout| {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if !line.is_empty() {
                    ui_log!(
                        &app_stdout,
                        info,
                        "commands::start_environment",
                        "   {}",
                        line
                    );
                }
            }
        })
    });

    // stderr 线程：读取 build/pull 进度（Docker Compose 的进度信息输出到 stderr）
    let app_stderr = app_handle.clone();
    let stderr_lines_clone = stderr_lines_shared.clone();
    let stderr_thread = stderr_opt.map(|stderr| {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if !line.is_empty() {
                    // Parse compose progress lines
                    let progress_msg = parse_docker_progress(&line);
                    if let Some(msg) = progress_msg {
                        ui_log!(&app_stderr, info, "commands::start_environment", "{}", msg);
                    } else {
                        ui_log!(
                            &app_stderr,
                            info,
                            "commands::start_environment",
                            "   {}",
                            line
                        );
                    }
                    if let Ok(mut lines) = stderr_lines_clone.lock() {
                        lines.push(line);
                    }
                }
            }
        })
    });

    // 等待 up -d 进程完成
    // R3: child.wait() 会阻塞 async 线程直到 up -d 结束，同样挪走
    let status = run_blocking_command(move || {
        child
            .wait()
            .map_err(|e| format!("failed waiting for docker compose: {e}"))
    })
    .await
    .map_err(|e| {
        ui_log!(app_handle, error, "commands::start_environment", "{}", e);
        e
    })?;

    if let Some(t) = stdout_thread {
        let _ = t.join();
    }
    if let Some(t) = stderr_thread {
        let _ = t.join();
    }

    // 启动失败：详细分析错误类型
    if !status.success() {
        let collected_stderr = stderr_lines_shared
            .lock()
            .map(|l| l.join("\n"))
            .unwrap_or_default();
        let exit_code = status.code();

        let is_port_conflict = collected_stderr.contains("port is already allocated")
            || collected_stderr.contains("Bind for")
            || collected_stderr.contains("address already in use");

        if is_port_conflict {
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "Port conflict detected"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Possible causes:"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "1. Another container is using the same port"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "2. A local service (MySQL, Nginx, ...) is running"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "How to fix:"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Option 1: stop the container using the port"
            );
            ui_log!(app_handle, info, "commands::start_environment", "docker ps");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "docker stop <name>"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Option 2: change the port in .env"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "e.g. MYSQL_PORT=3307"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Then apply config again"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Option 3: stop the local service"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Check for local MySQL/Nginx/Redis"
            );
            ui_log!(app_handle, info, "commands::start_environment", "");

            if let Some(line) = collected_stderr.lines().find(|l| l.contains("Bind for")) {
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "Detail: {}",
                    line.trim()
                );
            }
        } else {
            ui_log!(
                app_handle,
                error,
                "commands::start_environment",
                "Docker Compose failed, exit code: {:?}",
                exit_code
            );
            ui_log!(app_handle, info, "commands::start_environment", "");
            ui_log!(app_handle, info, "commands::start_environment", "Check:");
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "1. Docker Desktop is running"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "2. docker-compose.yml is valid"
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "3. Images exist and the network is OK"
            );
        }

        let err_msg = format!(
            "Docker Compose failed: {}",
            if is_port_conflict {
                "port conflict"
            } else {
                "unknown error"
            }
        );
        return Err(err_msg);
    }

    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Containers started in background"
    );
    ui_log!(app_handle, info, "commands::start_environment", "");

    // 第三步：轮询容器运行日志 + 检查就绪状态
    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Watching container logs..."
    );

    let docker_manager = match DockerManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "DockerManager failed: {}",
                e
            );
            ui_log!(
                app_handle,
                info,
                "commands::start_environment",
                "Skipping log watch"
            );
            return Ok("Environment started (container status not checked)".to_string());
        }
    };

    let mut last_line_count: usize = 0;
    let mut consecutive_failures: u32 = 0;
    let max_consecutive_failures = 10;
    let start_time = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(300);

    loop {
        if start_time.elapsed() > max_wait {
            ui_log!(
                app_handle,
                warn,
                "commands::start_environment",
                "Timed out waiting for ready (5 min)"
            );
            break;
        }

        // 1. 获取最新的容器日志
        match get_compose_logs(&project_root, last_line_count).await {
            Ok((new_lines, total_count)) => {
                if !new_lines.is_empty() {
                    for line in &new_lines {
                        let progress_msg = parse_docker_progress(line);
                        if let Some(msg) = progress_msg {
                            ui_log!(app_handle, info, "commands::start_environment", "{}", msg);
                        } else {
                            ui_log!(
                                app_handle,
                                info,
                                "commands::start_environment",
                                "   {}",
                                line
                            );
                        }
                    }
                    last_line_count = total_count;
                    consecutive_failures = 0;
                }
            }
            Err(e) => {
                consecutive_failures += 1;
                if consecutive_failures >= max_consecutive_failures {
                    ui_log!(
                        app_handle,
                        warn,
                        "commands::start_environment",
                        "Failed to fetch logs {} times, skipping: {}",
                        consecutive_failures,
                        e
                    );
                    break;
                }
            }
        }

        // 2. 检查所有容器是否就绪
        match docker_manager.check_all_ps_containers_running().await {
            Ok(true) => {
                ui_log!(
                    app_handle,
                    info,
                    "commands::start_environment",
                    "All containers ready"
                );
                if let Ok((final_lines, _)) = get_compose_logs(&project_root, last_line_count).await
                {
                    for line in &final_lines {
                        let progress_msg = parse_docker_progress(line);
                        if let Some(msg) = progress_msg {
                            ui_log!(app_handle, info, "commands::start_environment", "{}", msg);
                        } else {
                            ui_log!(
                                app_handle,
                                info,
                                "commands::start_environment",
                                "   {}",
                                line
                            );
                        }
                    }
                }
                break;
            }
            Ok(false) => {}
            Err(e) => {
                ui_log!(
                    app_handle,
                    warn,
                    "commands::start_environment",
                    "Failed to check container status: {}",
                    e
                );
            }
        }

        // 3. 每 2 秒检查一次
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    ui_log!(
        app_handle,
        info,
        "commands::start_environment",
        "Environment started"
    );
    Ok("Environment started".to_string())
}

/// 一键重启环境。
///
/// 使用 `docker compose up -d --force-recreate`，而不是 `compose restart`。
/// 后者只重启进程，不会套用新的 volumes。站点挂载变更后，Nginx 配置已经指向新目录，
/// 容器里却没有对应挂载，PHP 就会返回 File not found.
#[tauri::command]
pub async fn restart_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use crate::ui_log;
    use std::process::Command;
    use tauri::Emitter;

    ui_log!(
        app_handle,
        info,
        "commands::restart_environment",
        "Restarting environment..."
    );

    let project_root = get_project_root()?;
    ui_log!(
        app_handle,
        info,
        "commands::restart_environment",
        "Project root: {:?}",
        project_root
    );

    let compose_file = project_root.join("docker-compose.yml");

    if !compose_file.exists() {
        ui_log!(
            app_handle,
            error,
            "commands::restart_environment",
            "docker-compose.yml not found"
        );
        return Err("docker-compose.yml not found; apply config first".to_string());
    }

    ui_log!(
        app_handle,
        info,
        "commands::restart_environment",
        "docker-compose.yml found"
    );

    // compose restart 不会更新 volumes；站点目录变更后必须按新 compose 重建容器。
    ui_log!(
        app_handle,
        info,
        "commands::restart_environment",
        "Running: docker compose up -d --force-recreate"
    );

    let mut restart_cmd = Command::new("docker");
    restart_cmd
        .args(["compose", "up", "-d", "--force-recreate"])
        .current_dir(&project_root);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        restart_cmd.creation_flags(0x08000200);
    }

    let output = restart_cmd.output().map_err(|e| {
        let err_msg = format!("failed to run docker compose up: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::restart_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        ui_log!(
            app_handle,
            error,
            "commands::restart_environment",
            "Restart failed"
        );
        ui_log!(
            app_handle,
            info,
            "commands::restart_environment",
            "stderr: {}",
            stderr
        );
        return Err(format!("Docker Compose recreate failed: {stderr}"));
    }

    // 记录重启结果
    if !stdout.is_empty() {
        for line in stdout.lines() {
            if !line.is_empty() {
                ui_log!(
                    app_handle,
                    info,
                    "commands::restart_environment",
                    "   {}",
                    line
                );
            }
        }
    }

    ui_log!(
        app_handle,
        info,
        "commands::restart_environment",
        "Environment restarted"
    );
    Ok("Environment restarted".to_string())
}

/// 一键停止环境（docker compose stop）
#[tauri::command]
pub async fn stop_environment(app_handle: tauri::AppHandle) -> Result<String, String> {
    use crate::ui_log;
    use std::process::Command;
    use tauri::Emitter;

    ui_log!(
        app_handle,
        info,
        "commands::stop_environment",
        "Stopping environment..."
    );

    let project_root = get_project_root()?;
    ui_log!(
        app_handle,
        info,
        "commands::stop_environment",
        "Project root: {:?}",
        project_root
    );

    let compose_file = project_root.join("docker-compose.yml");

    if !compose_file.exists() {
        ui_log!(
            app_handle,
            error,
            "commands::stop_environment",
            "docker-compose.yml not found"
        );
        return Err("docker-compose.yml not found; apply config first".to_string());
    }

    ui_log!(
        app_handle,
        info,
        "commands::stop_environment",
        "docker-compose.yml found"
    );

    // 使用 docker compose stop 停止容器（保留容器，前端可显示"已停用"状态）
    ui_log!(
        app_handle,
        info,
        "commands::stop_environment",
        "Running: docker compose stop"
    );

    let mut stop_cmd = Command::new("docker");
    stop_cmd
        .args(["compose", "stop"])
        .current_dir(&project_root);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        stop_cmd.creation_flags(0x08000200);
    }

    let output = stop_cmd.output().map_err(|e| {
        let err_msg = format!("failed to run docker compose stop: {e}");
        ui_log!(
            app_handle,
            error,
            "commands::stop_environment",
            "{}",
            err_msg
        );
        err_msg
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        ui_log!(
            app_handle,
            error,
            "commands::stop_environment",
            "Stop failed"
        );
        ui_log!(
            app_handle,
            info,
            "commands::stop_environment",
            "stderr: {}",
            stderr
        );
        return Err(format!("Docker Compose stop failed: {stderr}"));
    }

    // 记录停止结果
    if !stdout.is_empty() {
        for line in stdout.lines() {
            if !line.is_empty() {
                ui_log!(
                    app_handle,
                    info,
                    "commands::stop_environment",
                    "   {}",
                    line
                );
            }
        }
    }

    ui_log!(
        app_handle,
        info,
        "commands::stop_environment",
        "Environment stopped"
    );
    Ok("Environment stopped".to_string())
}

#[cfg(test)]
mod tests {
    use super::collect_unique_image_tags;
    use super::parse_env_to_services;
    use super::run_blocking_command;
    use super::PullImageResult;
    use crate::engine::config_generator::{EnvConfig, ServiceEntry};
    use crate::engine::env_parser::EnvFile;
    use crate::engine::user_override_manager::UserOverrideManager;
    use crate::engine::version_manifest::VersionManifest;

    #[test]
    fn test_collect_unique_image_tags_dedupes_same_tag() {
        // 两条服务解析到同一 image_tag 时只保留一条（不依赖 docker）
        let tmp = tempfile::tempdir().unwrap();
        let override_manager = UserOverrideManager::new(tmp.path());
        let manifest = VersionManifest::new();
        let tag = resolve_expected_mysql80_tag(&manifest);

        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "mysql".to_string(),
                    version: "mysql80".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "mysql".to_string(),
                    version: "mysql80".to_string(),
                    host_port: 3307,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let tags = collect_unique_image_tags(&config, &override_manager, &manifest);
        assert_eq!(tags, vec![tag]);
    }

    fn resolve_expected_mysql80_tag(manifest: &VersionManifest) -> String {
        manifest
            .get_entry("mysql", "mysql80")
            .expect("builtin mysql80")
            .image_tag
            .clone()
    }

    /// R3: 同步阻塞调用统一走 `run_blocking_command`。
    /// 「在阻塞线程池执行」由 tokio 保证，这里锁住的是对外契约：
    /// 闭包的成功/失败结果必须原样透传给调用方，不能被包装或吞掉。
    #[test]
    fn test_run_blocking_command_passes_through_result() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("创建多线程 runtime");

        let ok: Result<i32, String> = rt.block_on(run_blocking_command(|| Ok(42)));
        assert_eq!(ok.expect("成功分支应返回值"), 42);

        let err: Result<i32, String> =
            rt.block_on(run_blocking_command(|| Err("boom".to_string())));
        assert_eq!(err.expect_err("失败分支应返回错误"), "boom");
    }

    fn parse_env(content: &str) -> std::collections::HashMap<String, String> {
        EnvFile::parse(content)
            .expect("解析 .env 内容失败")
            .to_map()
    }

    /// 测试解析多版本 Redis
    #[test]
    fn test_parse_env_to_services_multi_redis() {
        let env_map = parse_env(
            "SOURCE_DIR=./www\nTZ=Asia/Shanghai\nREDIS62_VERSION=6.2-alpine-01\nREDIS62_HOST_PORT=6379\nREDIS72_VERSION=7.2-alpine\nREDIS72_HOST_PORT=6380\n",
        );
        let manifest = VersionManifest::new();
        let services = parse_env_to_services(&env_map, &manifest, None, std::path::Path::new("."));

        let redis: Vec<_> = services
            .iter()
            .filter(|s| s.service_type == "redis")
            .collect();
        assert_eq!(
            redis.len(),
            2,
            "应解析出 2 个 Redis 服务，实际: {services:?}"
        );

        let ports: Vec<u16> = redis.iter().map(|s| s.host_port).collect();
        assert!(ports.contains(&6379), "REDIS62 端口应为 6379");
        assert!(ports.contains(&6380), "REDIS72 端口应为 6380");
    }

    /// 测试解析多版本 Nginx
    #[test]
    fn test_parse_env_to_services_multi_nginx() {
        let env_map = parse_env(
            "SOURCE_DIR=./www\nTZ=Asia/Shanghai\nNGINX127_VERSION=1.27-alpine\nNGINX127_HTTP_HOST_PORT=80\nNGINX125_VERSION=1.25-alpine\nNGINX125_HTTP_HOST_PORT=8080\n",
        );
        let manifest = VersionManifest::new();
        let services = parse_env_to_services(&env_map, &manifest, None, std::path::Path::new("."));

        let nginx: Vec<_> = services
            .iter()
            .filter(|s| s.service_type == "nginx")
            .collect();
        assert_eq!(
            nginx.len(),
            2,
            "应解析出 2 个 Nginx 服务，实际: {services:?}"
        );

        let ports: Vec<u16> = nginx.iter().map(|s| s.host_port).collect();
        assert!(ports.contains(&80), "NGINX127 端口应为 80");
        assert!(ports.contains(&8080), "NGINX125 端口应为 8080");

        // manifest 收录的版本应反查出 ID，未收录的回退为小写前缀
        let ids: Vec<&str> = nginx.iter().map(|s| s.version.as_str()).collect();
        assert!(ids.contains(&"nginx127"), "NGINX127 应反查为 ID nginx127");
        assert!(ids.contains(&"nginx125"), "NGINX125 应反查为 ID nginx125");
    }

    /// 测试解析混合服务（manifest 驱动，按 service_dir 匹配）
    #[test]
    fn test_parse_env_to_services_mixed_services() {
        let env_map = parse_env(
            "SOURCE_DIR=./www\nTZ=Asia/Shanghai\nPHP85_VERSION=8.5\nPHP85_HOST_PORT=9000\nPHP85_EXTENSIONS=mysqli,mbstring\nMYSQL84_VERSION=8.4\nMYSQL84_HOST_PORT=3306\nREDIS62_VERSION=6.2-alpine-01\nREDIS62_HOST_PORT=6379\nNGINX127_VERSION=1.27-alpine\nNGINX127_HTTP_HOST_PORT=80\n",
        );
        let manifest = VersionManifest::new();
        let services = parse_env_to_services(&env_map, &manifest, None, std::path::Path::new("."));

        assert_eq!(services.len(), 4, "应解析出 4 个服务，实际: {services:?}");

        let php = services
            .iter()
            .find(|s| s.service_type == "php")
            .expect("应包含 PHP 服务");
        assert_eq!(php.version, "php85", "PHP85 应反查为 ID php85");
        assert_eq!(php.host_port, 9000);
        let php_exts = php.extensions.as_ref().expect("PHP 服务应有扩展列表");
        assert_eq!(
            php_exts,
            &["mysqli".to_string(), "mbstring".to_string()],
            "扩展列表应按逗号拆分"
        );

        let mysql = services
            .iter()
            .find(|s| s.service_type == "mysql")
            .expect("应包含 MySQL 服务");
        assert_eq!(mysql.version, "mysql84");
        assert_eq!(mysql.host_port, 3306);

        // MYSQL_ROOT_PASSWORD 不应被误识别为 MySQL 服务
        let with_root = parse_env("MYSQL_ROOT_PASSWORD=secret\n");
        let no_services = parse_env_to_services(
            &with_root,
            &VersionManifest::new(),
            None,
            std::path::Path::new("."),
        );
        assert!(
            no_services.is_empty(),
            "仅 ROOT_PASSWORD 时不应解析出服务，实际: {no_services:?}"
        );
    }
    // ─── 前后端 serde 契约 ──────────────────────────────────────
    // 前端 `PullImageResultItem` 按 `success` 判定成功/失败、按 `error` 展示原因。
    // pull_service_images 的失败是「部分失败也继续」语义，因此 error 字段的
    // 有无必须稳定：成功时省略（skip_serializing_if），失败时必带。

    #[test]
    fn test_pull_image_result_serde_contract_success() {
        let ok = PullImageResult {
            tag: "mysql:8.4".to_string(),
            success: true,
            error: None,
        };
        let json: serde_json::Value = serde_json::to_value(&ok).unwrap();
        assert_eq!(json["tag"], "mysql:8.4");
        assert_eq!(json["success"], true);
        assert!(
            json.get("error").is_none(),
            "成功时 error 应被省略（skip_serializing_if），前端依赖此形态"
        );
    }

    #[test]
    fn test_pull_image_result_serde_contract_failure() {
        let failed = PullImageResult {
            tag: "redis:8.2-alpine".to_string(),
            success: false,
            error: Some("manifest unknown".to_string()),
        };
        let json: serde_json::Value = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["tag"], "redis:8.2-alpine");
        assert_eq!(json["success"], false);
        assert_eq!(json["error"], "manifest unknown");
    }
}
