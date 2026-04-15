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
      commands::check_docker,
      commands::list_containers,
      commands::start_container,
      commands::stop_container,
      commands::restart_container,
      commands::set_docker_mirror,
      commands::export_stack
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
