use std::sync::Arc;

use tauri::State;
use tracing::Instrument;

use tingyuxuan_core::config::{AppConfig, LLMProviderType};
use tingyuxuan_core::error::{PipelineError, StructuredError, UserAction};
use tingyuxuan_core::history::{AggregateStats, TranscriptRecord};
use tingyuxuan_core::llm::multimodal::MultimodalProvider;
use tingyuxuan_core::llm::provider::ProcessingMode;
use tingyuxuan_core::pipeline::events::PipelineEvent;
use tingyuxuan_core::pipeline::{Pipeline, ProcessingRequest};

use crate::platform::{ContextDetector, TextInjector};
use crate::state::{
    ActiveSession, ConfigState, DetectorState, EventBus, HistoryState, InjectorState,
    PipelineState, RecorderState, SessionState, TelemetryState,
};

// ---------------------------------------------------------------------------
// Input validation constants
// ---------------------------------------------------------------------------

const MAX_INJECT_TEXT_LEN: usize = 50_000;
const MAX_API_KEY_LEN: usize = 512;
const MAX_SEARCH_QUERY_LEN: usize = 500;
const MAX_DICT_WORD_LEN: usize = 100;
const VALID_KEY_SERVICES: &[&str] = &["llm"];
const RUNTIME_LLM_MODEL: &str = "qwen3-omni-flash";
const RUNTIME_DASHSCOPE_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

/// 音频最短时长阈值（毫秒）：低于此值视为无有效语音。
const MIN_AUDIO_DURATION_MS: u64 = 300;
/// MVP 录音时长上限（毫秒）：当前仅支持 <= 5 分钟。
const MAX_AUDIO_DURATION_MS: u64 = 5 * 60 * 1000;

/// Reject strings that contain null bytes — these can cause undefined behaviour
/// when passed to C libraries or shell commands.
fn check_no_null_bytes(s: &str, field_name: &str) -> Result<(), String> {
    if s.contains('\0') {
        Err(format!("{field_name} 不能包含 null 字节"))
    } else {
        Ok(())
    }
}

/// Reject strings exceeding a maximum byte length.
fn check_max_len(s: &str, max: usize, field_name: &str) -> Result<(), String> {
    if s.len() > max {
        Err(format!(
            "{field_name} 超过最大长度限制（最大 {max} 字节，实际 {} 字节）",
            s.len()
        ))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// API Key management (keyring-based secure storage)
// ---------------------------------------------------------------------------

/// Save an API key to the OS keychain.
///
/// `service` is "llm".
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn save_api_key(service: String, key: String) -> Result<(), String> {
    if !VALID_KEY_SERVICES.contains(&service.as_str()) {
        return Err(format!("无效的服务名称: {service}（允许: llm）"));
    }
    check_max_len(&key, MAX_API_KEY_LEN, "API Key")?;
    check_no_null_bytes(&key, "API Key")?;

    let entry =
        keyring::Entry::new("tingyuxuan", &service).map_err(|e| format!("Keyring error: {e}"))?;
    entry
        .set_password(&key)
        .map_err(|e| format!("Failed to save key: {e}"))?;
    tracing::info!("API key saved for service: {}", service);
    Ok(())
}

/// Retrieve an API key from the OS keychain.  Returns `None` if no key is stored.
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_api_key(service: String) -> Result<Option<String>, String> {
    if !VALID_KEY_SERVICES.contains(&service.as_str()) {
        return Err(format!("无效的服务名称: {service}（允许: llm）"));
    }
    let entry =
        keyring::Entry::new("tingyuxuan", &service).map_err(|e| format!("Keyring error: {e}"))?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::PlatformFailure(_)) => {
            tracing::warn!("Keyring platform not available — falling back to config.api_key_ref");
            Ok(None)
        }
        Err(e) => Err(format!("Failed to get key: {e}")),
    }
}

