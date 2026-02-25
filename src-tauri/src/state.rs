use std::sync::Arc;

use tokio::sync::{Mutex, RwLock, broadcast};
use tokio_util::sync::CancellationToken;

use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::history::HistoryManager;
use tingyuxuan_core::llm::provider::ProcessingMode;
use tingyuxuan_core::pipeline::Pipeline;
use tingyuxuan_core::pipeline::events::PipelineEvent;

use tingyuxuan_core::pipeline::queue::OfflineQueue;

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

/// Offline recording queue — persistent SQLite-backed queue for recordings
/// captured while the network is down.  Survives application restarts.
pub struct QueueState(pub Arc<Mutex<OfflineQueue>>);

/// Tracks current network connectivity status (true = online).
pub struct NetworkState(pub Arc<std::sync::atomic::AtomicBool>);

/// Platform text injector — created once, used for all inject operations.
/// Wrapped in Arc so it can be cloned into spawned async tasks.
pub struct InjectorState(pub Arc<PlatformInjector>);

/// Platform context detector — created once, used for all context queries.
pub struct DetectorState(pub PlatformDetector);

/// Tracks the in-progress recording/processing session.
pub struct ActiveSession {
    pub session_id: String,
    pub mode: ProcessingMode,
    pub selected_text: Option<String>,
    pub target_language: Option<String>,
    pub app_context: Option<String>,
    /// Token that can be cancelled to abort in-progress STT/LLM requests.
    pub cancel_token: CancellationToken,
}

/// Helper to create all managed states used by the application.
pub struct AppStates {
    pub config: ConfigState,
    pub history: HistoryState,
    pub pipeline: PipelineState,
    pub event_bus: EventBus,
    pub session: SessionState,
    pub recorder: RecorderState,
    pub queue: QueueState,
    pub network: NetworkState,
    pub injector: InjectorState,
    pub detector: DetectorState,
}

impl AppStates {
    /// Build all application states from the persisted configuration.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = AppConfig::load_with_migration().unwrap_or_default();
        let history = HistoryManager::new()?;
        let (event_tx, _) = broadcast::channel::<PipelineEvent>(64);

        // Spawn the recorder actor on a dedicated OS thread.
        // Volume updates are pushed to the event bus automatically.
        let recorder_handle = RecorderHandle::spawn(event_tx.clone());

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
            queue: QueueState(Arc::new(Mutex::new(OfflineQueue::new()))),
            network: NetworkState(Arc::new(std::sync::atomic::AtomicBool::new(true))),
            injector: InjectorState(Arc::new(injector)),
            detector: DetectorState(detector),
        })
    }
}
