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

mod handle;

use handle::{get_handle, register_handle, remove_handle};
use jni::objects::{JClass, JString};
use jni::sys::jlong;
use jni::JNIEnv;
use std::sync::Arc;
use tingyuxuan_core::pipeline::Pipeline;

/// Initialize a new processing pipeline and return an opaque handle.
///
/// # JNI Signature
/// `(Ljava/lang/String;)J` — takes config JSON, returns handle (long).
#[no_mangle]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_initPipeline(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let _span = tracing::info_span!("jni_init_pipeline").entered();

    let config_str: String = match env.get_string(&config_json) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("Failed to get config string from JNI: {e}");
            return 0;
        }
    };

    let config: tingyuxuan_core::config::AppConfig = match serde_json::from_str(&config_str) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to parse config JSON: {e}");
            return 0;
        }
    };

    // Create pipeline components from config.
    let stt_key = config.stt.api_key_ref.clone();
    let llm_key = config.llm.api_key_ref.clone();

    let stt_provider = match tingyuxuan_core::stt::create_stt_provider(&config.stt, stt_key) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to create STT provider: {e}");
            return 0;
        }
    };

    let llm_base_url = config
        .llm
        .base_url
        .clone()
        .unwrap_or_else(|| config.llm_base_url());
    let llm_provider = Box::new(
        tingyuxuan_core::llm::openai_compat::OpenAICompatProvider::new(
            llm_key,
            llm_base_url,
            config.llm.model.clone(),
        ),
    );

    let (event_tx, _) = tokio::sync::broadcast::channel(64);
    let pipeline = Arc::new(Pipeline::new(stt_provider, llm_provider, event_tx));

    let handle = register_handle(pipeline);
    tracing::info!(handle, "Pipeline initialized");
    handle as jlong
}

/// Process an audio file through the pipeline.
///
/// # JNI Signature
/// `(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;`
#[no_mangle]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_processAudio<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    audio_path: JString<'local>,
    mode: JString<'local>,
    selected_text: JString<'local>,
) -> JString<'local> {
    let _span = tracing::info_span!("jni_process_audio", handle).entered();

    // Extract all JNI strings upfront to avoid borrow conflicts.
    let audio_path_str: String = match env.get_string(&audio_path) {
        Ok(s) => s.into(),
        Err(_) => return env.new_string("").expect("empty string"),
    };
    let mode_str: String = match env.get_string(&mode) {
        Ok(s) => s.into(),
        Err(_) => return env.new_string("").expect("empty string"),
    };
    let selected_text_opt: Option<String> = env.get_string(&selected_text).ok().and_then(|s| {
        let s: String = s.into();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    });

    let pipeline = match get_handle(handle as u64) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Invalid handle: {e}");
            return env.new_string("").expect("empty string");
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

    // Run the async pipeline on the tokio runtime.
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let result = rt.block_on(pipeline.process_audio(&request, cancel));

    let json_str = match result {
        Ok(text) => serde_json::json!({ "success": true, "text": text }).to_string(),
        Err(e) => {
            tracing::error!("Pipeline processing failed: {e}");
            serde_json::json!({ "success": false, "error": e.to_string() }).to_string()
        }
    };

    env.new_string(json_str)
        .expect("Failed to create result string")
}

/// Destroy a pipeline handle, releasing the associated resources.
///
/// # JNI Signature
/// `(J)V` — takes handle (long), returns void.
#[no_mangle]
pub extern "system" fn Java_com_tingyuxuan_core_NativeCore_destroyPipeline(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    let _span = tracing::info_span!("jni_destroy_pipeline", handle).entered();

    if remove_handle(handle as u64) {
        tracing::info!(handle, "Pipeline destroyed");
    } else {
        tracing::warn!(handle, "Attempted to destroy invalid pipeline handle");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::handle::{get_handle, remove_handle};

    #[test]
    fn test_handle_register_get_remove() {
        // We can't easily create a real Pipeline in tests without API keys,
        // so we test the handle table mechanics via the get/remove interface.
        // Note: handle module tests cover the core logic.
        assert!(get_handle(0).is_err());
        assert!(!remove_handle(0));
    }
}
