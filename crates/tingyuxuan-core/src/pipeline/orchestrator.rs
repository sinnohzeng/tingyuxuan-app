use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::context::InputContext;
use crate::error::PipelineError;
use crate::llm::provider::{LLMInput, LLMProvider, ProcessingMode};
use crate::pipeline::events::PipelineEvent;
use crate::pipeline::retry::{RetryPolicy, execute_with_retry};
use crate::stt::provider::STTOptions;
use crate::stt::streaming::{StreamingSTTProvider, StreamingSession};

/// 处理请求 — 不再包含 audio_path（流式架构，音频通过 channel 传递）。
#[derive(Debug, Clone)]
pub struct ProcessingRequest {
    pub mode: ProcessingMode,
    pub context: InputContext,
    pub target_language: Option<String>,
    pub user_dictionary: Vec<String>,
}

// ProcessingRequest 使用 struct literal 构造，不提供便捷方法。
// 所有字段均为 pub，调用者直接构造。

/// 管线编排器 — 协调流式 STT 和 LLM 处理。
///
/// 新架构：
/// - 录音开始时调用 `start_streaming()` 建立 STT WebSocket 连接
/// - 录音期间音频帧通过 channel 实时发送到 STT
/// - 录音结束后调用 `process_transcript()` 用 LLM 处理转写文本
pub struct Pipeline {
    stt: Box<dyn StreamingSTTProvider>,
    llm: Box<dyn LLMProvider>,
    event_tx: broadcast::Sender<PipelineEvent>,
    retry_policy: RetryPolicy,
}

impl Pipeline {
    /// 创建新的管线。
    pub fn new(
        stt: Box<dyn StreamingSTTProvider>,
        llm: Box<dyn LLMProvider>,
        event_tx: broadcast::Sender<PipelineEvent>,
    ) -> Self {
        Self {
            stt,
            llm,
            event_tx,
            retry_policy: RetryPolicy::default(),
        }
    }

    /// 订阅管线事件。
    pub fn subscribe(&self) -> broadcast::Receiver<PipelineEvent> {
        self.event_tx.subscribe()
    }

    /// 发送事件（尽力交付，无接收者时静默丢弃）。
    fn emit(&self, event: PipelineEvent) {
        if let Err(e) = self.event_tx.send(event) {
            tracing::debug!("Event dropped (no receivers): {:?}", e.0);
        }
    }

    /// 开始流式 STT 会话。
    ///
    /// 返回 [StreamingSession]，调用者通过 `audio_tx` 发送 PCM 帧，
    /// 通过 `event_rx` 接收转写事件。
    pub async fn start_streaming(
        &self,
        options: &STTOptions,
    ) -> Result<StreamingSession, PipelineError> {
        async {
            self.emit(PipelineEvent::TranscriptionStarted);

            let session = self.stt.start_stream(options).await.map_err(|e| {
                self.emit(PipelineEvent::Error {
                    message: e.to_string(),
                    user_action: e.user_action(),
                    raw_text: None,
                });
                PipelineError::Stt(e)
            })?;

            Ok(session)
        }
        .instrument(tracing::info_span!("stt_connect"))
        .await
    }

    /// 用 LLM 处理转写文本。
    ///
    /// 在流式 STT 完成后调用，将原始转写文本通过 LLM 润色/翻译/AI 处理。
    pub async fn process_transcript(
        &self,
        raw_text: String,
        request: &ProcessingRequest,
        cancel_token: CancellationToken,
    ) -> Result<String, PipelineError> {
        let span = tracing::info_span!("llm_process",
            mode = %request.mode,
            raw_len = raw_text.len(),
        );

        async {
            if cancel_token.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }

            self.emit(PipelineEvent::TranscriptionComplete {
                raw_text: raw_text.clone(),
            });

            self.emit(PipelineEvent::ProcessingStarted);

            let llm_input = LLMInput {
                mode: request.mode.clone(),
                raw_transcript: raw_text.clone(),
                target_language: request.target_language.clone(),
                context: request.context.clone(),
                user_dictionary: request.user_dictionary.clone(),
            };

            let llm_result = {
                let llm = &self.llm;
                let input = &llm_input;
                tokio::select! {
                    result = execute_with_retry(&self.retry_policy, &cancel_token, || async {
                        llm.process(input).await
                    }) => result,
                    _ = cancel_token.cancelled() => {
                        return Err(PipelineError::Cancelled);
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                        Err(crate::error::LLMError::Timeout)
                    }
                }
            };

            let llm_result = match llm_result {
                Ok(r) => r,
                Err(llm_err) => {
                    self.emit(PipelineEvent::Error {
                        message: llm_err.to_string(),
                        user_action: llm_err.user_action(),
                        raw_text: Some(raw_text),
                    });
                    return Err(PipelineError::Llm(llm_err));
                }
            };

            let processed = llm_result.processed_text;
            self.emit(PipelineEvent::ProcessingComplete {
                processed_text: processed.clone(),
            });

            Ok(processed)
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputContext;
    use crate::error::{LLMError, STTError};
    use crate::llm::provider::LLMResult;
    use crate::stt::streaming::{AudioChunk, STREAMING_CHANNEL_CAPACITY, StreamingSTTEvent};
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc;

    // -- Mock Streaming STT ---------------------------------------------------

    struct MockStreamingSTT;

    impl StreamingSTTProvider for MockStreamingSTT {
        fn name(&self) -> &str {
            "mock-streaming-stt"
        }
        fn start_stream<'a>(
            &'a self,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
            Box::pin(async {
                let (audio_tx, mut audio_rx) = mpsc::channel(STREAMING_CHANNEL_CAPACITY);
                let (event_tx, event_rx) = mpsc::channel(64);

                // 模拟：接收音频帧后返回最终结果
                tokio::spawn(async move {
                    let mut received = false;
                    while let Some(_chunk) = audio_rx.recv().await {
                        if !received {
                            let _ = event_tx
                                .send(StreamingSTTEvent::Partial {
                                    text: "你好".to_string(),
                                    sentence_index: 0,
                                })
                                .await;
                            received = true;
                        }
                    }
                    // audio_tx 关闭后发送最终结果
                    let _ = event_tx
                        .send(StreamingSTTEvent::Final {
                            text: "你好世界".to_string(),
                            sentence_index: 0,
                        })
                        .await;
                });

                Ok(StreamingSession { audio_tx, event_rx })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    struct FailingStreamingSTT;

    impl StreamingSTTProvider for FailingStreamingSTT {
        fn name(&self) -> &str {
            "failing-streaming-stt"
        }
        fn start_stream<'a>(
            &'a self,
            _options: &'a STTOptions,
        ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
            Box::pin(async { Err(STTError::AuthFailed) })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
            Box::pin(async { Ok(false) })
        }
    }

    // -- Mock LLM provider --------------------------------------------------

    struct MockLLM {
        response: String,
    }

    impl LLMProvider for MockLLM {
        fn name(&self) -> &str {
            "mock-llm"
        }
        fn process<'a>(
            &'a self,
            _input: &'a LLMInput,
        ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
            Box::pin(async move {
                Ok(LLMResult {
                    processed_text: self.response.clone(),
                    tokens_used: Some(42),
                })
            })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
        }
    }

    struct FailingLLM;

    impl LLMProvider for FailingLLM {
        fn name(&self) -> &str {
            "failing-llm"
        }
        fn process<'a>(
            &'a self,
            _input: &'a LLMInput,
        ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
            Box::pin(async { Err(LLMError::Timeout) })
        }
        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
            Box::pin(async { Ok(false) })
        }
    }

