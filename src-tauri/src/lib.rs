mod commands;
mod platform;
mod recorder_actor;
mod state;
mod tray;

use state::AppStates;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{App, AppHandle, Emitter, Manager};
use tingyuxuan_core::pipeline::events::PipelineEvent;
use tokio::sync::{RwLock, broadcast};

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

type SessionPhaseLock = Arc<RwLock<state::SessionPhase>>;
const SHORTCUT_DEBOUNCE_MS: u64 = 250;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _log_guard = init_tracing();
    let sentry_client = init_sentry();
    build_tauri_app(&sentry_client)
        .run(tauri::generate_context!())
        .expect("error while running TingYuXuan");
}

fn build_tauri_app(sentry_client: &sentry::ClientInitGuard) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .device_event_filter(tauri::DeviceEventFilter::Never)
        .plugin(tauri_plugin_sentry::init(sentry_client))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(setup_app)
        .on_window_event(handle_main_close)
        .invoke_handler(app_invoke_handler())
}

#[rustfmt::skip]
fn app_invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::start_recording, commands::stop_recording, commands::cancel_recording, commands::get_config, commands::save_config,
        commands::test_multimodal_connection, commands::get_recent_history, commands::save_api_key, commands::get_api_key, commands::inject_text,
        commands::get_dictionary, commands::add_dictionary_word, commands::remove_dictionary_word, commands::search_history, commands::get_history_page,
        commands::delete_history, commands::delete_history_batch, commands::clear_history, commands::is_first_launch, commands::get_dashboard_stats,
        commands::check_platform_permissions, commands::open_permission_settings, commands::report_telemetry_event, commands::list_input_devices,
        commands::set_input_device
    ]
}

fn setup_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let states = build_states()?;
    let session_phase = Arc::new(RwLock::new(state::SessionPhase::Idle));

    spawn_event_bridge(
        &states.event_bus.0,
        app.handle().clone(),
        states.network.0.clone(),
        session_phase.clone(),
    );
    register_global_shortcuts(app)?;
    setup_network_monitor(app, &states);
    manage_app_states(app, states, session_phase);
    tray::create_tray(app.handle())?;

    tracing::info!("TingYuXuan started successfully");
    Ok(())
}

fn build_states() -> Result<AppStates, Box<dyn std::error::Error>> {
    let states = AppStates::new()?;
    hydrate_pipeline(&states);
    Ok(states)
}

fn hydrate_pipeline(states: &AppStates) {
    let config = states.config.0.blocking_read();
    let pipeline = commands::build_pipeline(&config, &states.event_bus.0);
    *states.pipeline.0.blocking_write() = pipeline;
}

fn setup_network_monitor(app: &mut App, states: &AppStates) {
    let config = states.config.0.blocking_read();
    let check_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| config.llm_base_url());
    drop(config);

    let monitor = tingyuxuan_core::pipeline::network::NetworkMonitor::new(check_url);
    let event_bus_clone = states.event_bus.0.clone();
    let monitor_token =
        tauri::async_runtime::block_on(async move { monitor.start(event_bus_clone) });
    app.manage(state::MonitorState(monitor_token));
}

fn manage_app_states(app: &mut App, states: AppStates, session_phase: SessionPhaseLock) {
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

    let app_version = env!("CARGO_PKG_VERSION");
    let telemetry_backend = tingyuxuan_core::telemetry::sls::create_backend(app_version);
    app.manage(state::TelemetryState(telemetry_backend));
    app.manage(state::SessionPhaseState(session_phase));
}

fn handle_main_close(window: &tauri::Window, event: &tauri::WindowEvent) {
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
}

fn spawn_event_bridge(
    event_tx: &broadcast::Sender<PipelineEvent>,
    handle: AppHandle,
    network_flag: Arc<AtomicBool>,
    session_phase: SessionPhaseLock,
) {
    let mut event_rx = event_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = receive_pipeline_event(&mut event_rx).await {
            process_pipeline_event(&handle, &network_flag, &session_phase, event).await;
        }
    });
}

async fn receive_pipeline_event(
    event_rx: &mut broadcast::Receiver<PipelineEvent>,
) -> Option<PipelineEvent> {
    match event_rx.recv().await {
        Ok(event) => Some(event),
        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!("Event bridge lagged, skipped {n} event(s)");
            None
        }
        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
            tracing::info!("Event bus closed, stopping event bridge");
            None
        }
    }
}