/// Try to get an API key: first from keyring, then fall back to config's api_key_ref.
fn resolve_api_key(service: &str, config_key_ref: &str) -> Option<String> {
    if let Ok(entry) = keyring::Entry::new("tingyuxuan", service)
        && let Ok(key) = entry.get_password()
        && !key.is_empty()
    {
        return Some(key);
    }
    if !config_key_ref.is_empty() && !config_key_ref.starts_with("@keyref:") {
        return Some(config_key_ref.to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// Pipeline factory
// ---------------------------------------------------------------------------

/// Build a Pipeline from the current config and stored API keys.
/// Returns `None` if API keys are missing or providers can't be created.
pub fn build_pipeline(
    config: &AppConfig,
    event_tx: &tokio::sync::broadcast::Sender<PipelineEvent>,
) -> Option<Arc<Pipeline>> {
    let llm_key = resolve_api_key("llm", &config.llm.api_key_ref).or_else(|| {
        tracing::warn!("Pipeline build skipped: no LLM API key");
        None
    })?;

    let (base_url, model) = resolve_runtime_llm(config);
    let provider = Box::new(
        MultimodalProvider::new(llm_key, base_url, model.clone())
            .map_err(|e| tracing::error!(error = %e, "Failed to create LLM provider"))
            .ok()?,
    );

    tracing::info!(
        configured_provider = ?config.llm.provider,
        configured_model = %config.llm.model,
        runtime_model = %model,
        "Pipeline built successfully"
    );
    Some(Arc::new(Pipeline::new(provider, event_tx.clone())))
}

fn resolve_runtime_llm(config: &AppConfig) -> (String, String) {
    if !matches!(config.llm.provider, LLMProviderType::DashScope) {
        tracing::warn!(
            configured_provider = ?config.llm.provider,
            "MVP runtime forces DashScope-compatible multimodal model"
        );
    }
    let base_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| RUNTIME_DASHSCOPE_BASE_URL.to_string());
    (base_url, RUNTIME_LLM_MODEL.to_string())
}

fn normalize_runtime_llm_config(config: &mut AppConfig) {
    config.llm.provider = LLMProviderType::DashScope;
    config.llm.model = RUNTIME_LLM_MODEL.to_string();
    if config.llm.base_url.is_none() {
        config.llm.base_url = Some(RUNTIME_DASHSCOPE_BASE_URL.to_string());
    }
}

// ---------------------------------------------------------------------------
// Recording commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_recording(
    mode: String,
    config_state: State<'_, ConfigState>,
    event_bus: State<'_, EventBus>,
    session_state: State<'_, SessionState>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
    pipeline_state: State<'_, PipelineState>,
    detector_state: State<'_, DetectorState>,
) -> Result<String, String> {
    let pipeline = require_active_pipeline(&pipeline_state, &event_bus).await?;
    let start_ctx = build_start_context(mode, &config_state, &detector_state, pipeline).await;

    emit_recorder_starting(&event_bus, &start_ctx.effective_mode);
    start_recorder(&recorder_state, &event_bus).await?;
    create_recording_history(&history_state, &start_ctx).await;
    *session_state.0.lock().await = Some(start_ctx.session);
    emit_recording_started(&event_bus, &start_ctx.session_id, &start_ctx.effective_mode);

    Ok(start_ctx.session_id)
}

struct StartContext {
    session_id: String,
    effective_mode: String,
    session: ActiveSession,
    context_json: Option<String>,
}

async fn require_active_pipeline(
    pipeline_state: &State<'_, PipelineState>,
    event_bus: &State<'_, EventBus>,
) -> Result<Arc<Pipeline>, String> {
    pipeline_state.0.read().await.clone().ok_or_else(|| {
        let se = StructuredError {
            error_code: "not_configured".into(),
            message: "请先在设置中配置 LLM 的 API Key".into(),
            user_action: UserAction::CheckApiKey,
        };
        let _ = event_bus.0.send(PipelineEvent::Error {
            message: se.message.clone(),
            user_action: se.user_action.clone(),
        });
        serde_json::to_string(&se).unwrap()
    })
}

async fn build_start_context(
    mode: String,
    config_state: &State<'_, ConfigState>,
    detector_state: &State<'_, DetectorState>,
    pipeline: Arc<Pipeline>,
) -> StartContext {
    let (session_id, session_span) = init_session_tracking(&mode);
    let context = detector_state.0.collect_context();
    let processing_mode = resolve_processing_mode(&mode, &context);
    let (target_language, user_dictionary) =
        load_language_and_dictionary(config_state, &processing_mode).await;
    let effective_mode = processing_mode.to_string();
    session_span.record("mode", effective_mode.as_str());

    StartContext {
        session_id: session_id.clone(),
        effective_mode: effective_mode.clone(),
        context_json: serde_json::to_string(&context).ok(),
        session: ActiveSession {
            session_id,
            config: ProcessingRequest {
                mode: processing_mode,
                context,
                target_language,
                user_dictionary,
            },
            pipeline,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            started_at: std::time::Instant::now(),
            session_span: session_span.clone(),
        },
    }
}

fn init_session_tracking(mode: &str) -> (String, tracing::Span) {
    let session_id = uuid::Uuid::new_v4().to_string();
    add_start_breadcrumb(&session_id, mode);
    let session_span =
        tracing::info_span!("session", session_id = %session_id, mode = tracing::field::Empty);
    (session_id, session_span)
}

fn add_start_breadcrumb(session_id: &str, mode: &str) {
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("recording".into()),
        message: Some(format!("start: mode={mode}")),
        level: sentry::Level::Info,
        data: {
            let mut map = sentry::protocol::Map::new();
            map.insert("session_id".into(), session_id.to_string().into());
            map
        },
        ..Default::default()
    });
}

