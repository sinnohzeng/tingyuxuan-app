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
    // 获取 pipeline 引用并存入 session，防止 save_config 在录音期间重建 pipeline（TOCTOU）。
    let pipeline = pipeline_state.0.read().await.clone().ok_or_else(|| {
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
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();

    // Sentry breadcrumb: 录音开始
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("recording".into()),
        message: Some(format!("start: mode={mode}")),
        level: sentry::Level::Info,
        data: {
            let mut m = sentry::protocol::Map::new();
            m.insert("session_id".into(), session_id.clone().into());
            m
        },
        ..Default::default()
    });

    let session_span = tracing::info_span!("session",
        session_id = %session_id,
        mode = tracing::field::Empty,
    );

    let mut processing_mode = mode
        .parse::<ProcessingMode>()
        .unwrap_or(ProcessingMode::Dictate);

    // 采集当前输入上下文（整体 ~200ms 超时，各项独立容错）
    let context = detector_state.0.collect_context();

    // Auto-switch to Edit mode when user has selected text and pressed Dictate.
    if context.selected_text.is_some() && matches!(processing_mode, ProcessingMode::Dictate) {
        processing_mode = ProcessingMode::Edit;
        tracing::info!("Auto-switched to Edit mode (selected text detected)");
    }

    // Read config for translate target language.
    let config = config_state.0.read().await;
    let target_language = if matches!(processing_mode, ProcessingMode::Translate) {
        Some(config.language.translation_target.clone())
    } else {
        None
    };
    let user_dictionary = config.user_dictionary.clone();
    drop(config);

    // Determine the effective mode string for events/history.
    let effective_mode = processing_mode.to_string();

    // 1. 进入 recorder 启动过渡状态（微加载），随后正式开始录音。
    let _ = event_bus.0.send(PipelineEvent::RecorderStarting {
        mode: effective_mode.clone(),
    });

    // 2. 启动录音（PCM 采样累积在 recorder 内部的 buffer 中）。
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

    // Serialize context for history storage.
    let context_json = serde_json::to_string(&context).ok();

    // 记录 effective mode
    session_span.record("mode", effective_mode.as_str());

    // Store active session（pipeline 在此锁定，stop_recording 直接使用）。
    let session = ActiveSession {
        session_id: session_id.clone(),
        config: ProcessingRequest {
            mode: processing_mode,
            context: context.clone(),
            target_language,
            user_dictionary,
        },
        pipeline,
        cancel_token: tokio_util::sync::CancellationToken::new(),
        started_at: std::time::Instant::now(),
        session_span: session_span.clone(),
    };
    *session_state.0.lock().await = Some(session);

    // Emit recording started event.
    tracing::info!("start_recording: emitting RecordingStarted event");
    let send_result = event_bus.0.send(PipelineEvent::RecordingStarted {
        session_id: session_id.clone(),
        mode: effective_mode.clone(),
    });
    tracing::info!(?send_result, "start_recording: RecordingStarted sent");

    // Create a history record with status "recording".
    {
        let history = history_state.0.lock().await;
        let record = TranscriptRecord {
            id: session_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: effective_mode.clone(),
            raw_text: None,
            processed_text: None,
            status: "recording".to_string(),
            context_json,
            duration_ms: None,
            language: None,
            error_message: None,
        };
        let _ = history.save_transcript(&record);
    }

    Ok(session_id)
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
    // 1. 停止录音 → 获取累积的 AudioBuffer。
    let buffer = recorder_state.0.stop().await.map_err(|e| e.to_string())?;

    // 2. Take the active session（包含录音开始时锁定的 pipeline 引用）。
    let session = session_state
        .0
        .lock()
        .await
        .take()
        .ok_or_else(|| "No active recording session".to_string())?;

    let session_id = session.session_id.clone();
    let duration_ms = session.started_at.elapsed().as_millis() as u64;
    let session_span = session.session_span.clone();

    // Sentry breadcrumb: 录音停止
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("recording".into()),
        message: Some(format!("stop: duration={duration_ms}ms")),
        level: sentry::Level::Info,
        data: {
            let mut m = sentry::protocol::Map::new();
            m.insert("session_id".into(), session_id.clone().into());
            m.insert("duration_ms".into(), duration_ms.into());
            m
        },
        ..Default::default()
    });

    // Emit recording stopped event.
    let _ = event_bus
        .0
        .send(PipelineEvent::RecordingStopped { duration_ms });

    if buffer.duration_ms() > MAX_AUDIO_DURATION_MS {
        tracing::warn!(
            duration_ms = buffer.duration_ms(),
            "Audio exceeds MVP max duration"
        );
        let _ = event_bus.0.send(PipelineEvent::Error {
            message: "当前版本仅支持单次录音小于等于 5 分钟".to_string(),
            user_action: tingyuxuan_core::error::UserAction::Retry,
        });
        let h = history_state.0.lock().await;
        let _ = h.update_status(&session_id, "failed");
        return Ok("too_long".to_string());
    }

    // 3. 检查音频是否过短或为静音。
    if buffer.duration_ms() < MIN_AUDIO_DURATION_MS {
        tracing::warn!(duration_ms = buffer.duration_ms(), "Audio too short");
        let _ = event_bus.0.send(PipelineEvent::Error {
            message: "录音时间过短，请重试".to_string(),
            user_action: tingyuxuan_core::error::UserAction::Retry,
        });
        let h = history_state.0.lock().await;
        let _ = h.update_status(&session_id, "failed");
        return Ok("empty".to_string());
    }

    let rms = buffer.rms_level();
    tracing::info!(
        rms,
        duration_ms = buffer.duration_ms(),
        "Audio buffer stats"
    );
    if rms < 200.0 {
        tracing::warn!(rms, "Audio appears to be silence, skipping LLM");
        let _ = event_bus.0.send(PipelineEvent::RecordingCancelled);
        let h = history_state.0.lock().await;
        let _ = h.update_status(&session_id, "cancelled");
        return Ok("silence".to_string());
    }

    // 4. 使用 session 中锁定的 pipeline 引用（无 TOCTOU 风险）。
    let pipeline = session.pipeline;

    // Remember the mode for deciding whether to auto-inject.
    let is_ai_assistant = matches!(session.config.mode, ProcessingMode::AiAssistant);
    let request = session.config;
    let cancel_token = session.cancel_token;

    let injector = injector_state.0.clone();
    let event_tx = event_bus.0.clone();
    let history = history_state.0.clone();

    // 5. 异步处理：编码音频 → 调用多模态 LLM → 注入文本。
    tokio::spawn(
        async move {
            match pipeline.process_audio(buffer, &request, cancel_token).await {
                Ok(processed_text) => {
                    if is_ai_assistant {
                        tracing::info!("AI assistant result ready (no auto-inject)");
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        if let Err(e) = injector.inject_text(&processed_text) {
                            tracing::error!("Text injection failed: {}", e);
                        }
                    }

                    let h = history.lock().await;
                    let _ = h.update_processed(&session_id, "", &processed_text);
                }
                Err(tingyuxuan_core::error::PipelineError::Cancelled) => {
                    tracing::info!("Session was cancelled");
                    let h = history.lock().await;
                    let _ = h.update_status(&session_id, "cancelled");
                }
                Err(error) => {
                    tracing::error!("Pipeline processing failed: {error}");
                    let se = StructuredError::from(&error);
                    let _ = event_tx.send(PipelineEvent::Error {
                        message: se.message,
                        user_action: se.user_action,
                    });
                    let h = history.lock().await;
                    let _ = h.update_status(&session_id, "failed");
                }
            }
        }
        .instrument(session_span),
    );
    Ok("processing".to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn cancel_recording(
    session_state: State<'_, SessionState>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
) -> Result<(), String> {
    // Cancel the recording via the actor (discards buffer).
    let _ = recorder_state.0.cancel().await;

    let session = session_state.0.lock().await.take();
    if let Some(session) = session {
        // Cancel any in-progress LLM operations.
        session.cancel_token.cancel();

        // Sentry breadcrumb: 录音取消
        sentry::add_breadcrumb(sentry::Breadcrumb {
            category: Some("recording".into()),
            message: Some("cancel".into()),
            level: sentry::Level::Info,
            data: {
                let mut m = sentry::protocol::Map::new();
                m.insert("session_id".into(), session.session_id.clone().into());
                m
            },
            ..Default::default()
        });

        // Update history.
        let history = history_state.0.lock().await;
        let _ = history.update_status(&session.session_id, "cancelled");

        tracing::info!("Recording cancelled: session={}", session.session_id);
    }
    Ok(())
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
