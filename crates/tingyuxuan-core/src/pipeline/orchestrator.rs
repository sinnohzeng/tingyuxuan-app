use std::time::Instant;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::audio::encoder::{AudioBuffer, AudioFormat};
use crate::context::InputContext;
use crate::error::{LLMError, PipelineError};
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

    async fn invoke_llm(
        &self,
        input: &ProcessingInput,
        cancel_token: &CancellationToken,
    ) -> Result<crate::llm::provider::LLMResult, PipelineError> {
        let llm = &self.llm;
        tokio::select! {
            result = execute_with_retry(&self.retry_policy, cancel_token, || async {
                llm.process(input).await
            }) => result.map_err(PipelineError::Llm),
            _ = cancel_token.cancelled() => Err(PipelineError::Cancelled),
        }
    }

    fn emit_error_from_pipeline_error(&self, error: &PipelineError) {
        self.emit(PipelineEvent::Error {
            message: error.to_string(),
            user_action: error.user_action(),
        });
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

            // 1. 编码音频（优先 MP3，高压缩；失败回退 WAV）。
            let encode_start = Instant::now();
            let raw_pcm_bytes = buffer.len().saturating_mul(2);
            let encoded = {
                let _encode_span = tracing::info_span!("audio_encode").entered();
                let result = match buffer.encode(AudioFormat::Mp3) {
                    Ok(mp3) => mp3,
                    Err(mp3_err) => {
                        tracing::warn!(%mp3_err, "MP3 encode failed, falling back to WAV");
                        buffer
                            .encode(AudioFormat::Wav)
                            .map_err(PipelineError::Audio)?
                    }
                };
                let encode_ms = encode_start.elapsed().as_millis() as u64;
                let compression_ratio = if raw_pcm_bytes == 0 {
                    1.0_f64
                } else {
                    result.data.len() as f64 / raw_pcm_bytes as f64
                };
                tracing::info!(
                    encode_ms,
                    duration_ms = result.duration_ms,
                    encoded_bytes = result.data.len(),
                    format = result.format_str(),
                    raw_pcm_bytes,
                    compression_ratio,
                    "Audio encoded"
                );
                result
            };
            let primary_was_mp3 = matches!(encoded.format, AudioFormat::Mp3);

            // 2. 构建处理输入。
            let input = ProcessingInput {
                mode: request.mode.clone(),
                audio: encoded,
                context: request.context.clone(),
                target_language: request.target_language.clone(),
                user_dictionary: request.user_dictionary.clone(),
            };

            // 3. 发送处理开始事件。
            self.emit(PipelineEvent::ThinkingStarted);

            // 4. 调用 LLM（带重试、取消支持和计时；MP3 不兼容时自动回退 WAV）。
            let llm_start = Instant::now();
            let mut llm_result = match self.invoke_llm(&input, &cancel_token).await {
                Ok(r) => r,
                Err(PipelineError::Llm(llm_err))
                    if primary_was_mp3 && should_retry_with_wav(&llm_err) =>
                {
                    tracing::warn!(
                        %llm_err,
                        "Server rejected MP3 input, retrying once with WAV"
                    );
                    let fallback_audio = buffer
                        .encode(AudioFormat::Wav)
                        .map_err(PipelineError::Audio)?;
                    let fallback_input = ProcessingInput {
                        mode: request.mode.clone(),
                        audio: fallback_audio,
                        context: request.context.clone(),
                        target_language: request.target_language.clone(),
                        user_dictionary: request.user_dictionary.clone(),
                    };
                    match self.invoke_llm(&fallback_input, &cancel_token).await {
                        Ok(r) => r,
                        Err(error) => {
                            let llm_ms = llm_start.elapsed().as_millis() as u64;
                            tracing::error!(llm_ms, %error, "LLM failed (after WAV fallback)");
                            if !matches!(error, PipelineError::Cancelled) {
                                self.emit_error_from_pipeline_error(&error);
                            }
                            return Err(error);
                        }
                    }
                }
                Err(error) => {
                    let llm_ms = llm_start.elapsed().as_millis() as u64;
                    tracing::error!(llm_ms, %error, "LLM failed");
                    if !matches!(error, PipelineError::Cancelled) {
                        self.emit_error_from_pipeline_error(&error);
                    }
                    return Err(error);
                }
            };
            let llm_ms = llm_start.elapsed().as_millis() as u64;
            tracing::info!(llm_ms, tokens = ?llm_result.tokens_used, "LLM complete");

            // 5. 发送处理完成事件。
            let processed = llm_result.processed_text.trim().to_string();
            if let Err(invalid) = validate_transcript_quality(&processed) {
                let error = PipelineError::Llm(invalid);
                tracing::warn!(%error, "Transcript failed quality gate");
                self.emit_error_from_pipeline_error(&error);
                return Err(error);
            }
            llm_result.processed_text = processed.clone();
            self.emit(PipelineEvent::ProcessingComplete {
                processed_text: processed.clone(),
            });

            Ok(processed)
        }
        .instrument(span)
        .await
    }
}