fn resolve_processing_mode(
    mode: &str,
    context: &tingyuxuan_core::context::InputContext,
) -> ProcessingMode {
    let parsed = mode
        .parse::<ProcessingMode>()
        .unwrap_or(ProcessingMode::Dictate);
    if context.selected_text.is_some() && matches!(parsed, ProcessingMode::Dictate) {
        tracing::info!("Auto-switched to Edit mode (selected text detected)");
        return ProcessingMode::Edit;
    }
    parsed
}

async fn load_language_and_dictionary(
    config_state: &State<'_, ConfigState>,
    processing_mode: &ProcessingMode,
) -> (Option<String>, Vec<String>) {
    let config = config_state.0.read().await;
    let target_language = matches!(processing_mode, ProcessingMode::Translate)
        .then(|| config.language.translation_target.clone());
    let dictionary = config.user_dictionary.clone();
    (target_language, dictionary)
}

fn emit_recorder_starting(event_bus: &State<'_, EventBus>, mode: &str) {
    let _ = event_bus.0.send(PipelineEvent::RecorderStarting {
        mode: mode.to_string(),
    });
}

async fn start_recorder(
    recorder_state: &State<'_, RecorderState>,
    event_bus: &State<'_, EventBus>,
) -> Result<(), String> {
    tracing::info!("start_recording: calling recorder.start()");
    if let Err(audio_err) = recorder_state.0.start().await {
        tracing::error!(%audio_err, "start_recording: recorder.start() FAILED");
        let se = StructuredError::from(&PipelineError::Audio(audio_err));
        let _ = event_bus.0.send(PipelineEvent::Error {
            message: se.message.clone(),
            user_action: se.user_action.clone(),
        });
        return Err(serde_json::to_string(&se).unwrap());
    }
    tracing::info!("start_recording: recorder started OK");
    Ok(())
}

async fn create_recording_history(
    history_state: &State<'_, HistoryState>,
    start_ctx: &StartContext,
) {
    let history = history_state.0.lock().await;
    let record = TranscriptRecord {
        id: start_ctx.session_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        mode: start_ctx.effective_mode.clone(),
        raw_text: None,
        processed_text: None,
        status: "recording".to_string(),
        context_json: start_ctx.context_json.clone(),
        duration_ms: None,
        language: None,
        error_message: None,
    };
    let _ = history.save_transcript(&record);
}

