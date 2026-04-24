pub mod docker;
pub mod commands;
pub mod engine;
pub mod logging;
#[macro_use]
pub mod macros;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 获取项目根目录（优先 workspace.json，否则 exe 同级目录）
            let log_dir = if cfg!(debug_assertions) {
                // 开发模式：使用项目根目录
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).and_then(|p| p.parent()).map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            } else {
                // 生产模式：使用可执行文件所在目录
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            };
            
            // 初始化日志系统
            if let Err(e) = logging::init_logging(&log_dir) {
                eprintln!("Failed to initialize logging: {}", e);
            }
            
            app_log!(info, "app", "PHP-Stack 启动，日志文件位于: {:?}", log_dir.join("php-stack.log"));
            
            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_clipboard_manager::init())?;
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
      commands::select_project_folder,
      commands::convert_to_relative_path,
      // 恢复
      commands::preview_restore,
      commands::verify_backup,
      commands::execute_restore,
      // 工作目录管理
      commands::get_workspace_info,
      commands::set_workspace_path,
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
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
