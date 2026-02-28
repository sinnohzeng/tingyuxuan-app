use std::sync::Arc;

use tauri::State;

use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::error::StructuredError;
use tingyuxuan_core::history::TranscriptRecord;
use tingyuxuan_core::llm::openai_compat::OpenAICompatProvider;
use tingyuxuan_core::llm::provider::{LLMProvider, ProcessingMode};
use tingyuxuan_core::pipeline::events::PipelineEvent;
use tingyuxuan_core::pipeline::{Pipeline, SessionConfig, SessionOrchestrator, SessionResult};
use tingyuxuan_core::stt;

use crate::platform::{ContextDetector, TextInjector};
use crate::state::{
    ActiveSession, ConfigState, DetectorState, EventBus, HistoryState, InjectorState,
    PipelineState, RecorderState, SessionState,
};

// ---------------------------------------------------------------------------
// Input validation constants
// ---------------------------------------------------------------------------

const MAX_INJECT_TEXT_LEN: usize = 50_000;
const MAX_API_KEY_LEN: usize = 512;
const MAX_SEARCH_QUERY_LEN: usize = 500;
const MAX_DICT_WORD_LEN: usize = 100;
const VALID_KEY_SERVICES: &[&str] = &["stt", "llm"];

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
/// `service` is "stt" or "llm".
#[tauri::command]
pub async fn save_api_key(service: String, key: String) -> Result<(), String> {
    // Validate service name against whitelist.
    if !VALID_KEY_SERVICES.contains(&service.as_str()) {
        return Err(format!("无效的服务名称: {service}（允许: stt, llm）"));
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
pub async fn get_api_key(service: String) -> Result<Option<String>, String> {
    if !VALID_KEY_SERVICES.contains(&service.as_str()) {
        return Err(format!("无效的服务名称: {service}（允许: stt, llm）"));
    }
    let entry =
        keyring::Entry::new("tingyuxuan", &service).map_err(|e| format!("Keyring error: {e}"))?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::PlatformFailure(_)) => {
            // No keyring backend available (headless server).
            tracing::warn!("Keyring platform not available — falling back to config.api_key_ref");
            Ok(None)
        }
        Err(e) => Err(format!("Failed to get key: {e}")),
    }
}

/// Try to get an API key: first from keyring, then fall back to config's api_key_ref.
fn resolve_api_key(service: &str, config_key_ref: &str) -> Option<String> {
    // Try keyring first.
    if let Ok(entry) = keyring::Entry::new("tingyuxuan", service)
        && let Ok(key) = entry.get_password()
        && !key.is_empty()
    {
        return Some(key);
    }
    // Fall back to config's api_key_ref (for dev/headless environments).
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
    let stt_key = resolve_api_key("stt", &config.stt.api_key_ref)?;
    let llm_key = resolve_api_key("llm", &config.llm.api_key_ref)?;

    let stt_provider = stt::create_streaming_stt_provider(&config.stt, stt_key).ok()?;

    let llm_base_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| config.llm_base_url());
    let llm_provider =
        Box::new(OpenAICompatProvider::new(llm_key, llm_base_url, config.llm.model.clone()).ok()?);

    Some(Arc::new(Pipeline::new(
        stt_provider,
        llm_provider,
        event_tx.clone(),
    )))
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
    // Validate pipeline is available before starting recording.
    let pipeline = pipeline_state
        .0
        .read()
        .await
        .clone()
        .ok_or_else(|| "请先在设置中配置 STT 和 LLM 的 API Key".to_string())?;

    let session_id = uuid::Uuid::new_v4().to_string();
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

    // 1. 启动录音 — 返回 PCM 音频帧 channel。
    let audio_rx = recorder_state.0.start().await?;

    // 2. 通过 SessionOrchestrator 建立流式 STT 连接。
    let session_config = SessionConfig {
        mode: processing_mode.clone(),
        context: context.clone(),
        target_language,
        user_dictionary,
        stt_options: tingyuxuan_core::stt::STTOptions {
            language: None,
            prompt: None,
        },
    };

    let managed_session = match SessionOrchestrator::start(&pipeline, session_config).await {
        Ok(session) => session,
        Err(e) => {
            // STT 连接失败，停止录音。
            let _ = recorder_state.0.stop().await;
            return Err(format!("流式 STT 连接失败: {e}"));
        }
    };

    // 3. 启动桥接 task：音频帧从 recorder → STT session。
    let stt_audio_tx = managed_session.audio_sender();
    tokio::spawn(async move {
        bridge_audio(audio_rx, stt_audio_tx).await;
    });

    // Serialize context for history storage.
    let context_json = serde_json::to_string(&context).ok();

    // Determine the effective mode string for events/history.
    let effective_mode = processing_mode.to_string();

    // Store active session.
    let session = ActiveSession {
        session_id: session_id.clone(),
        managed_session: Some(managed_session),
        started_at: std::time::Instant::now(),
    };
    *session_state.0.lock().await = Some(session);

    // Emit recording started event.
    let _ = event_bus.0.send(PipelineEvent::RecordingStarted {
        session_id: session_id.clone(),
        mode: effective_mode.clone(),
    });

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

    tracing::info!(
        "Recording started: session={}, mode={}",
        session_id,
        effective_mode
    );
    Ok(session_id)
}

