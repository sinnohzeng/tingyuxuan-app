mod commands;
mod platform;
mod recorder_actor;
mod state;
mod tray;

use state::AppStates;
use tauri::{Emitter, Manager};
use tingyuxuan_core::pipeline::events::PipelineEvent;

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _log_guard = init_tracing();
    let sentry_client = init_sentry();

    tauri::Builder::default()
        // 允许设备事件在所有窗口焦点状态下传递，确保 WH_KEYBOARD_LL
        // 钩子在 Tauri/WebView2 窗口获焦时仍能接收键盘事件。
        // 默认值 Unfocused 会导致 Tauri 窗口获焦时钩子收不到事件。
        // 参考: https://github.com/tauri-apps/tauri/issues/13919
        .device_event_filter(tauri::DeviceEventFilter::Never)
        .plugin(tauri_plugin_sentry::init(&sentry_client))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
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
            // 会话阶段状态 — 在 event bridge 和 handle_shortcut_action 之间共享。
            let session_phase = std::sync::Arc::new(tokio::sync::RwLock::new(
                state::SessionPhase::Idle,
            ));
            let phase_for_bridge = session_phase.clone();

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
                        // 诊断日志：非 VolumeUpdate 事件提升到 info 级别
                        if !matches!(&event, PipelineEvent::VolumeUpdate { .. }) {
                            tracing::info!(?event, "Event bridge: received pipeline event");
                        }

                        // Window visibility management.
                        if let Some(window) = handle.get_webview_window("floating-bar") {
                            match &event {
                                PipelineEvent::RecordingStarted { .. } => {
                                    position_floating_bar(&window);
                                    let result = window.show();
                                    tracing::info!(?result, "Event bridge: floating-bar window.show()");
                                }
                                PipelineEvent::Error { .. } => {
                                    position_floating_bar(&window);
                                    let _ = window.show();
                                }
                                PipelineEvent::ProcessingComplete { .. } => {
                                    // Auto-hide after a short delay (frontend handles the
                                    // 1.5s "done" display then calls window.hide()).
                                }
                                _ => {}
                            }
                        } else {
                            tracing::warn!("Event bridge: floating-bar window NOT FOUND");
                        }

                        // 会话阶段复位：处理完成/错误/取消后回到 Idle。
                        match &event {
                            PipelineEvent::ProcessingComplete { .. }
                            | PipelineEvent::Error { .. }
                            | PipelineEvent::RecordingCancelled => {
                                *phase_for_bridge.write().await = state::SessionPhase::Idle;
                                tracing::info!("Session phase → Idle");
                            }
                            _ => {}
                        }

                        // Track network status.
                        if let PipelineEvent::NetworkStatusChanged { online } = &event {
                            network_flag.store(*online, std::sync::atomic::Ordering::Release);
                        }

                        // Forward all events to the frontend.
                        if let Err(e) = handle.emit("pipeline-event", &event) {
                            tracing::error!(?e, "Failed to emit pipeline-event to frontend");
                        }
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
                let monitor_token =
                    tauri::async_runtime::block_on(async move { monitor.start(event_bus_clone) });
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
            app.manage(states.tray);

            // Initialize telemetry backend (SLS or noop).
            let app_version = env!("CARGO_PKG_VERSION");
            let telemetry_backend = tingyuxuan_core::telemetry::sls::create_backend(app_version);
            app.manage(state::TelemetryState(telemetry_backend));

            // 会话阶段状态（与 event bridge 共享同一个 Arc）。
            app.manage(state::SessionPhaseState(session_phase));

            // Set up system tray.
            tray::create_tray(app.handle())?;

            tracing::info!("TingYuXuan started successfully");
            Ok(())
        })
        // 关闭主窗口时：根据配置决定隐藏到托盘还是正常关闭。
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                let config_state = window.state::<state::ConfigState>();
                let minimize = config_state.0.blocking_read().general.minimize_to_tray;
                if minimize {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
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
            commands::report_telemetry_event,
            commands::list_input_devices,
            commands::set_input_device,
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

/// 将浮动条窗口定位到屏幕底部居中（任务栏上方约 60 逻辑像素）。
fn position_floating_bar(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let phys = monitor.size();
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let win_w = 360.0_f64;
        let win_h = 80.0_f64;
        let taskbar_margin = 60.0_f64;

        let logical_w = phys.width as f64 / scale;
        let logical_h = phys.height as f64 / scale;
        let origin_x = origin.x as f64 / scale;
        let origin_y = origin.y as f64 / scale;

        let x = origin_x + (logical_w - win_w) / 2.0;
        let y = origin_y + logical_h - win_h - taskbar_margin;

        if let Err(e) = window.set_position(tauri::Position::Logical(
            tauri::LogicalPosition::new(x, y),
        )) {
            tracing::warn!(?e, "Failed to position floating bar");
        }
    }
}

