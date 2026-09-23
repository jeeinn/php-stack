pub mod commands;
pub mod docker;
pub mod engine;
pub mod logging;
#[macro_use]
pub mod macros;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;

            // 用户级配置与日志统一落在 Tauri 官方应用数据目录：
            // Windows %APPDATA%\<identifier>、macOS ~/Library/Application Support/<identifier>。
            // 此前写在 exe 同级目录，装进 Program Files 后无写权限。
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {e}"))?;
            commands::paths::init_app_data_dir(app_data.clone());

            // 旧版本把 workspace.json 放在 exe 同级目录，首次启动自动搬迁
            let migrated = commands::paths::migrate_legacy_config();

            // 初始化日志系统
            if let Err(e) = logging::init_logging(&app_data) {
                eprintln!("Failed to initialize logging: {e}");
            }

            let log_path =
                commands::paths::log_file().unwrap_or_else(|_| app_data.join("php-stack.log"));

            app_log!(
                info,
                "app",
                "PHP-Stack started, log file at: {:?}",
                log_path
            );

            if !migrated.is_empty() {
                app_log!(
                    info,
                    "app",
                    "migrated user config to {:?}: {}",
                    app_data,
                    migrated.join(", ")
                );
            }

            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle()
                .plugin(tauri_plugin_clipboard_manager::init())?;
            app.handle().plugin(tauri_plugin_shell::init())?;
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.handle().plugin(tauri_plugin_process::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Dashboard
            commands::check_docker,
            commands::list_containers,
            commands::list_all_running_containers,
            commands::start_container,
            commands::stop_container,
            commands::restart_container,
            commands::open_service_config,
            // 可视化配置生成
            commands::load_existing_config,
            commands::validate_env_config,
            commands::generate_env_config,
            commands::preview_compose,
            commands::check_config_files_exist,
            commands::apply_env_config,
            commands::start_environment,
            commands::restart_environment,
            commands::stop_environment,
            // Phase 3: 镜像探测 / 拉取 / 配置提取
            commands::check_service_images_presence,
            commands::pull_service_images,
            commands::extract_service_config,
            // 统一镜像源管理
            commands::get_mirror_presets,
            commands::apply_mirror_preset,
            commands::update_single_mirror,
            commands::test_mirror,
            commands::get_mirror_status,
            commands::get_current_mirror_preset,
            // 增强镜像源管理
            commands::get_merged_mirror_list,
            commands::save_selected_mirror_option,
            commands::save_user_mirror_category,
            commands::remove_user_mirror_category,
            commands::reset_all_mirror_overrides,
            // 备份
            commands::create_backup,
            commands::convert_to_relative_path,
            // 恢复
            commands::preview_restore,
            commands::verify_backup,
            commands::execute_restore,
            // 工作目录管理
            commands::get_workspace_info,
            commands::set_workspace_path,
            commands::recreate_workspace_dir,
            // 版本管理
            commands::get_version_mappings,
            commands::validate_version,
            commands::get_recommended_version,
            // 用户版本覆盖
            commands::save_user_override,
            commands::remove_user_override,
            commands::reset_all_overrides,
            // 日志导出
            commands::export_logs,
            commands::export_logs_to,
            commands::get_support_info,
            // 前端错误上报（白屏兜底，见 public/boot-guard.js）
            commands::log_frontend_error,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