async fn process_pipeline_event(
    handle: &AppHandle,
    network_flag: &AtomicBool,
    session_phase: &SessionPhaseLock,
    event: PipelineEvent,
) {
    log_pipeline_event(&event);
    sync_window_visibility(handle, &event);
    sync_session_phase(session_phase, &event).await;
    sync_network_status(network_flag, &event);
    emit_pipeline_event(handle, &event);
}

fn log_pipeline_event(event: &PipelineEvent) {
    if !matches!(event, PipelineEvent::VolumeUpdate { .. }) {
        tracing::info!(?event, "Event bridge: received pipeline event");
    }
}

fn sync_window_visibility(handle: &AppHandle, event: &PipelineEvent) {
    let Some(window) = handle.get_webview_window("floating-bar") else {
        tracing::warn!("Event bridge: floating-bar window NOT FOUND");
        return;
    };

    if should_show_floating_bar(event) {
        position_floating_bar(&window);
        let _ = window.show();
    }
}

fn should_show_floating_bar(event: &PipelineEvent) -> bool {
    matches!(
        event,
        PipelineEvent::RecorderStarting { .. }
            | PipelineEvent::RecordingStarted { .. }
            | PipelineEvent::Error { .. }
    )
}

async fn sync_session_phase(session_phase: &SessionPhaseLock, event: &PipelineEvent) {
    use state::SessionPhase;

    match event {
        PipelineEvent::RecorderStarting { .. } => {
            *session_phase.write().await = SessionPhase::Starting {
                triggered_at: std::time::Instant::now(),
            };
        }
        PipelineEvent::RecordingStarted { .. } => {
            *session_phase.write().await = SessionPhase::Recording {
                started_at: std::time::Instant::now(),
            };
        }
        PipelineEvent::ThinkingStarted | PipelineEvent::ProcessingStarted => {
            *session_phase.write().await = SessionPhase::Thinking;
        }
        PipelineEvent::ProcessingComplete { .. }
        | PipelineEvent::Error { .. }
        | PipelineEvent::RecordingCancelled => {
            *session_phase.write().await = SessionPhase::Idle;
            tracing::info!("Session phase -> Idle");
        }
        _ => {}
    }
}

fn sync_network_status(network_flag: &AtomicBool, event: &PipelineEvent) {
    if let PipelineEvent::NetworkStatusChanged { online } = event {
        network_flag.store(*online, Ordering::Release);
    }
}

fn emit_pipeline_event(handle: &AppHandle, event: &PipelineEvent) {
    if let Err(e) = handle.emit("pipeline-event", event) {
        tracing::error!(?e, "Failed to emit pipeline-event to frontend");
    }
}

/// Register global keyboard shortcuts for recording control.
fn register_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        register_macos_shortcuts(app);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        register_windows_shortcuts(app);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        register_linux_shortcuts(app)
    }
}

#[cfg(target_os = "macos")]
fn register_macos_shortcuts(app: &tauri::App) {
    match platform::macos::register_platform_hotkeys(app) {
        Ok(fn_monitor) => {
            if let Some(monitor) = fn_monitor {
                app.manage(state::FnKeyMonitorState(Some(monitor)));
            }
        }
        Err(e) => tracing::warn!("Failed to register macOS platform hotkeys: {e}"),
    }
}

#[cfg(target_os = "windows")]
fn register_windows_shortcuts(app: &tauri::App) {
    match platform::windows::register_platform_hotkeys(app) {
        Ok(monitor) => {
            app.manage(state::RAltKeyMonitorState(Some(monitor)));
        }
        Err(e) => tracing::warn!("Failed to register Windows platform hotkeys: {e}"),
    }
}

/// Linux 标准快捷键注册（全部通过 tauri-plugin-global-shortcut）。
#[cfg(target_os = "linux")]
fn register_linux_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

    warn_if_wayland();
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
        register_linux_shortcut(app, &handle, shortcut, action);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn warn_if_wayland() {
    let display = platform::linux::detect_display_server();
    if display == platform::linux::DisplayServer::Wayland {
        tracing::warn!(
            "Running on Wayland - global shortcuts may not work. \
             Consider using the system tray or in-app buttons."
        );
    }
}

