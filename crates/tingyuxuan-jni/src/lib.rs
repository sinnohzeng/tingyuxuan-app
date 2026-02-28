//! JNI bridge for TingYuXuan Android IME.
//!
//! This crate provides JNI-exported functions that allow the Kotlin/Java
//! Android IME to call into the Rust `tingyuxuan-core` engine.
//!
//! # Handle Safety
//!
//! Instead of passing raw pointers across the JNI boundary (which would risk
//! use-after-free and double-free bugs), we use a **generation-based handle
//! table**: each Pipeline instance is stored in a global `HashMap<u64, Arc<Pipeline>>`
//! and the Kotlin side only sees an opaque `Long` handle ID.
//!
//! # Streaming Architecture
//!
//! 流式录音流程：
//! 1. `startStreaming(handle, contextJson)` → 建立 WebSocket + 创建 session
//! 2. `sendAudioChunk(handle, pcmData)` → 发送 PCM 帧到 STT
//! 3. `stopStreaming(handle)` → 收集 STT 结果 → LLM 处理 → 返回结果 JSON
//!
//! # Runtime Management
//!
//! A single tokio `Runtime` is shared across all JNI calls via `OnceLock`,
//! avoiding the overhead of creating a new runtime per call.

mod handle;

use handle::{get_handle, register_handle, remove_handle};
use jni::EnvUnowned;
use jni::errors::LogErrorAndDefault;
use jni::objects::{JClass, JShortArray, JString};
use jni::sys::jlong;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tingyuxuan_core::context::InputContext;
use tingyuxuan_core::error::StructuredError;
use tingyuxuan_core::llm::LLMProvider;
use tingyuxuan_core::llm::provider::ProcessingMode;
use tingyuxuan_core::pipeline::{
    ManagedSession, Pipeline, SessionConfig, SessionOrchestrator, SessionResult,
};
use tingyuxuan_core::stt::streaming::AudioChunk;

/// 全局共享的 tokio Runtime，通过 OnceLock 懒初始化。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 获取或初始化全局 tokio Runtime。
fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2) // Android 设备通常核心有限
            .enable_all()
            .thread_name("tingyuxuan-rt")
            .build()
            // Tokio runtime creation is a one-time, unrecoverable operation. If it fails,
            // the JNI layer cannot function at all, so panicking is the correct behavior.
            .expect("Failed to create tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Streaming session state
// ---------------------------------------------------------------------------

/// 全局流式会话表。key = pipeline handle ID。
static STREAMING_SESSIONS: OnceLock<Mutex<HashMap<u64, ManagedSession>>> = OnceLock::new();

fn streaming_sessions() -> &'static Mutex<HashMap<u64, ManagedSession>> {
    STREAMING_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_managed_session(handle_id: u64, session: ManagedSession) -> Result<(), String> {
    let mut map = streaming_sessions()
        .lock()
        .map_err(|_| "Streaming sessions mutex poisoned".to_string())?;
    if map.contains_key(&handle_id) {
        tracing::warn!(
            handle_id,
            "Replacing existing streaming session (resource leak)"
        );
    }
    map.insert(handle_id, session);
    Ok(())
}

fn take_managed_session(handle_id: u64) -> Result<ManagedSession, String> {
    let mut map = streaming_sessions()
        .lock()
        .map_err(|_| "Streaming sessions mutex poisoned".to_string())?;
    map.remove(&handle_id)
        .ok_or_else(|| format!("No active streaming session for handle: {handle_id}"))
}

// ---------------------------------------------------------------------------
// JNI_OnLoad
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("TingYuXuanRust"),
    );
    let _ = tracing_log::LogTracer::init();

    tracing::info!("TingYuXuan JNI library loaded");
    jni::sys::JNI_VERSION_1_6
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 构建错误 JSON 字符串，供 Kotlin 侧解析。
fn error_json(error_code: &str, message: &str, user_action: &str) -> String {
    serde_json::json!({
        "success": false,
        "error_code": error_code,
        "message": message,
        "user_action": user_action,
    })
    .to_string()
}

