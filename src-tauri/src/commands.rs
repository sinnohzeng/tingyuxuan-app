use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tokio_util::sync::CancellationToken;

use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::history::TranscriptRecord;
use tingyuxuan_core::llm::openai_compat::OpenAICompatProvider;
use tingyuxuan_core::llm::provider::{LLMProvider, ProcessingMode};
use tingyuxuan_core::pipeline::events::PipelineEvent;
use tingyuxuan_core::pipeline::{Pipeline, ProcessingRequest};
use tingyuxuan_core::stt;

use crate::platform::{ContextDetector, TextInjector};
use crate::state::{
    ActiveSession, ConfigState, DetectorState, EventBus, HistoryState, InjectorState, NetworkState,
    PipelineState, QueueState, RecorderState, SessionState,
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
    if let Ok(entry) = keyring::Entry::new("tingyuxuan", service) {
        if let Ok(key) = entry.get_password() {
            if !key.is_empty() {
                return Some(key);
            }
        }
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

    let stt_provider = stt::create_stt_provider(&config.stt, stt_key).ok()?;

    let llm_base_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| config.llm_base_url());
    let llm_provider = Box::new(OpenAICompatProvider::new(
        llm_key,
        llm_base_url,
        config.llm.model.clone(),
    ));

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
    if pipeline_state.0.read().await.is_none() {
        return Err("请先在设置中配置 STT 和 LLM 的 API Key".to_string());
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let mut processing_mode = parse_mode(&mode);

    // Detect context: selected text and active window name.
    // These are synchronous system calls — fast enough to inline.
    let selected_text = detector_state.0.get_selected_text();
    let app_context = detector_state.0.get_active_window_name();

    // Auto-switch to Edit mode when user has selected text and pressed Dictate.
    // This enables the "speak-to-edit" workflow.
    if selected_text.is_some() && matches!(processing_mode, ProcessingMode::Dictate) {
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
    drop(config);

    // Determine the effective mode string for the recorder filename.
    let effective_mode = match processing_mode {
        ProcessingMode::Dictate => "dictate",
        ProcessingMode::Translate => "translate",
        ProcessingMode::AiAssistant => "ai_assistant",
        ProcessingMode::Edit => "edit",
    };

    // Determine cache directory for audio files.
    let cache_dir = AppConfig::data_dir()
        .map(|d| d.join("cache").join("audio"))
        .map_err(|e| format!("Cannot determine cache directory: {e}"))?;

    // Start the actual recording via the actor.
    let audio_path = recorder_state
        .0
        .start(&session_id, effective_mode, &cache_dir)
        .await?;

    // Store active session.
    let cancel_token = CancellationToken::new();
    let session = ActiveSession {
        session_id: session_id.clone(),
        mode: processing_mode,
        selected_text,
        target_language,
        app_context: app_context.clone(),
        cancel_token,
    };
    *session_state.0.lock().await = Some(session);

    // Emit recording started event.
    let _ = event_bus.0.send(PipelineEvent::RecordingStarted {
        session_id: session_id.clone(),
        mode: effective_mode.to_string(),
    });

    // Create a history record with status "recording".
    {
        let history = history_state.0.lock().await;
        let record = TranscriptRecord {
            id: session_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: effective_mode.to_string(),
            raw_text: None,
            processed_text: None,
            audio_path: Some(audio_path.to_string_lossy().to_string()),
            status: "recording".to_string(),
            app_context,
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

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn stop_recording(
    session_state: State<'_, SessionState>,
    pipeline_state: State<'_, PipelineState>,
    event_bus: State<'_, EventBus>,
    history_state: State<'_, HistoryState>,
    recorder_state: State<'_, RecorderState>,
    network_state: State<'_, NetworkState>,
    queue_state: State<'_, QueueState>,
    config_state: State<'_, ConfigState>,
    injector_state: State<'_, InjectorState>,
) -> Result<String, String> {
    use tingyuxuan_core::pipeline::queue::QueuedRecording;

    // Stop the actual recording via the actor.
    let (audio_path, duration_ms) = recorder_state.0.stop().await?;

    // Take the active session.
    let session = session_state
        .0
        .lock()
        .await
        .take()
        .ok_or_else(|| "No active recording session".to_string())?;

    let session_id = session.session_id.clone();

    // Emit recording stopped event with real duration.
    let _ = event_bus
        .0
        .send(PipelineEvent::RecordingStopped { duration_ms });

    // Check network status — if offline, queue for later processing.
    let is_online = network_state.0.load(std::sync::atomic::Ordering::Relaxed);

    if !is_online {
        let queued = QueuedRecording {
            session_id: session_id.clone(),
            audio_path,
            mode: session.mode,
            target_language: session.target_language,
            selected_text: session.selected_text,
            app_context: session.app_context,
        };
        queue_state.0.lock().await.enqueue(queued);

        let _ = event_bus.0.send(PipelineEvent::QueuedForLater {
            session_id: session_id.clone(),
        });

        // Update history status.
        if let Ok(history) = history_state.0.try_lock() {
            let _ = history.update_status(&session_id, "queued");
        }

        tracing::info!(
            "Recording queued for later (offline): session={}",
            session_id
        );
        return Ok("queued".to_string());
    }

    // Get pipeline reference.
    let pipeline = pipeline_state
        .0
        .read()
        .await
        .clone()
        .ok_or_else(|| "Pipeline not configured — check API keys in Settings".to_string())?;

    // Read user dictionary from config for the processing request.
    let user_dictionary = {
        let config = config_state.0.read().await;
        config.user_dictionary.clone()
    };

    // Build processing request with the actual recorded audio path.
    let request = ProcessingRequest {
        audio_path,
        mode: session.mode,
        app_context: session.app_context,
        target_language: session.target_language,
        selected_text: session.selected_text,
        user_dictionary,
    };

    let cancel_token = session.cancel_token;
    let history = history_state.0.clone();

    // Remember the mode for deciding whether to auto-inject.
    let is_ai_assistant = matches!(request.mode, ProcessingMode::AiAssistant);
    let injector = injector_state.0.clone();

    // Spawn async processing task — does not block the command response.
    tokio::spawn(async move {
        match pipeline.process_audio(&request, cancel_token).await {
            Ok(processed_text) => {
                if is_ai_assistant {
                    // AI assistant mode: show result panel, don't auto-inject.
                    // ProcessingComplete event is already emitted by Pipeline.
                    tracing::info!("AI assistant result ready (no auto-inject)");
                } else {
                    // Other modes: auto-inject text into the active application.
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    if let Err(e) = injector.inject_text(&processed_text) {
                        tracing::error!("Text injection failed: {}", e);
                    }
                }

                // Update history record.
                if let Ok(history) = history.try_lock() {
                    let _ = history.update_result(&session_id, &processed_text);
                }
            }
            Err(e) => {
                tracing::error!("Pipeline processing failed: {}", e);
                // Error event already emitted by the Pipeline.
                if let Ok(history) = history.try_lock() {
                    let _ = history.update_status(&session_id, "failed");
                }
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
    // Cancel the recording via the actor (deletes the WAV file).
    // Ignore errors — recording may have already been stopped.
    let _ = recorder_state.0.cancel().await;

    let session = session_state.0.lock().await.take();
    if let Some(session) = session {
        // Cancel any in-progress STT/LLM operations.
        session.cancel_token.cancel();

        // Update history.
        let history = history_state.0.lock().await;
        let _ = history.update_status(&session.session_id, "cancelled");

        tracing::info!("Recording cancelled: session={}", session.session_id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Retry
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

    // 2. Verify the audio file still exists (may have been cleaned up).
    let audio_path = record
        .audio_path
        .ok_or_else(|| "此记录没有关联的音频文件".to_string())?;
    let audio_path = PathBuf::from(&audio_path);
    if !audio_path.exists() {
        return Err("音频文件已过期，无法重试".to_string());
    }

    // 3. Get pipeline.
    let pipeline = pipeline_state
        .0
        .read()
        .await
        .clone()
        .ok_or_else(|| "Pipeline 未配置，请先设置 API Key".to_string())?;

    // 4. Build processing request.
    let request = ProcessingRequest {
        audio_path,
        mode: parse_mode(&record.mode),
        app_context: record.app_context,
        target_language: None,
        selected_text: None,
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
    let cancel = CancellationToken::new();
    let tx = event_bus.0.clone();
    tokio::spawn(async move {
        match pipeline.process_audio(&request, cancel).await {
            Ok(processed_text) => {
                let _ = tx.send(PipelineEvent::ProcessingComplete {
                    processed_text: processed_text.clone(),
                });
                if let Ok(h) = history.try_lock() {
                    let _ = h.update_result(&id, &processed_text);
                }
                tracing::info!("Retry succeeded for session {}", id);
            }
            Err(e) => {
                tracing::error!("Retry failed for {}: {}", id, e);
                if let Ok(h) = history.try_lock() {
                    let _ = h.update_status(&id, "failed");
                }
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

    let provider = stt::create_stt_provider(&config.stt, api_key).map_err(|e| e.to_string())?;

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
    let provider = OpenAICompatProvider::new(api_key, base_url, config.llm.model.clone());

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
// Helpers
// ---------------------------------------------------------------------------

fn parse_mode(mode: &str) -> ProcessingMode {
    match mode {
        "translate" => ProcessingMode::Translate,
        "ai_assistant" => ProcessingMode::AiAssistant,
        "edit" => ProcessingMode::Edit,
        _ => ProcessingMode::Dictate,
    }
}
