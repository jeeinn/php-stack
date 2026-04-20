pub mod docker;
pub mod commands;
pub mod engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      app.handle().plugin(tauri_plugin_dialog::init())?;
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      // Dashboard
      commands::check_docker,
      commands::list_containers,
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
      // 统一镜像源管理
      commands::get_mirror_presets,
      commands::apply_mirror_preset,
      commands::update_single_mirror,
      commands::test_mirror,
      commands::get_mirror_status,
      commands::get_current_mirror_preset,
      // 备份
      commands::create_backup,
      // 恢复
      commands::preview_restore,
      commands::verify_backup,
      commands::execute_restore,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