fn emit_recording_started(event_bus: &State<'_, EventBus>, session_id: &str, mode: &str) {
    tracing::info!("start_recording: emitting RecordingStarted event");
    let send_result = event_bus.0.send(PipelineEvent::RecordingStarted {
        session_id: session_id.to_string(),
        mode: mode.to_string(),
    });
    tracing::info!(?send_result, "start_recording: RecordingStarted sent");
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn stop_recording(
    session_state: State<'_, SessionState>,
    _pipeline_state: State<'_, PipelineState>,
    event_bus: State<'_, EventBus>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
    _config_state: State<'_, ConfigState>,
    injector_state: State<'_, InjectorState>,
) -> Result<String, String> {
    let buffer = recorder_state.0.stop().await.map_err(|e| e.to_string())?;
    let session = take_active_session(&session_state).await?;
    let stop_ctx = StopContext::from_session(session);

    add_stop_breadcrumb(&stop_ctx.session_id, stop_ctx.duration_ms);
    let _ = event_bus.0.send(PipelineEvent::RecordingStopped {
        duration_ms: stop_ctx.duration_ms,
    });
    if let Some(status) =
        validate_buffer(&buffer, &event_bus, &history_state, &stop_ctx.session_id).await
    {
        return Ok(status);
    }

    spawn_processing_task(
        buffer,
        stop_ctx,
        &injector_state,
        &event_bus,
        &history_state,
    );
    Ok("processing".to_string())
}

struct StopContext {
    session_id: String,
    duration_ms: u64,
    session_span: tracing::Span,
    pipeline: Arc<Pipeline>,
    request: ProcessingRequest,
    cancel_token: tokio_util::sync::CancellationToken,
    is_ai_assistant: bool,
}

impl StopContext {
    fn from_session(session: ActiveSession) -> Self {
        let duration_ms = session.started_at.elapsed().as_millis() as u64;
        let is_ai_assistant = matches!(session.config.mode, ProcessingMode::AiAssistant);
        Self {
            session_id: session.session_id,
            duration_ms,
            session_span: session.session_span,
            pipeline: session.pipeline,
            request: session.config,
            cancel_token: session.cancel_token,
            is_ai_assistant,
        }
    }
}

async fn take_active_session(
    session_state: &State<'_, SessionState>,
) -> Result<ActiveSession, String> {
    session_state
        .0
        .lock()
        .await
        .take()
        .ok_or_else(|| "No active recording session".to_string())
}

fn add_stop_breadcrumb(session_id: &str, duration_ms: u64) {
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("recording".into()),
        message: Some(format!("stop: duration={duration_ms}ms")),
        level: sentry::Level::Info,
        data: {
            let mut map = sentry::protocol::Map::new();
            map.insert("session_id".into(), session_id.to_string().into());
            map.insert("duration_ms".into(), duration_ms.into());
            map
        },
        ..Default::default()
    });
}

async fn validate_buffer(
    buffer: &tingyuxuan_core::audio::encoder::AudioBuffer,
    event_bus: &State<'_, EventBus>,
    history_state: &State<'_, HistoryState>,
    session_id: &str,
) -> Option<String> {
    if let Some(status) = reject_too_long(buffer, event_bus, history_state, session_id).await {
        return Some(status);
    }
    if let Some(status) = reject_too_short(buffer, event_bus, history_state, session_id).await {
        return Some(status);
    }
    reject_silence(buffer, event_bus, history_state, session_id).await
}

async fn reject_too_long(
    buffer: &tingyuxuan_core::audio::encoder::AudioBuffer,
    event_bus: &State<'_, EventBus>,
    history_state: &State<'_, HistoryState>,
    session_id: &str,
) -> Option<String> {
    if buffer.duration_ms() <= MAX_AUDIO_DURATION_MS {
        return None;
    }
    tracing::warn!(
        duration_ms = buffer.duration_ms(),
        "Audio exceeds MVP max duration"
    );
    let _ = event_bus.0.send(PipelineEvent::Error {
        message: "当前版本仅支持单次录音小于等于 5 分钟".to_string(),
        user_action: tingyuxuan_core::error::UserAction::Retry,
    });
    update_history_status(history_state, session_id, "failed").await;
    Some("too_long".to_string())
}

