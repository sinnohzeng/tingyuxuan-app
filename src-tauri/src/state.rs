use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{Mutex, RwLock, broadcast};
use tokio_util::sync::CancellationToken;

use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::history::HistoryManager;
use tingyuxuan_core::pipeline::Pipeline;
use tingyuxuan_core::pipeline::ProcessingRequest;
use tingyuxuan_core::pipeline::events::PipelineEvent;

use crate::platform::{PlatformDetector, PlatformInjector};
use crate::recorder_actor::RecorderHandle;

/// Configuration state — reads are frequent, writes are rare.
pub struct ConfigState(pub Arc<RwLock<AppConfig>>);

/// History (SQLite) — behind its own mutex to avoid contending with config reads.
pub struct HistoryState(pub Arc<Mutex<HistoryManager>>);

/// Pipeline — behind RwLock so it can be rebuilt when config changes.
/// `None` when API keys are not yet configured.
pub struct PipelineState(pub Arc<RwLock<Option<Arc<Pipeline>>>>);

/// Event bus — broadcast sender for pipeline events.
pub struct EventBus(pub broadcast::Sender<PipelineEvent>);

/// Recorder handle — communicates with the dedicated recorder OS thread.
pub struct RecorderState(pub RecorderHandle);

/// Currently active recording session.
pub struct SessionState(pub Arc<Mutex<Option<ActiveSession>>>);

/// Tracks current network connectivity status (true = online).
pub struct NetworkState(pub Arc<std::sync::atomic::AtomicBool>);

/// Platform text injector — created once, used for all inject operations.
/// Wrapped in Arc so it can be cloned into spawned async tasks.
pub struct InjectorState(pub Arc<PlatformInjector>);

/// Platform context detector — created once, used for all context queries.
pub struct DetectorState(pub PlatformDetector);

/// 托盘图标 handle — 用于运行时重建菜单（惰性设备枚举）。
pub struct TrayState(pub Arc<Mutex<Option<tauri::tray::TrayIcon>>>);

/// Holds the network monitor cancellation token to keep it alive for the app's lifetime.
/// The field is never read directly — the struct exists solely to keep the token alive.
#[allow(dead_code)]
pub struct MonitorState(pub tokio_util::sync::CancellationToken);

/// Telemetry 后端 — 应用生命周期内存活，用于上报事件到 SLS。
pub struct TelemetryState(pub Box<dyn tingyuxuan_core::telemetry::TelemetryBackend>);

/// 会话阶段追踪 — 后端唯一真值，防止快捷键在不当时刻触发。
///
/// 状态机: Idle → Recording → Processing → Idle
/// - Recording 前 800ms 内忽略 RAlt（防误触）
/// - Processing 期间忽略所有 RAlt
pub struct SessionPhaseState(pub Arc<RwLock<SessionPhase>>);

#[derive(Debug, Clone, Copy)]
pub enum SessionPhase {
    Idle,
    Recording { started_at: Instant },
    Processing,
}

/// macOS Fn 键监听器状态 — 持有 FnKeyMonitor 使其在应用生命周期内存活。
/// 在非 macOS 平台上不使用。
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub struct FnKeyMonitorState(pub Option<crate::platform::macos::FnKeyMonitor>);

/// Windows RAlt 键监听器状态 — 持有 RAltKeyMonitor 使其在应用生命周期内存活。
/// 在非 Windows 平台上不使用。
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub struct RAltKeyMonitorState(pub Option<crate::platform::windows::RAltKeyMonitor>);

/// Tracks the in-progress recording/processing session.
pub struct ActiveSession {
    pub session_id: String,
    /// 录音参数（模式、上下文、目标语言、词典）。
    pub config: ProcessingRequest,
    /// 录音开始时锁定的 Pipeline 引用 — 防止 save_config 在录音期间重建 pipeline 导致 TOCTOU。
    pub pipeline: Arc<Pipeline>,
    /// 取消令牌 — 用于取消录音或处理中的 LLM 调用。
    pub cancel_token: CancellationToken,
    /// 录音开始时间（用于计算 duration_ms）。
    pub started_at: std::time::Instant,
    /// 贯穿 session 生命周期的 tracing span。
    pub session_span: tracing::Span,
}

/// Helper to create all managed states used by the application.
pub struct AppStates {
    pub config: ConfigState,
    pub history: HistoryState,
    pub pipeline: PipelineState,
    pub event_bus: EventBus,
    pub session: SessionState,
    pub recorder: RecorderState,
    pub network: NetworkState,
    pub injector: InjectorState,
    pub detector: DetectorState,
    pub tray: TrayState,
}

impl AppStates {
    /// Build all application states from the persisted configuration.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = AppConfig::load_with_migration().unwrap_or_default();
        let history = HistoryManager::new()?;
        let (event_tx, _) = broadcast::channel::<PipelineEvent>(64);

        // 读取用户选择的麦克风设备 ID（None = 系统默认）。
        let device_id = config.audio.input_device_id.clone();

        // Spawn the recorder actor on a dedicated OS thread.
        // Volume updates are pushed to the event bus automatically.
        let recorder_handle = RecorderHandle::spawn(event_tx.clone(), device_id);

        // Create platform-specific injector and detector once at startup.
        let injector = PlatformInjector::new();
        let detector = PlatformDetector::new();

        Ok(Self {
            config: ConfigState(Arc::new(RwLock::new(config))),
            history: HistoryState(Arc::new(Mutex::new(history))),
            pipeline: PipelineState(Arc::new(RwLock::new(None))),
            event_bus: EventBus(event_tx),
            session: SessionState(Arc::new(Mutex::new(None))),
            recorder: RecorderState(recorder_handle),
            network: NetworkState(Arc::new(std::sync::atomic::AtomicBool::new(true))),
            injector: InjectorState(Arc::new(injector)),
            detector: DetectorState(detector),
            tray: TrayState(Arc::new(Mutex::new(None))),
        })
    }
}
