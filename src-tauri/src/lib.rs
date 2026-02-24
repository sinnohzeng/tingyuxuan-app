mod commands;
mod context;
mod state;
mod text_injector;
mod tray;

use state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tingyuxuan=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_state = AppState::new()?;
            app.manage(Arc::new(Mutex::new(app_state)));

            // Set up system tray
            tray::create_tray(app.handle())?;

            tracing::info!("TingYuXuan started successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::cancel_recording,
            commands::get_config,
            commands::save_config,
            commands::test_stt_connection,
            commands::test_llm_connection,
            commands::get_recent_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TingYuXuan");
}
