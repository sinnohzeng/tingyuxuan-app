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
//! # Runtime Management
//!
//! A single tokio `Runtime` is shared across all JNI calls via `OnceLock`,
//! avoiding the overhead of creating a new runtime per call (~10-50ms + thread
//! pool resource leaks).

mod handle;

use handle::{get_handle, register_handle, remove_handle};
use jni::EnvUnowned;
use jni::errors::LogErrorAndDefault;
use jni::objects::{JClass, JString};
use jni::sys::jlong;
use std::sync::{Arc, OnceLock};
use tingyuxuan_core::llm::LLMProvider;
use tingyuxuan_core::pipeline::Pipeline;

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

/// JNI_OnLoad — 在 System.loadLibrary() 时自动调用。
/// 初始化 Android logcat 日志输出。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    // 初始化 android_logger → logcat
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("TingYuXuanRust"),
    );
    // 桥接 tracing → log → logcat
    let _ = tracing_log::LogTracer::init();

    tracing::info!("TingYuXuan JNI library loaded");
    jni::sys::JNI_VERSION_1_6
}

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

/// 从 PipelineError 映射到结构化错误 JSON。
fn pipeline_error_to_json(e: &tingyuxuan_core::error::PipelineError) -> String {
    use tingyuxuan_core::error::PipelineError;
    match e {
        PipelineError::Stt(stt_err) => {
            use tingyuxuan_core::error::STTError;
            match stt_err {
                STTError::AuthFailed => {
                    error_json("stt_auth_failed", &e.to_string(), "check_api_key")
                }
                STTError::Timeout(_) => error_json("timeout", &e.to_string(), "retry"),
                STTError::NetworkError(_) => {
                    error_json("network_error", &e.to_string(), "retry")
                }
                STTError::NotConfigured => {
                    error_json("not_configured", &e.to_string(), "open_settings")
                }
                _ => error_json("stt_error", &e.to_string(), "retry"),
            }
        }
        PipelineError::Llm(llm_err) => {
            use tingyuxuan_core::error::LLMError;
            match llm_err {
                LLMError::AuthFailed => {
                    error_json("llm_auth_failed", &e.to_string(), "check_api_key")
                }
                LLMError::Timeout => error_json("timeout", &e.to_string(), "retry"),
                LLMError::NetworkError(_) => {
                    error_json("network_error", &e.to_string(), "retry")
                }
                LLMError::NotConfigured => {
                    error_json("not_configured", &e.to_string(), "open_settings")
                }
                _ => error_json("llm_error", &e.to_string(), "retry"),
            }
        }
        PipelineError::Cancelled => error_json("cancelled", "Processing cancelled", "dismiss"),
        PipelineError::Busy => error_json("busy", "Pipeline is busy", "retry"),
        PipelineError::Audio(audio_err) => {
            error_json("audio_error", &audio_err.to_string(), "retry")
        }
    }
}

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
                tracing::error!("Failed to parse config JSON: {e}");
                return Ok(0);
            }
        };

        // Create pipeline components from config.
        let stt_key = config.stt.api_key_ref.clone();
        let llm_key = config.llm.api_key_ref.clone();

        let stt_provider = match tingyuxuan_core::stt::create_stt_provider(&config.stt, stt_key) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to create STT provider: {e}");
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
                tracing::error!("Failed to create LLM provider: {e}");
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

/// Process an audio file through the pipeline.
///
/// Returns a JSON string with structured result including error_code and user_action
/// on failure, enabling the Kotlin UI to show appropriate error messages.
///
/// # JNI Signature
/// `(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_processAudio<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    audio_path: JString<'local>,
    mode: JString<'local>,
    selected_text: JString<'local>,
) -> JString<'local> {
    let _span = tracing::info_span!("jni_process_audio", handle).entered();

    env.with_env(|env| -> jni::errors::Result<JString<'local>> {
        // Extract all JNI strings upfront to avoid borrow conflicts.
        let audio_path_str: String = audio_path.try_to_string(env)?;
        let mode_str: String = mode.try_to_string(env)?;
        let selected_text_opt: Option<String> = selected_text
            .try_to_string(env)
            .ok()
            .and_then(|s| if s.is_empty() { None } else { Some(s) });

        let pipeline = match get_handle(handle as u64) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Invalid handle: {e}");
                let json = error_json("invalid_handle", &e, "dismiss");
                return env.new_string(json);
            }
        };

        let processing_mode = match mode_str.as_str() {
            "translate" => tingyuxuan_core::llm::provider::ProcessingMode::Translate,
            "ai_assistant" => tingyuxuan_core::llm::provider::ProcessingMode::AiAssistant,
            "edit" => tingyuxuan_core::llm::provider::ProcessingMode::Edit,
            _ => tingyuxuan_core::llm::provider::ProcessingMode::Dictate,
        };

        let request = tingyuxuan_core::pipeline::ProcessingRequest {
            audio_path: std::path::PathBuf::from(audio_path_str),
            mode: processing_mode,
            app_context: None,
            target_language: None,
            selected_text: selected_text_opt,
            user_dictionary: Vec::new(),
        };

        let cancel = tokio_util::sync::CancellationToken::new();

        // 使用全局共享的 tokio Runtime。
        let rt = runtime();
        let result = rt.block_on(pipeline.process_audio(&request, cancel));

        let json_str = match result {
            Ok(text) => serde_json::json!({ "success": true, "text": text }).to_string(),
            Err(e) => {
                tracing::error!("Pipeline processing failed: {e}");
                pipeline_error_to_json(&e)
            }
        };

        env.new_string(json_str)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Destroy a pipeline handle, releasing the associated resources.