/// 从 StructuredError 构建 JSON 字符串。
fn structured_error_to_json(se: &StructuredError) -> String {
    serde_json::json!({
        "success": false,
        "error_code": se.error_code,
        "message": se.message,
        "user_action": format!("{:?}", se.user_action).to_lowercase(),
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// initPipeline / destroyPipeline
// ---------------------------------------------------------------------------

/// Initialize a new processing pipeline and return an opaque handle.
///
/// # JNI Signature
/// `(Ljava/lang/String;)J` — takes config JSON, returns handle (long).
/// Returns 0 on failure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_initPipeline(
    mut env: EnvUnowned,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let _span = tracing::info_span!("jni_init_pipeline").entered();

    env.with_env(|env| -> jni::errors::Result<jlong> {
        let config_str: String = config_json.try_to_string(env)?;

        let config: tingyuxuan_core::config::AppConfig = match serde_json::from_str(&config_str) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to parse config JSON: {e}");
                return Ok(0);
            }
        };

        let stt_key = config.stt.api_key_ref.clone();
        let llm_key = config.llm.api_key_ref.clone();

        let stt_provider =
            match tingyuxuan_core::stt::create_streaming_stt_provider(&config.stt, stt_key) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Failed to create STT provider: {e}");
                    return Ok(0);
                }
            };

        let llm_base_url = config
            .llm
            .base_url
            .clone()
            .unwrap_or_else(|| config.llm_base_url());
        let llm_provider = match tingyuxuan_core::llm::openai_compat::OpenAICompatProvider::new(
            llm_key,
            llm_base_url,
            config.llm.model.clone(),
        ) {
            Ok(p) => Box::new(p),
            Err(e) => {
                tracing::warn!("Failed to create LLM provider: {e}");
                return Ok(0);
            }
        };

        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        let pipeline = Arc::new(Pipeline::new(stt_provider, llm_provider, event_tx));

        let handle = match register_handle(pipeline) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to register pipeline handle: {e}");
                return Ok(0);
            }
        };
        tracing::info!(handle, "Pipeline initialized");
        Ok(handle as jlong)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Destroy a pipeline handle, releasing the associated resources.
///
/// # JNI Signature
/// `(J)V`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_destroyPipeline(
    _env: EnvUnowned,
    _class: JClass,
    handle: jlong,
) {
    let _span = tracing::info_span!("jni_destroy_pipeline", handle).entered();

    // 同时清理可能残留的流式会话。
    if let Ok(session) = take_managed_session(handle as u64) {
        session.cancel();
    }

    match remove_handle(handle as u64) {
        Ok(true) => tracing::info!(handle, "Pipeline destroyed"),
        Ok(false) => tracing::warn!(handle, "Attempted to destroy invalid pipeline handle"),
        Err(e) => tracing::error!(handle, "Failed to remove handle: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Streaming API: startStreaming / sendAudioChunk / stopStreaming
// ---------------------------------------------------------------------------

/// 开始流式 STT 会话。
///
/// 建立 WebSocket 连接，准备接收音频帧。
///
/// # JNI Signature
/// `(JLjava/lang/String;Ljava/lang/String;)Ljava/lang/String;`
/// 参数：handle, mode, contextJson
/// 返回：JSON `{ "success": true }` 或错误 JSON
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_startStreaming<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    mode: JString<'local>,
    context_json: JString<'local>,
) -> JString<'local> {
    let _span = tracing::info_span!("jni_start_streaming", handle).entered();

    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        let mode_str: String = mode.try_to_string(env)?;
        let context: InputContext = context_json
            .try_to_string(env)
            .ok()
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(|s| match serde_json::from_str(&s) {
                Ok(ctx) => Some(ctx),
                Err(e) => {
                    tracing::warn!("Failed to parse InputContext JSON: {e}");
                    None
                }
            })
            .unwrap_or_default();

        let pipeline = match get_handle(handle as u64) {
            Ok(p) => p,
            Err(e) => {
                let json = error_json("invalid_handle", &e, "dismiss");
                return env.new_string(json);
            }
        };

        let processing_mode = mode_str.parse::<ProcessingMode>().unwrap_or_else(|_| {
            tracing::warn!("Unknown processing mode '{mode_str}', falling back to Dictate");
            ProcessingMode::Dictate
        });

        let session_config = SessionConfig {
            mode: processing_mode,
            context,
            target_language: None,
            user_dictionary: Vec::new(),
            stt_options: tingyuxuan_core::stt::STTOptions {
                language: None,
                prompt: None,
            },
        };

        let rt = runtime();
        let result = rt.block_on(SessionOrchestrator::start(&pipeline, session_config));

        match result {
            Ok(managed_session) => {
                if let Err(e) = store_managed_session(handle as u64, managed_session) {
                    let json = error_json("internal_error", &e, "retry");
                    return env.new_string(json);
                }
                let json = serde_json::json!({ "success": true }).to_string();
                env.new_string(json)
            }
            Err(e) => {
                tracing::warn!("Failed to start streaming: {e}");
                let se = StructuredError::from(&e);
                let json = structured_error_to_json(&se);
                env.new_string(json)
            }
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// 发送一帧 PCM 音频到流式 STT。
///
/// # JNI Signature
/// `(J[S)Z`
/// 参数：handle, pcmData (ShortArray)
/// 返回：true 表示成功发送，false 表示会话不存在或已关闭
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_sendAudioChunk(
    mut env: EnvUnowned,
    _class: JClass,
    handle: jlong,
    pcm_data: JShortArray,
) -> bool {
    env.with_env(|env| -> jni::errors::Result<bool> {
        let map = match streaming_sessions().lock() {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };
        let Some(session) = map.get(&(handle as u64)) else {
            return Ok(false);
        };

        let len = pcm_data.len(env)?;
        let mut buf = vec![0i16; len];
        pcm_data.get_region(env, 0, &mut buf)?;

        Ok(session.send_audio(AudioChunk { samples: buf }))
    })
    .resolve::<LogErrorAndDefault>()
}

/// 停止流式录音，收集 STT 结果，执行 LLM 处理。
///
/// 阻塞调用 — 等待 STT 最终结果和 LLM 处理完成。
///
/// # JNI Signature
/// `(J)Ljava/lang/String;`
/// 参数：handle
/// 返回：JSON `{ "success": true, "text": "..." }` 或错误 JSON
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_stopStreaming<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> JString<'local> {
    let _span = tracing::info_span!("jni_stop_streaming", handle).entered();

    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        let pipeline = match get_handle(handle as u64) {
            Ok(p) => p,
            Err(e) => {
                let json = error_json("invalid_handle", &e, "dismiss");
                return env.new_string(json);
            }
        };

        let session = match take_managed_session(handle as u64) {
            Ok(s) => s,
            Err(e) => {
                let json = error_json("no_session", &e, "dismiss");
                return env.new_string(json);
            }
        };

        let rt = runtime();
        let json_str = rt.block_on(async {
            match SessionOrchestrator::finish(&pipeline, session).await {
                SessionResult::Success { processed_text, .. } => {
                    serde_json::json!({ "success": true, "text": processed_text }).to_string()
                }
                SessionResult::EmptyTranscript => {
                    error_json("stt_empty", "STT returned empty text", "retry")
                }
                SessionResult::Failed { error, .. } => {
                    tracing::error!("Pipeline processing failed: {error}");
                    let se = StructuredError::from(&error);
                    structured_error_to_json(&se)
                }
                SessionResult::Cancelled => {
                    error_json("cancelled", "Processing cancelled", "dismiss")
                }
            }
        });

        env.new_string(json_str)
    })
    .resolve::<LogErrorAndDefault>()
}

