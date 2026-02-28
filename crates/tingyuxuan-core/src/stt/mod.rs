pub mod dashscope_streaming;
pub mod provider;
pub mod streaming;

pub use provider::STTOptions;
pub use streaming::{AudioChunk, StreamingSTTEvent, StreamingSTTProvider, StreamingSession};

use crate::config::{STTConfig, STTProviderType};
use crate::error::STTError;

/// 创建流式 STT provider。
///
/// MVP 仅支持 DashScope Paraformer 流式识别。
pub fn create_streaming_stt_provider(
    config: &STTConfig,
    api_key: String,
) -> Result<Box<dyn StreamingSTTProvider>, STTError> {
    if api_key.is_empty() {
        return Err(STTError::NotConfigured);
    }

    match config.provider {
        STTProviderType::DashScopeStreaming => {
            Ok(Box::new(dashscope_streaming::DashScopeStreamingProvider::new(
                api_key,
                config.base_url.clone(),
                config.model.clone(),
            )?))
        }
    }
}