///
/// # JNI Signature
/// `(J)V` — takes handle (long), returns void.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_destroyPipeline(
    _env: EnvUnowned,
    _class: JClass,
    handle: jlong,
) {
    let _span = tracing::info_span!("jni_destroy_pipeline", handle).entered();

    match remove_handle(handle as u64) {
        Ok(true) => tracing::info!(handle, "Pipeline destroyed"),
        Ok(false) => tracing::warn!(handle, "Attempted to destroy invalid pipeline handle"),
        Err(e) => tracing::error!(handle, "Failed to remove handle: {e}"),
    }
}

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
            Ok(_) => serde_json::json!({ "valid": true }).to_string(),
            Err(e) => serde_json::json!({ "valid": false, "error": e.to_string() }).to_string(),
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
                let json =
                    serde_json::json!({ "success": false, "error": format!("Invalid config: {e}") })
                        .to_string();
                return env.new_string(json);
            }
        };

        let rt = runtime();

        let json = match service_str.as_str() {
            "stt" => {
                let stt_key = config.stt.api_key_ref.clone();
                match tingyuxuan_core::stt::create_stt_provider(&config.stt, stt_key) {
                    Ok(provider) => match rt.block_on(provider.test_connection()) {
                        Ok(true) => serde_json::json!({ "success": true }).to_string(),
                        Ok(false) => {
                            serde_json::json!({ "success": false, "error": "Connection test failed" })
                                .to_string()
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            serde_json::json!({ "success": false, "error": msg }).to_string()
                        }
                    },
                    Err(e) => {
                        let msg = e.to_string();
                        serde_json::json!({ "success": false, "error": msg }).to_string()
                    }
                }
            }
            "llm" => {
                let llm_key = config.llm.api_key_ref.clone();
                let llm_base_url = config
                    .llm
                    .base_url
                    .clone()
                    .unwrap_or_else(|| config.llm_base_url());
                let provider: Box<dyn LLMProvider> = match tingyuxuan_core::llm::openai_compat::OpenAICompatProvider::new(
                    llm_key,
                    llm_base_url,
                    config.llm.model.clone(),
                ) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        let json = serde_json::json!({ "success": false, "error": format!("LLM init failed: {e}") }).to_string();
                        return Ok(env.new_string(&json)?);
                    }
                };
                match rt.block_on(provider.test_connection()) {
                    Ok(true) => serde_json::json!({ "success": true }).to_string(),
                    Ok(false) => {
                        serde_json::json!({ "success": false, "error": "Connection test failed" })
                            .to_string()
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        serde_json::json!({ "success": false, "error": msg }).to_string()
                    }
                }
            }
            other => {
                serde_json::json!({ "success": false, "error": format!("Unknown service: {other}") })
                    .to_string()
            }
        };

        env.new_string(json)
    })
    .resolve::<LogErrorAndDefault>()
}

/// Cancel an in-progress audio processing task.
/// 当前实现为销毁并重建 pipeline（未来可改为 CancellationToken 传递）。
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
    // TODO: 传递 CancellationToken 进行取消。
    // 当前通过 pipeline handle 无法直接取消，仅记录日志。
    tracing::info!(handle, "Cancel processing requested (not yet implemented)");
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

    #[test]
    fn test_handle_register_get_remove() {
        // We can't easily create a real Pipeline in tests without API keys,
        // so we test the handle table mechanics via the get/remove interface.
        // Note: handle module tests cover the core logic.
        assert!(get_handle(0).is_err());
        assert!(!remove_handle(0).unwrap());
    }

    #[test]
    fn test_runtime_singleton() {
        let rt1 = runtime();
        let rt2 = runtime();
        // 确保返回的是同一个 Runtime 实例。
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
}
