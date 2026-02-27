pub mod dashscope_asr;
pub mod provider;
pub mod whisper;

pub use provider::{STTOptions, STTProvider, STTResult};

use crate::config::{STTConfig, STTProviderType};
use crate::error::STTError;

/// Create an STT provider from the given configuration.
///
/// The `api_key` parameter should contain the resolved API key (not the key reference).
pub fn create_stt_provider(
    config: &STTConfig,
    api_key: String,
) -> Result<Box<dyn STTProvider>, STTError> {
    if api_key.is_empty() {
        return Err(STTError::NotConfigured);
    }

    match config.provider {
        STTProviderType::Whisper => Ok(Box::new(whisper::WhisperProvider::new(
            api_key,
            config.base_url.clone(),
            config.model.clone(),
        )?)),
        STTProviderType::DashScopeASR => Ok(Box::new(dashscope_asr::DashScopeASRProvider::new(
            api_key,
            config.base_url.clone(),
            config.model.clone(),
        )?)),
        STTProviderType::Custom => {
            // Custom providers are treated as Whisper-compatible (OpenAI API format).
            Ok(Box::new(whisper::WhisperProvider::new(
                api_key,
                config.base_url.clone(),
                config.model.clone(),
            )?))
        }
    }
}