async fn reject_too_short(
    buffer: &tingyuxuan_core::audio::encoder::AudioBuffer,
    event_bus: &State<'_, EventBus>,
    history_state: &State<'_, HistoryState>,
    session_id: &str,
) -> Option<String> {
    if buffer.duration_ms() >= MIN_AUDIO_DURATION_MS {
        return None;
    }
    tracing::warn!(duration_ms = buffer.duration_ms(), "Audio too short");
    let _ = event_bus.0.send(PipelineEvent::Error {
        message: "录音时间过短，请重试".to_string(),
        user_action: tingyuxuan_core::error::UserAction::Retry,
    });
    update_history_status(history_state, session_id, "failed").await;
    Some("empty".to_string())
}

async fn reject_silence(
    buffer: &tingyuxuan_core::audio::encoder::AudioBuffer,
    event_bus: &State<'_, EventBus>,
    history_state: &State<'_, HistoryState>,
    session_id: &str,
) -> Option<String> {
    let rms = buffer.rms_level();
    tracing::info!(
        rms,
        duration_ms = buffer.duration_ms(),
        "Audio buffer stats"
    );
    if rms >= 200.0 {
        return None;
    }
    tracing::warn!(rms, "Audio appears to be silence, skipping LLM");
    let _ = event_bus.0.send(PipelineEvent::RecordingCancelled);
    update_history_status(history_state, session_id, "cancelled").await;
    Some("silence".to_string())
}

async fn update_history_status(
    history_state: &State<'_, HistoryState>,
    session_id: &str,
    status: &str,
) {
    let history = history_state.0.lock().await;
    let _ = history.update_status(session_id, status);
}

fn spawn_processing_task(
    buffer: tingyuxuan_core::audio::encoder::AudioBuffer,
    stop_ctx: StopContext,
    injector_state: &State<'_, InjectorState>,
    event_bus: &State<'_, EventBus>,
    history_state: &State<'_, HistoryState>,
) {
    let injector = injector_state.0.clone();
    let history = history_state.0.clone();
    let event_tx = event_bus.0.clone();

    tokio::spawn(
        async move {
            let result = stop_ctx
                .pipeline
                .process_audio(buffer, &stop_ctx.request, stop_ctx.cancel_token)
                .await;
            handle_pipeline_result(
                result,
                stop_ctx.is_ai_assistant,
                &stop_ctx.session_id,
                &injector,
                &event_tx,
                &history,
            )
            .await;
        }
        .instrument(stop_ctx.session_span),
    );
}

async fn handle_pipeline_result(
    result: Result<String, PipelineError>,
    is_ai_assistant: bool,
    session_id: &str,
    injector: &Arc<crate::platform::PlatformInjector>,
    event_tx: &tokio::sync::broadcast::Sender<PipelineEvent>,
    history: &Arc<tokio::sync::Mutex<tingyuxuan_core::history::HistoryManager>>,
) {
    match result {
        Ok(processed_text) => {
            maybe_inject_processed_text(is_ai_assistant, injector, &processed_text).await;
            let h = history.lock().await;
            let _ = h.update_processed(session_id, "", &processed_text);
        }
        Err(PipelineError::Cancelled) => {
            tracing::info!("Session was cancelled");
            let h = history.lock().await;
            let _ = h.update_status(session_id, "cancelled");
        }
        Err(error) => {
            tracing::error!("Pipeline processing failed: {error}");
            let se = StructuredError::from(&error);
            let _ = event_tx.send(PipelineEvent::Error {
                message: se.message,
                user_action: se.user_action,
            });
            let h = history.lock().await;
            let _ = h.update_status(session_id, "failed");
        }
    }
}

