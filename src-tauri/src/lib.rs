mod commands;
mod context;
mod recorder_actor;
mod state;
mod text_injector;
mod tray;

use state::AppStates;
use tauri::Manager;
use tingyuxuan_core::pipeline::events::PipelineEvent;
use tingyuxuan_core::pipeline::ProcessingRequest;

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
            // Build all application states (split into independent managed states).
            let states = AppStates::new()?;

            // Try to build pipeline from existing config + API keys.
            {
                let config = states.config.0.blocking_read();
                let pipeline = commands::build_pipeline(&config, &states.event_bus.0);
                *states.pipeline.0.blocking_write() = pipeline;
            }

            // ----------------------------------------------------------
            // Event bridge: forward pipeline events to the frontend,
            // manage floating-bar window visibility, and handle
            // network status changes (queue drain on restore).
            // ----------------------------------------------------------
            let mut event_rx = states.event_bus.0.subscribe();
            let handle = app.handle().clone();
            let network_flag = states.network.0.clone();
            let queue_arc = states.queue.0.clone();
            let pipeline_arc = states.pipeline.0.clone();
            let history_arc = states.history.0.clone();
            let event_tx_clone = states.event_bus.0.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    // Window visibility management.
                    if let Some(window) = handle.get_webview_window("floating-bar") {
                        match &event {
                            PipelineEvent::RecordingStarted { .. } => {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            PipelineEvent::ProcessingComplete { .. } => {
                                // Auto-hide after a short delay (frontend handles the
                                // 1.5s "done" display then calls window.hide()).
                            }
                            _ => {}
                        }
                    }

                    // Track network status and drain offline queue on restore.
                    if let PipelineEvent::NetworkStatusChanged { online } = &event {
                        network_flag.store(*online, std::sync::atomic::Ordering::Relaxed);

                        if *online {
                            // Network is back — drain the offline queue and process.
                            let queued_items = queue_arc.lock().await.drain();
                            if !queued_items.is_empty() {
                                tracing::info!(
                                    "Network restored — processing {} queued recording(s)",
                                    queued_items.len()
                                );
                                let pipeline_opt = pipeline_arc.read().await.clone();
                                if let Some(pipeline) = pipeline_opt {
                                    for item in queued_items {
                                        let p = pipeline.clone();
                                        let h = history_arc.clone();
                                        let tx = event_tx_clone.clone();
                                        tokio::spawn(async move {
                                            let request = ProcessingRequest {
                                                audio_path: item.audio_path,
                                                mode: item.mode,
                                                app_context: item.app_context,
                                                target_language: item.target_language,
                                                selected_text: item.selected_text,
                                                user_dictionary: Vec::new(),
                                            };
                                            let cancel = tokio_util::sync::CancellationToken::new();
                                            match p.process_audio(&request, cancel).await {
                                                Ok(processed_text) => {
                                                    let _ = tx.send(PipelineEvent::ProcessingComplete {
                                                        processed_text: processed_text.clone(),
                                                    });
                                                    if let Ok(h) = h.try_lock() {
                                                        let _ = h.update_result(
                                                            &item.session_id,
                                                            &processed_text,
                                                        );
                                                    }
                                                    tracing::info!(
                                                        "Queued recording processed: {}",
                                                        item.session_id
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::error!(
                                                        "Failed to process queued recording {}: {}",
                                                        item.session_id,
                                                        e
                                                    );
                                                    if let Ok(h) = h.try_lock() {
                                                        let _ = h.update_status(
                                                            &item.session_id,
                                                            "failed",
                                                        );
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Forward all events to the frontend.
                    let _ = handle.emit("pipeline-event", &event);
                }
            });

            // ----------------------------------------------------------
            // Global shortcuts
            // ----------------------------------------------------------
            register_global_shortcuts(app)?;

            // ----------------------------------------------------------
            // Network monitor: check connectivity every 30s
            // ----------------------------------------------------------
            {
                let config = states.config.0.blocking_read();
                let check_url = config
                    .stt
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com".to_string());
                drop(config);

                let monitor =
                    tingyuxuan_core::pipeline::network::NetworkMonitor::new(check_url);
                let _monitor_token = monitor.start(states.event_bus.0.clone());
                // _monitor_token is dropped when the app exits, stopping the monitor.
            }

            // ----------------------------------------------------------
            // Recovery check: scan for unfinished recordings
            // ----------------------------------------------------------
            {
                let handle2 = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data_dir) = tingyuxuan_core::config::AppConfig::data_dir() {
                        let cache_dir = data_dir.join("cache").join("audio");
                        let unfinished =
                            tingyuxuan_core::pipeline::recovery::scan_unfinished_recordings(
                                &cache_dir,
                            );
                        if !unfinished.is_empty() {
                            tracing::info!(
                                "Found {} unfinished recording(s) from previous session",
                                unfinished.len()
                            );
                            let _ = handle2.emit("recovery-available", &unfinished);
                        }
                    }
                });
            }

            // Register each state as a separate Tauri managed state.
            app.manage(states.config);
            app.manage(states.history);
            app.manage(states.pipeline);
            app.manage(states.event_bus);
            app.manage(states.session);
            app.manage(states.recorder);
            app.manage(states.queue);
            app.manage(states.network);

            // Set up system tray.
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
            commands::save_api_key,
            commands::get_api_key,
            commands::inject_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TingYuXuan");
}

/// Register global keyboard shortcuts for recording control.
///
/// If a shortcut fails to register (e.g. another app has claimed it, or we're
/// on Wayland where global shortcuts may not work), we log a warning but do NOT
/// abort startup.
fn register_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let display = text_injector::detect_display_server();
    if display == text_injector::DisplayServer::Wayland {
        tracing::warn!(
            "Running on Wayland — global shortcuts may not work. \
             Consider using the system tray or in-app buttons."
        );
    }

    // Define shortcuts: (shortcut, action_name, mode)
    let shortcuts = [
        (
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyD),
            "dictate",
        ),
        (
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT),
            "translate",
        ),
        (
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyA),
            "ai_assistant",
        ),
        (
            Shortcut::new(None, Code::Escape),
            "cancel",
        ),
    ];

    let handle = app.handle().clone();

    for (shortcut, action) in shortcuts {
        let h = handle.clone();
        let action_name = action.to_string();

        if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let h2 = h.clone();
            let mode = action_name.clone();
            tauri::async_runtime::spawn(async move {
                handle_shortcut_action(&h2, &mode).await;
            });
        }) {
            tracing::warn!(
                "Failed to register shortcut for '{}': {}. Another app may have claimed it.",
                action,
                e
            );
        }
    }

    Ok(())
}

/// Handle a global shortcut action by invoking the appropriate recording command.
async fn handle_shortcut_action(handle: &tauri::AppHandle, action: &str) {
    use crate::state::{RecorderState, SessionState};

    match action {
        "cancel" => {
            // Cancel is always safe to call — it's a no-op if not recording.
            let _ = handle.emit("shortcut-action", "cancel");
        }
        mode @ ("dictate" | "translate" | "ai_assistant") => {
            // Toggle behaviour: if already recording, stop; otherwise start.
            let recorder = handle.state::<RecorderState>();
            let is_recording = recorder.0.is_recording().await;

            if is_recording {
                let _ = handle.emit("shortcut-action", "stop");
            } else {
                let _ = handle.emit("shortcut-action", mode);
            }
        }
        _ => {}
    }
}