// ---------------------------------------------------------------------------
// Utility JNI functions
// ---------------------------------------------------------------------------

/// Validate a config JSON string without creating a pipeline.
///
/// # JNI Signature
/// `(Ljava/lang/String;)Ljava/lang/String;`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_validateConfig<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    config_json: JString<'local>,
) -> JString<'local> {
    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        let config_str: String = config_json.try_to_string(env)?;

        let json = match serde_json::from_str::<tingyuxuan_core::config::AppConfig>(&config_str) {
            Ok(_) => serde_json::json!({ "success": true }).to_string(),
            Err(e) => error_json("invalid_config", &e.to_string(), "open_settings"),
        };

        env.new_string(json)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Test connectivity to STT or LLM service.
///
/// # JNI Signature
/// `(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_testConnection<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    config_json: JString<'local>,
    service: JString<'local>,
) -> JString<'local> {
    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        let config_str: String = config_json.try_to_string(env)?;
        let service_str: String = service.try_to_string(env)?;

        let config: tingyuxuan_core::config::AppConfig = match serde_json::from_str(&config_str) {
            Ok(c) => c,
            Err(e) => {
                let json = error_json(
                    "invalid_config",
                    &format!("Invalid config: {e}"),
                    "open_settings",
                );
                return env.new_string(json);
            }
        };

        let rt = runtime();

        let json = match service_str.as_str() {
            "stt" => {
                let stt_key = config.stt.api_key_ref.clone();
                match tingyuxuan_core::stt::create_streaming_stt_provider(&config.stt, stt_key) {
                    Ok(provider) => match rt.block_on(provider.test_connection()) {
                        Ok(true) => serde_json::json!({ "success": true }).to_string(),
                        Ok(false) => {
                            error_json("stt_error", "Connection test failed", "check_api_key")
                        }
                        Err(e) => error_json("stt_error", &e.to_string(), "check_api_key"),
                    },
                    Err(e) => error_json("stt_error", &e.to_string(), "open_settings"),
                }
            }
            "llm" => {
                let llm_key = config.llm.api_key_ref.clone();
                let llm_base_url = config
                    .llm
                    .base_url
                    .clone()
                    .unwrap_or_else(|| config.llm_base_url());
                let provider: Box<dyn LLMProvider> =
                    match tingyuxuan_core::llm::openai_compat::OpenAICompatProvider::new(
                        llm_key,
                        llm_base_url,
                        config.llm.model.clone(),
                    ) {
                        Ok(p) => Box::new(p),
                        Err(e) => {
                            let json = error_json(
                                "llm_error",
                                &format!("LLM init failed: {e}"),
                                "open_settings",
                            );
                            return env.new_string(&json);
                        }
                    };
                match rt.block_on(provider.test_connection()) {
                    Ok(true) => serde_json::json!({ "success": true }).to_string(),
                    Ok(false) => error_json("llm_error", "Connection test failed", "check_api_key"),
                    Err(e) => error_json("llm_error", &e.to_string(), "check_api_key"),
                }
            }
            other => error_json(
                "invalid_service",
                &format!("Unknown service: {other}"),
                "dismiss",
            ),
        };

        env.new_string(json)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Cancel an in-progress streaming session.