/// 桥接 task：从 recorder 的 audio_rx 转发到 STT session 的 audio_tx。
/// recorder stop 时 audio_rx 关闭，此 task 自动结束，同时 drop audio_tx 通知 STT。
async fn bridge_audio(
    mut audio_rx: tokio::sync::mpsc::Receiver<tingyuxuan_core::stt::streaming::AudioChunk>,
    audio_tx: tokio::sync::mpsc::Sender<tingyuxuan_core::stt::streaming::AudioChunk>,
) {
    while let Some(chunk) = audio_rx.recv().await {
        // 背压：try_send 失败时丢帧，STT 容忍。
        if audio_tx.try_send(chunk).is_err() {
            // Channel full or closed — STT 端可能已关闭。
            if audio_tx.is_closed() {
                break;
            }
        }
    }
    // audio_rx 关闭 → drop audio_tx → STT 收到结束信号。
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn stop_recording(
    session_state: State<'_, SessionState>,
    pipeline_state: State<'_, PipelineState>,
    event_bus: State<'_, EventBus>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
    _config_state: State<'_, ConfigState>,
    injector_state: State<'_, InjectorState>,
) -> Result<String, String> {
    // 1. 停止录音 — drop audio_tx，桥接 task 结束，STT 开始产生最终结果。
    recorder_state.0.stop().await?;

    // 2. Take the active session.
    let session = session_state
        .0
        .lock()
        .await
        .take()
        .ok_or_else(|| "No active recording session".to_string())?;

    let session_id = session.session_id.clone();
    let duration_ms = session.started_at.elapsed().as_millis() as u64;

    // Emit recording stopped event.
    let _ = event_bus
        .0
        .send(PipelineEvent::RecordingStopped { duration_ms });

    // Get pipeline reference.
    let pipeline = pipeline_state
        .0
        .read()
        .await
        .clone()
        .ok_or_else(|| "Pipeline not configured — check API keys in Settings".to_string())?;

    // 取出 ManagedSession。
    let managed_session = session
        .managed_session
        .ok_or_else(|| "No streaming session available".to_string())?;

    // Remember the mode for deciding whether to auto-inject.
    let is_ai_assistant = matches!(managed_session.config().mode, ProcessingMode::AiAssistant);

    let injector = injector_state.0.clone();
    let event_tx = event_bus.0.clone();
    let history = history_state.0.clone();

    // 3. 异步处理：SessionOrchestrator::finish 处理完整流程。
    tokio::spawn(async move {
        match SessionOrchestrator::finish(&pipeline, managed_session).await {
            SessionResult::Success {
                raw_text,
                processed_text,
            } => {
                if is_ai_assistant {
                    tracing::info!("AI assistant result ready (no auto-inject)");
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    if let Err(e) = injector.inject_text(&processed_text) {
                        tracing::error!("Text injection failed: {}", e);
                    }
                }

                let h = history.lock().await;
                let _ = h.update_processed(&session_id, &raw_text, &processed_text);
            }
            SessionResult::EmptyTranscript => {
                tracing::warn!("STT returned empty transcript");
                let _ = event_tx.send(PipelineEvent::Error {
                    message: "未检测到语音内容，请重试".to_string(),
                    user_action: tingyuxuan_core::error::UserAction::Retry,
                    raw_text: None,
                });
                let h = history.lock().await;
                let _ = h.update_status(&session_id, "failed");
            }
            SessionResult::Failed { error, raw_text } => {
                tracing::error!("Pipeline processing failed: {error}");
                let se = StructuredError::from(&error);
                let _ = event_tx.send(PipelineEvent::Error {
                    message: se.message,
                    user_action: se.user_action,
                    raw_text,
                });
                let h = history.lock().await;
                let _ = h.update_status(&session_id, "failed");
            }
            SessionResult::Cancelled => {
                tracing::info!("Session was cancelled");
                let h = history.lock().await;
                let _ = h.update_status(&session_id, "cancelled");
            }
        }
    });

    tracing::info!("Recording stopped, processing started");
    Ok("processing".to_string())
}

#[tauri::command]
pub async fn cancel_recording(
    session_state: State<'_, SessionState>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
) -> Result<(), String> {
    // Cancel the recording via the actor.
    // Ignore errors — recording may have already been stopped.
    let _ = recorder_state.0.cancel().await;

    let session = session_state.0.lock().await.take();
    if let Some(session) = session {
        // Cancel any in-progress STT/LLM operations.
        if let Some(managed) = &session.managed_session {
            managed.cancel();
        }
        // Drop managed session → closes WebSocket.
        drop(session.managed_session);

        // Update history.
        let history = history_state.0.lock().await;
        let _ = history.update_status(&session.session_id, "cancelled");

        tracing::info!("Recording cancelled: session={}", session.session_id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Retry (重试 LLM 处理，使用历史记录中保存的 transcript)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn retry_transcription(
    id: String,
    pipeline_state: State<'_, PipelineState>,
    history_state: State<'_, HistoryState>,
    event_bus: State<'_, EventBus>,
) -> Result<(), String> {
    // 1. Look up the history record.
    let record = {
        let h = history_state.0.lock().await;
        h.get_by_id(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Record {} not found", id))?
    };

    // 2. 需要有原始转写文本才能重试 LLM。
    let raw_text = record
        .raw_text
        .ok_or_else(|| "此记录没有原始转写文本，无法重试".to_string())?;

    if raw_text.trim().is_empty() {
        return Err("原始转写文本为空，无法重试".to_string());
    }

    // 3. Get pipeline.
    let pipeline = pipeline_state
        .0
        .read()
        .await
        .clone()
        .ok_or_else(|| "Pipeline 未配置，请先设置 API Key".to_string())?;

    // 4. Build processing request.
    let context = record
        .context_json
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();
    let mode = record
        .mode
        .parse::<ProcessingMode>()
        .unwrap_or(ProcessingMode::Dictate);
    let request = tingyuxuan_core::pipeline::ProcessingRequest {
        mode,
        context,
        target_language: None,
        user_dictionary: Vec::new(),
    };

    // 5. Emit processing started event.
    let _ = event_bus.0.send(PipelineEvent::ProcessingStarted);

    // 6. Update status to "processing".
    {
        let h = history_state.0.lock().await;
        let _ = h.update_status(&id, "processing");
    }

    // 7. Spawn async processing.
    let history = history_state.0.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    let tx = event_bus.0.clone();
    tokio::spawn(async move {
        match pipeline
            .process_transcript(raw_text, &request, cancel)
            .await
        {
            Ok(processed_text) => {
                let _ = tx.send(PipelineEvent::ProcessingComplete {
                    processed_text: processed_text.clone(),
                });
                let h = history.lock().await;
                let _ = h.update_result(&id, &processed_text);
                tracing::info!("Retry succeeded for session {}", id);
            }
            Err(e) => {
                tracing::error!("Retry failed for {}: {}", id, e);
                let h = history.lock().await;
                let _ = h.update_status(&id, "failed");
            }
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Text injection
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn inject_text(
    text: String,
    injector_state: State<'_, InjectorState>,
) -> Result<(), String> {
    check_max_len(&text, MAX_INJECT_TEXT_LEN, "注入文本")?;
    check_no_null_bytes(&text, "注入文本")?;
    injector_state
        .0
        .inject_text(&text)
        .map_err(|e| format!("Text injection failed: {e}"))
}

// ---------------------------------------------------------------------------
// Connection testing
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn test_stt_connection(config_state: State<'_, ConfigState>) -> Result<bool, String> {
    let config = config_state.0.read().await;
    let api_key = resolve_api_key("stt", &config.stt.api_key_ref)
        .ok_or_else(|| "No STT API key configured".to_string())?;

    let provider =
        stt::create_streaming_stt_provider(&config.stt, api_key).map_err(|e| e.to_string())?;

    provider.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_llm_connection(config_state: State<'_, ConfigState>) -> Result<bool, String> {
    let config = config_state.0.read().await;
    let api_key = resolve_api_key("llm", &config.llm.api_key_ref)
        .ok_or_else(|| "No LLM API key configured".to_string())?;

    let base_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| config.llm_base_url());
    let provider = OpenAICompatProvider::new(api_key, base_url, config.llm.model.clone())
        .map_err(|e| e.to_string())?;

    provider.test_connection().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn is_first_launch(pipeline_state: State<'_, PipelineState>) -> Result<bool, String> {
    Ok(pipeline_state.0.read().await.is_none())
}

#[tauri::command]
pub async fn get_config(config_state: State<'_, ConfigState>) -> Result<AppConfig, String> {
    let config = config_state.0.read().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    config_state: State<'_, ConfigState>,
    pipeline_state: State<'_, PipelineState>,
    event_bus: State<'_, EventBus>,
) -> Result<(), String> {
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
pub async fn get_recent_history(
    limit: u32,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<TranscriptRecord>, String> {
    let history = history_state.0.lock().await;
    history.get_recent(limit).map_err(|e| e.to_string())
}

#[tauri::command]
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
pub async fn get_history_page(
    limit: u32,
    offset: u32,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<TranscriptRecord>, String> {
    let history = history_state.0.lock().await;
    history.get_page(limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_history(
    id: String,
    history_state: State<'_, HistoryState>,
) -> Result<(), String> {
    let history = history_state.0.lock().await;
    history.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_history_batch(
    ids: Vec<String>,
    history_state: State<'_, HistoryState>,
) -> Result<u64, String> {
    let history = history_state.0.lock().await;
    history.delete_batch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(history_state: State<'_, HistoryState>) -> Result<u64, String> {
    let history = history_state.0.lock().await;
    history.clear_all().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Dictionary
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_dictionary(config_state: State<'_, ConfigState>) -> Result<Vec<String>, String> {
    let config = config_state.0.read().await;
    Ok(config.user_dictionary.clone())
}

#[tauri::command]
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
        return Ok(()); // Already exists, no-op.
    }
    config.user_dictionary.push(trimmed);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
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
// Platform Permissions (macOS)
// ---------------------------------------------------------------------------

/// 检查 macOS 平台权限状态。
///
/// macOS 需要 Accessibility + Input Monitoring 权限，返回四值状态。
/// 其他平台始终返回 "granted"。
#[tauri::command]
pub async fn check_platform_permissions() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let status = crate::platform::macos::check_permissions();
        // 序列化为 snake_case 字符串（与 serde rename_all 一致）
        serde_json::to_string(&status)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(|e| format!("Serialization error: {e}"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok("granted".to_string())
    }
}

/// 打开系统权限设置页面。
///
/// `target` 参数可选：
/// - `"input_monitoring"` → 输入监控面板
/// - 其他值或 None → 辅助功能面板（默认）
#[tauri::command]
pub async fn open_permission_settings(target: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::open_permission_settings_for(target.as_deref());
    }
    let _ = target; // 非 macOS 平台忽略参数
    Ok(())
}
