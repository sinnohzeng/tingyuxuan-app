use std::time::Instant;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::audio::encoder::{AudioBuffer, AudioFormat};
use crate::context::InputContext;
use crate::error::PipelineError;
use crate::llm::provider::{LLMProvider, ProcessingInput, ProcessingMode};
use crate::pipeline::events::PipelineEvent;
use crate::pipeline::retry::{RetryPolicy, execute_with_retry};

/// 处理请求 — 描述一次多模态处理的参数。
#[derive(Debug, Clone)]
pub struct ProcessingRequest {
    pub mode: ProcessingMode,
    pub context: InputContext,
    pub target_language: Option<String>,
    pub user_dictionary: Vec<String>,
}

/// 管线编排器 — 单步多模态处理（音频编码 → LLM → 文本）。
pub struct Pipeline {
    llm: Box<dyn LLMProvider>,
    event_tx: broadcast::Sender<PipelineEvent>,
    retry_policy: RetryPolicy,
}

impl Pipeline {
    /// 创建新的管线。
    pub fn new(llm: Box<dyn LLMProvider>, event_tx: broadcast::Sender<PipelineEvent>) -> Self {
        Self {
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

    /// 单步处理：编码音频 → 调用多模态 LLM → 返回最终文字。
    pub async fn process_audio(
        &self,
        buffer: AudioBuffer,
        request: &ProcessingRequest,
        cancel_token: CancellationToken,
    ) -> Result<String, PipelineError> {
        let span = tracing::info_span!("pipeline",
            mode = %request.mode,
            audio_samples = buffer.len(),
        );

        async {
            if cancel_token.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }

            // 1. 编码音频（带计时）。
            let encode_start = Instant::now();
            let encoded = {
                let _encode_span = tracing::info_span!("audio_encode").entered();
                let result = buffer
                    .encode(AudioFormat::Wav)
                    .map_err(PipelineError::Audio)?;
                let encode_ms = encode_start.elapsed().as_millis() as u64;
                tracing::info!(
                    encode_ms,
                    duration_ms = result.duration_ms,
                    encoded_bytes = result.data.len(),
                    format = result.format_str(),
                    "Audio encoded"
                );
                result
            };

            // 2. 构建处理输入。
            let input = ProcessingInput {
                mode: request.mode.clone(),
                audio: encoded,
                context: request.context.clone(),
                target_language: request.target_language.clone(),
                user_dictionary: request.user_dictionary.clone(),
            };

            // 3. 发送处理开始事件。
            self.emit(PipelineEvent::ProcessingStarted);

            // 4. 调用 LLM（带重试、取消支持和计时）。
            let llm_start = Instant::now();
            let llm_result = {
                let llm = &self.llm;
                let llm_input = &input;
                tokio::select! {
                    result = execute_with_retry(&self.retry_policy, &cancel_token, || async {
                        llm.process(llm_input).await
                    }) => result,
                    _ = cancel_token.cancelled() => {
                        return Err(PipelineError::Cancelled);
                    }
                }
            };
            let llm_ms = llm_start.elapsed().as_millis() as u64;

            let llm_result = match llm_result {
                Ok(r) => {
                    tracing::info!(llm_ms, tokens = ?r.tokens_used, "LLM complete");
                    r
                }
                Err(llm_err) => {
                    tracing::error!(llm_ms, %llm_err, "LLM failed");
                    self.emit(PipelineEvent::Error {
                        message: llm_err.to_string(),
                        user_action: llm_err.user_action(),
                    });
                    return Err(PipelineError::Llm(llm_err));
                }
            };

            // 5. 发送处理完成事件。
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
    use crate::error::LLMError;
    use crate::llm::provider::LLMResult;
    use std::future::Future;
    use std::pin::Pin;

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
            _input: &'a ProcessingInput,
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
            _input: &'a ProcessingInput,
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

    fn make_pipeline(llm: Box<dyn LLMProvider>) -> (Pipeline, broadcast::Receiver<PipelineEvent>) {
        let (tx, rx) = broadcast::channel(32);
        let pipeline = Pipeline::new(llm, tx);
        (pipeline, rx)
    }

    fn sample_buffer() -> AudioBuffer {
        let mut buf = AudioBuffer::new(16_000, 1);
        buf.push_samples(&vec![0i16; 320]);
        buf
    }

    fn sample_request() -> ProcessingRequest {
        ProcessingRequest {
            mode: ProcessingMode::Dictate,
            context: InputContext::default(),
            target_language: None,
            user_dictionary: Vec::new(),
        }
    }

    // -- Tests --------------------------------------------------------------

    #[tokio::test]
    async fn test_process_audio_success() {
        let (pipeline, _rx) = make_pipeline(Box::new(MockLLM {
            response: "你好，世界。".to_string(),
        }));

        let token = CancellationToken::new();
        let result = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "你好，世界。");
    }

    #[tokio::test]
    async fn test_process_audio_llm_failure() {
        let (pipeline, mut rx) = make_pipeline(Box::new(FailingLLM));

        let token = CancellationToken::new();
        let result = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;

        assert!(result.is_err());

        // 应该有 Error 事件。
        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, PipelineEvent::Error { .. }) {
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_cancellation_before_processing() {
        let (pipeline, _rx) = make_pipeline(Box::new(MockLLM {
            response: "unused".to_string(),
        }));

        let token = CancellationToken::new();
        token.cancel();

        let result = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;
        assert!(matches!(result, Err(PipelineError::Cancelled)));
    }

    #[tokio::test]
    async fn test_processing_events_emitted() {
        let (pipeline, mut rx) = make_pipeline(Box::new(MockLLM {
            response: "result".to_string(),
        }));

        let token = CancellationToken::new();
        let _ = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;

        let mut found_started = false;
        let mut found_complete = false;
        while let Ok(event) = rx.try_recv() {
            match event {
                PipelineEvent::ProcessingStarted => found_started = true,
                PipelineEvent::ProcessingComplete { .. } => found_complete = true,
                _ => {}
            }
        }
        assert!(found_started);
        assert!(found_complete);
    }

    #[tokio::test]
    async fn test_translate_mode() {
        let (pipeline, _rx) = make_pipeline(Box::new(MockLLM {
            response: "Hello, world.".to_string(),
        }));

        let request = ProcessingRequest {
            mode: ProcessingMode::Translate,
            context: InputContext::default(),
            target_language: Some("en".to_string()),
            user_dictionary: Vec::new(),
        };
        let token = CancellationToken::new();
        let result = pipeline
            .process_audio(sample_buffer(), &request, token)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world.");
    }

    #[test]
    fn test_subscribe() {
        let (tx, _) = broadcast::channel(16);
        let pipeline = Pipeline::new(
            Box::new(MockLLM {
                response: String::new(),
            }),
            tx,
        );

        let _rx = pipeline.subscribe();
    }
}