async fn maybe_inject_processed_text(
    is_ai_assistant: bool,
    injector: &Arc<crate::platform::PlatformInjector>,
    processed_text: &str,
) {
    if is_ai_assistant {
        tracing::info!("AI assistant result ready (no auto-inject)");
        return;
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    if let Err(e) = injector.inject_text(processed_text) {
        tracing::error!("Text injection failed: {e}");
    }
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn cancel_recording(
    session_state: State<'_, SessionState>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
) -> Result<(), String> {
    let _ = recorder_state.0.cancel().await;
    if let Some(session) = session_state.0.lock().await.take() {
        handle_session_cancelled(session, &history_state).await;
    }
    Ok(())
}

async fn handle_session_cancelled(session: ActiveSession, history_state: &State<'_, HistoryState>) {
    session.cancel_token.cancel();
    add_cancel_breadcrumb(&session.session_id);
    update_history_status(history_state, &session.session_id, "cancelled").await;
    tracing::info!("Recording cancelled: session={}", session.session_id);
}

fn add_cancel_breadcrumb(session_id: &str) {
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("recording".into()),
        message: Some("cancel".into()),
        level: sentry::Level::Info,
        data: {
            let mut map = sentry::protocol::Map::new();
            map.insert("session_id".into(), session_id.to_string().into());
            map
        },
        ..Default::default()
    });
}