    // -- Helper -------------------------------------------------------------

    fn make_pipeline(
        stt: Box<dyn StreamingSTTProvider>,
        llm: Box<dyn LLMProvider>,
    ) -> (Pipeline, broadcast::Receiver<PipelineEvent>) {
        let (tx, rx) = broadcast::channel(32);
        let pipeline = Pipeline::new(stt, llm, tx);
        (pipeline, rx)
    }

    // -- Tests --------------------------------------------------------------

    #[tokio::test]
    async fn test_streaming_happy_path() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "你好，世界。".to_string(),
            }),
        );

        // 1. 开始流式会话
        let options = STTOptions {
            language: None,
            prompt: None,
        };
        let session = pipeline.start_streaming(&options).await.unwrap();

        // 2. 发送一帧音频
        session
            .audio_tx
            .send(AudioChunk {
                samples: vec![0i16; 320],
            })
            .await
            .unwrap();

        // 3. 关闭音频流（模拟录音停止）
        drop(session.audio_tx);

        // 4. 收集最终结果
        let mut event_rx = session.event_rx;
        let mut final_text = String::new();
        while let Some(event) = event_rx.recv().await {
            if let StreamingSTTEvent::Final { text, .. } = event {
                final_text = text;
            }
        }

        // 5. LLM 处理
        let request = ProcessingRequest {
            mode: ProcessingMode::Dictate,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        let result = pipeline
            .process_transcript(final_text, &request, token)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "你好，世界。");
    }

    #[tokio::test]
    async fn test_streaming_stt_failure() {
        let (pipeline, mut rx) = make_pipeline(
            Box::new(FailingStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let options = STTOptions {
            language: None,
            prompt: None,
        };
        let result = pipeline.start_streaming(&options).await;
        assert!(result.is_err());

        // 应该有 Error 事件
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, PipelineEvent::Error { .. }) {
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_llm_failure_includes_raw_text() {
        let (pipeline, mut rx) = make_pipeline(Box::new(MockStreamingSTT), Box::new(FailingLLM));

        let request = ProcessingRequest {
            mode: ProcessingMode::Dictate,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        let result = pipeline
            .process_transcript("raw transcript".to_string(), &request, token)
            .await;

        assert!(result.is_err());

        // Error 事件应包含 raw_text
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let PipelineEvent::Error { raw_text, .. } = event {
                assert_eq!(raw_text, Some("raw transcript".to_string()));
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_cancellation_before_llm() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "unused".to_string(),
            }),
        );

        let request = ProcessingRequest {
            mode: ProcessingMode::Dictate,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        token.cancel();

        let result = pipeline
            .process_transcript("hello".to_string(), &request, token)
            .await;
        assert!(matches!(result, Err(PipelineError::Cancelled)));
    }

    #[tokio::test]
    async fn test_processing_request_with_translate() {
        let (pipeline, _rx) = make_pipeline(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: "Hello, world.".to_string(),
            }),
        );

        let request = ProcessingRequest {
            mode: ProcessingMode::Translate,
            context: InputContext::default(),
            target_language: Some("en".to_string()),
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        let result = pipeline
            .process_transcript("你好世界".to_string(), &request, token)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world.");
    }

    #[test]
    fn test_subscribe() {
        let (tx, _) = broadcast::channel(16);
        let pipeline = Pipeline::new(
            Box::new(MockStreamingSTT),
            Box::new(MockLLM {
                response: String::new(),
            }),
            tx,
        );

        let _rx = pipeline.subscribe();
    }
}
