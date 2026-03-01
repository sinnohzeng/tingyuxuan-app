mod commands;
mod platform;
mod recorder_actor;
mod state;
mod tray;

use state::AppStates;
use tauri::{Emitter, Manager};
use tingyuxuan_core::pipeline::events::PipelineEvent;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _log_guard = init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init()) // 用于 shell:default 权能（未来可能需要 open URL）
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
            // manage floating-bar window visibility, and track
            // network status changes.
            // ----------------------------------------------------------
            let mut event_rx = states.event_bus.0.subscribe();
            let handle = app.handle().clone();
            let network_flag = states.network.0.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let event = match event_rx.recv().await {
                        Ok(event) => event,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Event bridge lagged, skipped {n} event(s)");
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            tracing::info!("Event bus closed, stopping event bridge");
                            break;
                        }
                    };
                    {
                        // Window visibility management.
                        if let Some(window) = handle.get_webview_window("floating-bar") {
                            match &event {
                                PipelineEvent::RecordingStarted { .. } => {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                                PipelineEvent::Error { .. } => {
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

                        // Track network status.
                        if let PipelineEvent::NetworkStatusChanged { online } = &event {
                            network_flag.store(*online, std::sync::atomic::Ordering::Release);
                        }

                        // Forward all events to the frontend.
                        if !matches!(&event, PipelineEvent::VolumeUpdate { .. }) {
                            tracing::debug!("Forwarding pipeline event to frontend");
                        }
                        let _ = handle.emit("pipeline-event", &event);
                    }
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
                    .llm
                    .base_url
                    .clone()
                    .unwrap_or_else(|| config.llm_base_url());
                drop(config);

                let monitor = tingyuxuan_core::pipeline::network::NetworkMonitor::new(check_url);
                let event_bus_clone = states.event_bus.0.clone();
                // setup 回调运行在主线程，无 tokio runtime 上下文。
                // 通过 block_on 进入异步运行时，使 NetworkMonitor 内部的 tokio::spawn 生效。
                let monitor_token = tauri::async_runtime::block_on(async move {
                    monitor.start(event_bus_clone)
                });
                // Store monitor token in managed state to keep it alive.
                app.manage(state::MonitorState(monitor_token));
            }

            // Register each state as a separate Tauri managed state.
            app.manage(states.config);
            app.manage(states.history);
            app.manage(states.pipeline);
            app.manage(states.event_bus);
            app.manage(states.session);
            app.manage(states.recorder);
            app.manage(states.network);
            app.manage(states.injector);
            app.manage(states.detector);

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
            commands::test_llm_connection,
            commands::get_recent_history,
            commands::save_api_key,
            commands::get_api_key,
            commands::inject_text,
            commands::get_dictionary,
            commands::add_dictionary_word,
            commands::remove_dictionary_word,
            commands::search_history,
            commands::get_history_page,
            commands::delete_history,
            commands::delete_history_batch,
            commands::clear_history,
            commands::is_first_launch,
            commands::get_dashboard_stats,
            commands::check_platform_permissions,
            commands::open_permission_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TingYuXuan");
}

/// Register global keyboard shortcuts for recording control.
///
/// If a shortcut fails to register (e.g. another app has claimed it, or we're
/// on Wayland where global shortcuts may not work), we log a warning but do NOT
/// abort startup.
///
/// 三平台分治：
/// - macOS: Fn 键通过 CGEventTap 监听，其余通过 tauri-plugin-global-shortcut
/// - Windows: RAlt/Shift+RAlt 通过 WH_KEYBOARD_LL 钩子，其余通过 tauri-plugin-global-shortcut
/// - Linux: 全部通过 tauri-plugin-global-shortcut（未来用 evdev 替换）
fn register_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        match platform::macos::register_platform_hotkeys(app) {
            Ok(fn_monitor) => {
                // 保持 FnKeyMonitor 存活（存入 managed state）
                if let Some(monitor) = fn_monitor {
                    app.manage(state::FnKeyMonitorState(Some(monitor)));
                }
            }
            Err(e) => {
                tracing::warn!("Failed to register macOS platform hotkeys: {e}");
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        match platform::windows::register_platform_hotkeys(app) {
            Ok(monitor) => {
                // 保持 RAltKeyMonitor 存活（存入 managed state）
                app.manage(state::RAltKeyMonitorState(Some(monitor)));
            }
            Err(e) => {
                tracing::warn!("Failed to register Windows platform hotkeys: {e}");
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        register_linux_shortcuts(app)
    }
}

/// Linux 标准快捷键注册（全部通过 tauri-plugin-global-shortcut）。
#[cfg(target_os = "linux")]
fn register_linux_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    {
        let display = platform::linux::detect_display_server();
        if display == platform::linux::DisplayServer::Wayland {
            tracing::warn!(
                "Running on Wayland — global shortcuts may not work. \
                 Consider using the system tray or in-app buttons."
            );
        }
    }

    // Default shortcuts: RAlt (dictate), Shift+RAlt (translate), Alt+Space (AI assistant).
    // NOTE: RAlt-alone may conflict with RAlt+key combos on some platforms.
    let shortcuts = [
        (Shortcut::new(None, Code::AltRight), "dictate"),
        (
            Shortcut::new(Some(Modifiers::SHIFT), Code::AltRight),
            "translate",
        ),
        (
            Shortcut::new(Some(Modifiers::ALT), Code::Space),
            "ai_assistant",
        ),
        (Shortcut::new(None, Code::Escape), "cancel"),
    ];

    let handle = app.handle().clone();

    for (shortcut, action) in shortcuts {
        let h = handle.clone();
        let action_name = action.to_string();

        if let Err(e) = app
            .global_shortcut()
            .on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let h2 = h.clone();
                let mode = action_name.clone();
                tauri::async_runtime::spawn(async move {
                    handle_shortcut_action(&h2, &mode).await;
                });
            })
        {
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
    use crate::state::RecorderState;

    tracing::debug!(action, "Shortcut triggered");

    match action {
        "cancel" => {
            // Cancel is always safe to call — it's a no-op if not recording.
            let _ = handle.emit("shortcut-action", "cancel");
        }
        mode @ ("dictate" | "translate" | "ai_assistant") => {
            // Toggle behaviour: if already recording, stop; otherwise start.
            let recorder = handle.state::<RecorderState>();
            let is_recording = recorder.0.is_recording().await;
            tracing::debug!(mode, is_recording, "Toggle recording");

            if is_recording {
                let _ = handle.emit("shortcut-action", "stop");
            } else {
                let _ = handle.emit("shortcut-action", mode);
            }
        }
        _ => {}
    }
}

/// 初始化 tracing subscriber：终端 + 可选日志文件。
/// 返回 guard 以保证文件写入器在 app 生命周期内存活。
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| "tingyuxuan=info".into());

    // 终端输出层（开发时看这个）
    let stderr_layer = fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);

    // 文件输出层（用户 bug report 时看这个）
    let (file_layer, guard) = match tingyuxuan_core::config::AppConfig::data_dir() {
        Ok(data_dir) => {
            let log_dir = data_dir.join("logs");
            let _ = std::fs::create_dir_all(&log_dir);
            let file_appender = tracing_appender::rolling::daily(log_dir, "tingyuxuan.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);
            (Some(layer), Some(guard))
        }
        Err(_) => (None, None),
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer) // Option<Layer> impl Layer — None 时为 noop
        .init();

    guard
}