#[cfg(target_os = "linux")]
fn register_linux_shortcut(
    app: &tauri::App,
    handle: &AppHandle,
    shortcut: tauri_plugin_global_shortcut::Shortcut,
    action: &str,
) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let h = handle.clone();
    let action_name = action.to_string();
    if let Err(e) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            dispatch_shortcut_action(h.clone(), action_name.clone());
        })
    {
        tracing::warn!(
            "Failed to register shortcut for '{}': {}. Another app may have claimed it.",
            action,
            e
        );
    }
}

#[cfg(target_os = "linux")]
fn dispatch_shortcut_action(handle: AppHandle, action: String) {
    tauri::async_runtime::spawn(async move {
        handle_shortcut_action(&handle, &action).await;
    });
}

/// 将浮动条窗口定位到屏幕底部居中（任务栏上方约 4 逻辑像素）。
fn position_floating_bar(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let phys = monitor.size();
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let (win_w, win_h) = match window.outer_size() {
            Ok(size) => (size.width as f64 / scale, size.height as f64 / scale),
            Err(_) => (220.0_f64, 56.0_f64),
        };
        let taskbar_margin = 4.0_f64;

        let logical_w = phys.width as f64 / scale;
        let logical_h = phys.height as f64 / scale;
        let origin_x = origin.x as f64 / scale;
        let origin_y = origin.y as f64 / scale;

        let x = origin_x + (logical_w - win_w) / 2.0;
        let y = origin_y + logical_h - win_h - taskbar_margin;

        if let Err(e) =
            window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)))
        {
            tracing::warn!(?e, "Failed to position floating bar");
        }
    }
}

/// 快捷键动作处理 — 基于 SessionPhase 状态机。
pub(crate) async fn handle_shortcut_action(handle: &tauri::AppHandle, action: &str) {
    use crate::state::SessionPhaseState;

    let phase_state = handle.state::<SessionPhaseState>();
    if action == "cancel" {
        handle_cancel_shortcut(handle, &phase_state).await;
        return;
    }
    handle_mode_shortcut(handle, &phase_state, action).await;
}

async fn handle_cancel_shortcut(handle: &AppHandle, phase_state: &state::SessionPhaseState) {
    use crate::state::SessionPhase;

    let mut phase = phase_state.0.write().await;
    if matches!(*phase, SessionPhase::Idle) {
        return;
    }
    *phase = SessionPhase::Idle;
    drop(phase);
    tracing::info!("Shortcut: cancel");
    let _ = handle.emit("shortcut-action", "cancel");
}

async fn handle_mode_shortcut(
    handle: &AppHandle,
    phase_state: &state::SessionPhaseState,
    action: &str,
) {
    use crate::state::SessionPhase;

    if !matches!(action, "dictate" | "translate" | "ai_assistant") {
        return;
    }

    let mut phase = phase_state.0.write().await;
    match *phase {
        SessionPhase::Idle => {
            *phase = SessionPhase::Starting {
                triggered_at: std::time::Instant::now(),
            };
            drop(phase);
            tracing::info!(mode = action, "Shortcut: start recording");
            let _ = handle.emit("shortcut-action", action);
        }
        SessionPhase::Starting { triggered_at } => {
            tracing::info!(
                elapsed_ms = triggered_at.elapsed().as_millis(),
                "Shortcut: ignoring (recorder still starting)"
            );
        }
        SessionPhase::Recording { started_at } => {
            let elapsed = started_at.elapsed();
            if elapsed < std::time::Duration::from_millis(SHORTCUT_DEBOUNCE_MS) {
                tracing::info!(
                    elapsed_ms = elapsed.as_millis(),
                    "Shortcut: debounce (too quick, ignoring)"
                );
                return;
            }
            *phase = SessionPhase::Thinking;
            drop(phase);
            tracing::info!("Shortcut: stop recording");
            let _ = handle.emit("shortcut-action", "stop");
        }
        SessionPhase::Thinking => tracing::info!("Shortcut: ignoring (thinking in progress)"),
    }
}

/// 初始化 Sentry 错误/崩溃上报。
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
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "tingyuxuan=info".into());
    let stderr_layer = fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);
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
        .with(file_layer)
        .init();

    guard
}