///
/// 不 take session — 只取消令牌。stopStreaming 仍能 take 并 finish。
/// 这解决了 C2 竞争问题：cancelProcessing 与 stopStreaming 不再冲突。
///
/// # JNI Signature
/// `(J)V`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_cancelProcessing(
    _env: EnvUnowned,
    _class: JClass,
    handle: jlong,
) {
    let _span = tracing::info_span!("jni_cancel_processing", handle).entered();
    let map = streaming_sessions().lock();
    match map {
        Ok(map) => {
            if let Some(session) = map.get(&(handle as u64)) {
                session.cancel();
                tracing::info!(handle, "Streaming session cancelled");
            } else {
                tracing::info!(handle, "Cancel requested but no active session");
            }
        }
        Err(_) => {
            tracing::error!(handle, "Streaming sessions mutex poisoned");
        }
    }
}

/// Get the core library version string.
///
/// # JNI Signature
/// `()Ljava/lang/String;`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_getVersion<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        env.new_string(env!("CARGO_PKG_VERSION"))
    })
    .resolve::<LogErrorAndDefault>()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::handle::{get_handle, remove_handle};
    use super::*;
    use tingyuxuan_core::stt::streaming::AudioChunk;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_handle_register_get_remove() {
        assert!(get_handle(0).is_err());
        assert!(!remove_handle(0).unwrap());
    }

    #[test]
    fn test_runtime_singleton() {
        let rt1 = runtime();
        let rt2 = runtime();
        assert!(std::ptr::eq(rt1, rt2));
    }

    #[test]
    fn test_error_json_format() {
        let json = error_json("test_error", "something failed", "retry");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error_code"], "test_error");
        assert_eq!(parsed["message"], "something failed");
        assert_eq!(parsed["user_action"], "retry");
    }

    #[test]
    fn test_version_not_empty() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        assert!(version.starts_with("0."));
    }

    #[test]
    fn test_managed_session_store_and_take() {
        let (tx, _rx) = mpsc::channel::<AudioChunk>(10);
        let (_, event_rx) = mpsc::channel(10);

        // 构造一个 ManagedSession 用于测试存取。
        // 直接构造而非通过 SessionOrchestrator::start() 以避免需要 Pipeline。
        let session = ManagedSession::new_for_testing(
            tx,
            event_rx,
            SessionConfig {
                mode: ProcessingMode::Dictate,
                context: InputContext::default(),
                target_language: None,
                user_dictionary: Vec::new(),
                stt_options: tingyuxuan_core::stt::STTOptions {
                    language: None,
                    prompt: None,
                },
            },
            CancellationToken::new(),
        );

        let test_handle_id = 777_777_777u64;
        store_managed_session(test_handle_id, session).unwrap();

        // Take it.
        let taken = take_managed_session(test_handle_id);
        assert!(taken.is_ok());

        // Should be gone now.
        assert!(take_managed_session(test_handle_id).is_err());
    }

    #[test]
    fn test_take_nonexistent_streaming_session() {
        assert!(take_managed_session(666_666_666).is_err());
    }

    #[test]
    fn test_structured_error_to_json() {
        let se = StructuredError::from(&tingyuxuan_core::error::PipelineError::Cancelled);
        let json = structured_error_to_json(&se);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error_code"], "cancelled");
    }
}
