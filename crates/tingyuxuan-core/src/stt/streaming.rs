use std::future::Future;
use std::pin::Pin;

use tokio::sync::mpsc;

use crate::error::STTError;
use crate::stt::provider::STTOptions;

/// 音频帧：原始 PCM16 16kHz mono。
///
/// 帧大小约 20ms（320 samples @ 16kHz），对齐 Opus 编码要求。
/// 实际大小可变，STT provider 内部缓冲对齐。
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<i16>,
}

/// 流式 STT 事件。
#[derive(Debug, Clone)]
pub enum StreamingSTTEvent {
    /// 中间结果（会被后续结果覆盖）。
    Partial { text: String, sentence_index: u32 },
    /// 最终结果（该句不再变化）。
    Final { text: String, sentence_index: u32 },
    /// 错误。
    Error(STTError),
}

/// 流式会话句柄。
///
/// - `audio_tx`: 有界 channel（容量 50 帧 ≈ 1s @20ms/帧），满时旧帧丢弃。
/// - `event_rx`: 接收 STT 事件。
///
/// Drop audio_tx 会通知 provider 音频流结束，provider 应随后发送最终结果并关闭 event_rx。
pub struct StreamingSession {
    pub audio_tx: mpsc::Sender<AudioChunk>,
    pub event_rx: mpsc::Receiver<StreamingSTTEvent>,
}

/// 流式 STT 提供商。
///
/// 使用 `Pin<Box<dyn Future>>` 保持与现有 LLMProvider 的 object-safety 一致。
pub trait StreamingSTTProvider: Send + Sync {
    /// 返回 provider 名称。
    fn name(&self) -> &str;

    /// 开始流式会话。
    ///
    /// 返回 [StreamingSession]，调用者通过 `audio_tx` 发送 PCM 帧，
    /// 通过 `event_rx` 接收转写事件。
    fn start_stream<'a>(
        &'a self,
        options: &'a STTOptions,
    ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>>;

    /// 测试连接和凭证是否有效。
    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>>;
}

/// 流式 STT 的有界 channel 容量（帧数）。
///
/// 50 帧 × 20ms/帧 = 1 秒的缓冲。满时 `try_send` 失败，
/// 旧帧丢弃——语音 STT 容忍少量丢帧。
pub const STREAMING_CHANNEL_CAPACITY: usize = 50;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_chunk_creation() {
        let chunk = AudioChunk {
            samples: vec![0i16; 320],
        };
        assert_eq!(chunk.samples.len(), 320);
    }

    #[test]
    fn test_streaming_event_variants() {
        let partial = StreamingSTTEvent::Partial {
            text: "你好".to_string(),
            sentence_index: 0,
        };
        let final_event = StreamingSTTEvent::Final {
            text: "你好世界".to_string(),
            sentence_index: 0,
        };
        let error = StreamingSTTEvent::Error(STTError::Timeout(10));

        // 确保枚举可以构造
        assert!(matches!(partial, StreamingSTTEvent::Partial { .. }));
        assert!(matches!(final_event, StreamingSTTEvent::Final { .. }));
        assert!(matches!(error, StreamingSTTEvent::Error(_)));
    }

    #[test]
    fn test_channel_capacity_constant() {
        assert_eq!(STREAMING_CHANNEL_CAPACITY, 50);
    }

    #[tokio::test]
    async fn test_streaming_session_channel_basics() {
        let (audio_tx, mut audio_rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(32);

        let _session = StreamingSession {
            audio_tx: audio_tx.clone(),
            event_rx,
        };

        // 发送音频帧
        let chunk = AudioChunk {
            samples: vec![100i16; 320],
        };
        audio_tx.send(chunk).await.unwrap();

        let received = audio_rx.recv().await.unwrap();
        assert_eq!(received.samples.len(), 320);
        assert_eq!(received.samples[0], 100);

        // 发送事件
        event_tx
            .send(StreamingSTTEvent::Final {
                text: "测试".to_string(),
                sentence_index: 0,
            })
            .await
            .unwrap();
    }
}