// ---------------------------------------------------------------------------
// Text injection
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn inject_text(
    text: String,
    injector_state: State<'_, InjectorState>,
) -> Result<(), String> {
    check_max_len(&text, MAX_INJECT_TEXT_LEN, "注入文本")?;
    check_no_null_bytes(&text, "注入文本")?;
    let char_count = text.chars().count();
    match injector_state.0.inject_text(&text) {
        Ok(()) => {
            tracing::info!(chars = char_count, "Text injected");
            Ok(())
        }
        Err(e) => {
            tracing::error!(%e, chars = char_count, "Injection failed");
            Err(format!("Text injection failed: {e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Connection testing
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn test_multimodal_connection(
    config_state: State<'_, ConfigState>,
) -> Result<bool, String> {
    let config = config_state.0.read().await;
    let api_key = resolve_api_key("llm", &config.llm.api_key_ref)
        .ok_or_else(|| "No LLM API key configured".to_string())?;

    let (base_url, model) = resolve_runtime_llm(&config);
    let provider = MultimodalProvider::new(api_key, base_url, model).map_err(|e| e.to_string())?;

    provider
        .test_multimodal_audio_connection()
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn is_first_launch(pipeline_state: State<'_, PipelineState>) -> Result<bool, String> {
    Ok(pipeline_state.0.read().await.is_none())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_config(config_state: State<'_, ConfigState>) -> Result<AppConfig, String> {
    let config = config_state.0.read().await;
    Ok(config.clone())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn save_config(
    mut config: AppConfig,
    config_state: State<'_, ConfigState>,
    pipeline_state: State<'_, PipelineState>,
    event_bus: State<'_, EventBus>,
) -> Result<(), String> {
    normalize_runtime_llm_config(&mut config);

    // Persist to disk.
    config.save().map_err(|e| e.to_string())?;

    // Update in-memory config.
    *config_state.0.write().await = config.clone();

    // Rebuild pipeline with new config (picks up new provider/model/base_url).
    let new_pipeline = build_pipeline(&config, &event_bus.0);
    *pipeline_state.0.write().await = new_pipeline;

    tracing::info!("Configuration saved and pipeline rebuilt");
    Ok(())
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_recent_history(
    limit: u32,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<TranscriptRecord>, String> {
    let history = history_state.0.lock().await;
    history.get_recent(limit).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn search_history(
    query: String,
    limit: u32,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<TranscriptRecord>, String> {
    check_max_len(&query, MAX_SEARCH_QUERY_LEN, "搜索关键词")?;
    check_no_null_bytes(&query, "搜索关键词")?;
    let history = history_state.0.lock().await;
    history.search(&query, limit).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_history_page(
    limit: u32,
    offset: u32,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<TranscriptRecord>, String> {
    let history = history_state.0.lock().await;
    history.get_page(limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn delete_history(
    id: String,
    history_state: State<'_, HistoryState>,
) -> Result<(), String> {
    let history = history_state.0.lock().await;
    history.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn delete_history_batch(
    ids: Vec<String>,
    history_state: State<'_, HistoryState>,
) -> Result<u64, String> {
    let history = history_state.0.lock().await;
    history.delete_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn clear_history(history_state: State<'_, HistoryState>) -> Result<u64, String> {
    let history = history_state.0.lock().await;
    history.clear_all().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Dictionary
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_dictionary(config_state: State<'_, ConfigState>) -> Result<Vec<String>, String> {
    let config = config_state.0.read().await;
    Ok(config.user_dictionary.clone())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn add_dictionary_word(
    word: String,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let trimmed = word.trim().to_string();
    if trimmed.is_empty() {
        return Err("词汇不能为空".to_string());
    }
    check_max_len(&trimmed, MAX_DICT_WORD_LEN, "词汇")?;
    check_no_null_bytes(&trimmed, "词汇")?;

    let mut config = config_state.0.write().await;
    if config.user_dictionary.contains(&trimmed) {
        return Ok(());
    }
    config.user_dictionary.push(trimmed);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn remove_dictionary_word(
    word: String,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let mut config = config_state.0.write().await;
    config.user_dictionary.retain(|w| w != &word);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Dashboard Stats
// ---------------------------------------------------------------------------

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn get_dashboard_stats(
    history_state: State<'_, HistoryState>,
    config_state: State<'_, ConfigState>,
) -> Result<AggregateStats, String> {
    let dictionary = config_state.0.read().await.user_dictionary.clone();
    let history = history_state.0.lock().await;
    let mut stats = history.get_stats().map_err(|e| e.to_string())?;
    stats.dictionary_utilization = history
        .get_dictionary_utilization(&dictionary)
        .map_err(|e| e.to_string())?;
    Ok(stats)
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

/// 接收前端埋点事件，通过 Rust 后端统一上报到 SLS。
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn report_telemetry_event(
    event: String,
    telemetry: State<'_, TelemetryState>,
) -> Result<(), String> {
    if let Ok(evt) = serde_json::from_str::<tingyuxuan_core::telemetry::TelemetryEvent>(&event) {
        telemetry.0.track(evt);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Platform Permissions
// ---------------------------------------------------------------------------

/// 检查全平台权限状态，返回 JSON 格式的 PermissionReport。
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn check_platform_permissions() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let report = crate::platform::macos::check_permissions();
    #[cfg(target_os = "windows")]
    let report = crate::platform::windows::check_permissions();
    #[cfg(target_os = "linux")]
    let report = crate::platform::linux::check_permissions();

    tracing::info!(?report, "Permission check");
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

/// 打开系统权限设置页面。
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn open_permission_settings(target: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    crate::platform::macos::open_permission_settings_for(target.as_deref());
    #[cfg(target_os = "windows")]
    crate::platform::windows::open_permission_settings_for(target.as_deref());
    #[cfg(target_os = "linux")]
    crate::platform::linux::open_permission_settings_for(target.as_deref());

    let _ = target;
    Ok(())
}

// ---------------------------------------------------------------------------
// Audio Devices
// ---------------------------------------------------------------------------

/// 枚举所有可用音频输入设备。
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn list_input_devices()
-> Result<Vec<tingyuxuan_core::audio::devices::AudioDeviceInfo>, String> {
    tingyuxuan_core::audio::devices::enumerate_input_devices().map_err(|e| e.to_string())
}

/// 设置输入设备并持久化。`device_id = None` 表示恢复为系统默认。
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn set_input_device(
    device_id: Option<String>,
    config_state: State<'_, ConfigState>,
    recorder_state: State<'_, RecorderState>,
) -> Result<(), String> {
    // 持久化到配置文件。
    {
        let mut config = config_state.0.write().await;
        config.audio.input_device_id = device_id.clone();
        config.save().map_err(|e| e.to_string())?;
    }
    // 通知 recorder actor 切换设备。
    recorder_state.0.set_device(device_id).await;
    tracing::info!("Input device updated");
    Ok(())
}