/// 快捷键动作处理 — 基于 SessionPhase 状态机。
///
/// 状态机: Idle → Recording → Processing → Idle
/// - Idle + RAlt → 开始录音
/// - Recording + RAlt（>800ms） → 停止录音
/// - Recording + RAlt（<800ms） → 忽略（防误触）
/// - Processing + RAlt → 忽略（等待处理完毕）
/// - 任意状态 + Cancel → 取消
pub(crate) async fn handle_shortcut_action(handle: &tauri::AppHandle, action: &str) {
    use crate::state::{SessionPhase, SessionPhaseState};

    /// 防误触最短录音时长（毫秒）。
    const DEBOUNCE_MS: u64 = 800;

    let phase_state = handle.state::<SessionPhaseState>();

    match action {
        "cancel" => {
            let mut phase = phase_state.0.write().await;
            if matches!(*phase, SessionPhase::Idle) {
                return;
            }
            *phase = SessionPhase::Idle;
            drop(phase);
            tracing::info!("Shortcut: cancel");
            let _ = handle.emit("shortcut-action", "cancel");
        }
        mode @ ("dictate" | "translate" | "ai_assistant") => {
            let mut phase = phase_state.0.write().await;
            match *phase {
                SessionPhase::Idle => {
                    *phase = SessionPhase::Recording {
                        started_at: std::time::Instant::now(),
                    };
                    drop(phase);
                    tracing::info!(mode, "Shortcut: start recording");
                    let _ = handle.emit("shortcut-action", mode);
                }
                SessionPhase::Recording { started_at } => {
                    let elapsed = started_at.elapsed();
                    if elapsed < std::time::Duration::from_millis(DEBOUNCE_MS) {
                        tracing::info!(
                            elapsed_ms = elapsed.as_millis(),
                            "Shortcut: debounce (too quick, ignoring)"
                        );
                        return;
                    }
                    *phase = SessionPhase::Processing;
                    drop(phase);
                    tracing::info!("Shortcut: stop recording");
                    let _ = handle.emit("shortcut-action", "stop");
                }
                SessionPhase::Processing => {
                    tracing::info!("Shortcut: ignoring (processing in progress)");
                }
            }
        }
        _ => {}
    }
}

/// 初始化 Sentry 错误/崩溃上报。
///
/// DSN 从环境变量 `SENTRY_DSN` 读取。未设置时 Sentry 处于禁用状态（零开销）。
/// 先用 sentry.io SaaS 免费版快速跑通，上线前可迁移到自托管（只需改 DSN）。
fn init_sentry() -> sentry::ClientInitGuard {
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
    sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(
                if cfg!(debug_assertions) {
                    "development"
                } else {
                    "production"
                }
                .into(),
            ),
            auto_session_tracking: true,
            sample_rate: 1.0,
            traces_sample_rate: 0.2,
            ..Default::default()
        },
    ))
}

/// 初始化 tracing subscriber：终端 + 可选日志文件。
/// 返回 guard 以保证文件写入器在 app 生命周期内存活。
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "tingyuxuan=info".into());

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