fn should_retry_with_wav(error: &LLMError) -> bool {
    match error {
        LLMError::ServerError(status, body) if matches!(*status, 400 | 415 | 422) => {
            let body = body.to_lowercase();
            body.contains("audio")
                && (body.contains("format")
                    || body.contains("codec")
                    || body.contains("mp3")
                    || body.contains("unsupported"))
        }
        LLMError::InvalidResponse(msg) => {
            let msg = msg.to_lowercase();
            msg.contains("audio") && msg.contains("format")
        }
        _ => false,
    }
}

fn validate_transcript_quality(text: &str) -> Result<(), LLMError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(LLMError::InvalidResponse(
            "empty transcript from multimodal model".to_string(),
        ));
    }
    if trimmed.chars().count() <= 1 {
        return Err(LLMError::InvalidResponse(
            "transcript too short to be valid".to_string(),
        ));
    }

    // 防止“模板化占位文案”被当成真实转写结果。
    let compact = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(*c, '，' | '。' | ',' | '.'))
        .collect::<String>();
    let compact_lower = compact.to_lowercase();
    let placeholders = [
        "我需要将语音内容转换为书面文字请开始录音",
        "请开始录音",
        "请说话",
        "开始录音",
        "i need to convert speech to text",
    ];

    if placeholders.iter().any(|p| compact_lower.contains(p)) {
        return Err(LLMError::InvalidResponse(
            "placeholder transcript detected".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InputContext;
    use crate::error::LLMError;
    use crate::llm::provider::LLMResult;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    struct Mp3RejectedThenSuccessLLM {
        calls: AtomicUsize,
    }

    impl LLMProvider for Mp3RejectedThenSuccessLLM {
        fn name(&self) -> &str {
            "mp3-reject-then-success"
        }

        fn process<'a>(
            &'a self,
            input: &'a ProcessingInput,
        ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>> {
            let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if call_index == 0 && matches!(input.audio.format, AudioFormat::Mp3) {
                    return Err(LLMError::ServerError(
                        415,
                        "unsupported audio format: mp3".to_string(),
                    ));
                }
                Ok(LLMResult {
                    processed_text: "fallback success".to_string(),
                    tokens_used: Some(7),
                })
            })
        }

        fn test_connection(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>> {
            Box::pin(async { Ok(true) })
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
                PipelineEvent::ThinkingStarted => found_started = true,
                PipelineEvent::ProcessingComplete { .. } => found_complete = true,
                _ => {}
            }
        }
        assert!(found_started);
        assert!(found_complete);
    }

    #[tokio::test]
    async fn test_placeholder_transcript_rejected() {
        let (pipeline, mut rx) = make_pipeline(Box::new(MockLLM {
            response: "我需要将语音内容转换为书面文字。请开始录音。".to_string(),
        }));

        let token = CancellationToken::new();
        let result = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;

        assert!(matches!(
            result,
            Err(PipelineError::Llm(LLMError::InvalidResponse(_)))
        ));

        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, PipelineEvent::Error { .. }) {
                found_error = true;
            }
        }
        assert!(found_error);
    }

    #[tokio::test]
    async fn test_retry_with_wav_after_mp3_rejection() {
        let (pipeline, _rx) = make_pipeline(Box::new(Mp3RejectedThenSuccessLLM {
            calls: AtomicUsize::new(0),
        }));

        let token = CancellationToken::new();
        let result = pipeline
            .process_audio(sample_buffer(), &sample_request(), token)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fallback success");
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
