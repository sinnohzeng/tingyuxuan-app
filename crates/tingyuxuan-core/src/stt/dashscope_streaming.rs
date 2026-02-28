//! DashScope Paraformer 实时流式语音识别 — WebSocket 实现。
//!
//! # 协议
//!
//! 使用 DashScope 实时语音识别 WebSocket API：
//! 1. 建立 WebSocket 连接（Bearer token 认证）
//! 2. 发送 StartTranscription JSON 消息
//! 3. 发送 PCM16 音频帧（二进制消息）
//! 4. 接收 TranscriptionResultChanged / SentenceEnd JSON 事件
//! 5. audio_tx 关闭时发送 StopTranscription
//! 6. 接收 TranscriptionCompleted 后关闭连接
//!
//! # 音频格式
//!
//! 当前使用 PCM16（16kHz mono），后续可选 Opus 编码以降低带宽。

use std::future::Future;
use std::pin::Pin;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::http;

use crate::api_key::ApiKey;
use crate::error::STTError;
use crate::stt::provider::STTOptions;
use crate::stt::streaming::{
    AudioChunk, STREAMING_CHANNEL_CAPACITY, StreamingSTTEvent, StreamingSTTProvider,
    StreamingSession,
};

/// DashScope 实时语音识别默认 WebSocket 端点。
const DEFAULT_WS_URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference/";

/// 默认模型：Paraformer 实时版。
const DEFAULT_MODEL: &str = "paraformer-realtime-v2";

/// DashScope Paraformer 流式 STT 提供商。
pub struct DashScopeStreamingProvider {
    api_key: ApiKey,
    ws_url: String,
    model: String,
}

impl DashScopeStreamingProvider {
    /// 创建新的 DashScope 流式 STT provider。
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        model: Option<String>,
    ) -> Result<Self, STTError> {
        if api_key.is_empty() {
            return Err(STTError::NotConfigured);
        }

        Ok(Self {
            api_key: ApiKey::new(api_key),
            ws_url: base_url.unwrap_or_else(|| DEFAULT_WS_URL.to_string()),
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        })
    }

    /// 构建 StartTranscription 消息。
    fn build_start_message(&self, task_id: &str) -> String {
        serde_json::json!({
            "header": {
                "message_id": uuid::Uuid::new_v4().to_string(),
                "task_id": task_id,
                "namespace": "SpeechTranscriber",
                "name": "StartTranscription",
                "appkey": self.api_key.expose_secret(),
            },
            "payload": {
                "model": self.model,
                "format": "pcm",
                "sample_rate": 16000,
                "enable_intermediate_result": true,
                "enable_punctuation_prediction": true,
                "enable_inverse_text_normalization": true,
            }
        })
        .to_string()
    }

    /// 构建 StopTranscription 消息。
    fn build_stop_message(&self, task_id: &str) -> String {
        serde_json::json!({
            "header": {
                "message_id": uuid::Uuid::new_v4().to_string(),
                "task_id": task_id,
                "namespace": "SpeechTranscriber",
                "name": "StopTranscription",
                "appkey": self.api_key.expose_secret(),
            }
        })
        .to_string()
    }
}

impl StreamingSTTProvider for DashScopeStreamingProvider {
    fn name(&self) -> &str {
        "dashscope-streaming"
    }

    fn start_stream<'a>(
        &'a self,
        _options: &'a STTOptions,
    ) -> Pin<Box<dyn Future<Output = Result<StreamingSession, STTError>> + Send + 'a>> {
        Box::pin(async move {
            let task_id = uuid::Uuid::new_v4().to_string();
            let _span = tracing::info_span!("stt_ws",
                task_id = %task_id,
                model = %self.model,
            )
            .entered();

            // 建立 WebSocket 连接（携带 Bearer token）
            let request = build_ws_request(&self.ws_url, self.api_key.expose_secret())?;
            let (ws_stream, _response) =
                tokio_tungstenite::connect_async_tls_with_config(request, None, false, None)
                    .await
                    .map_err(|e| {
                        tracing::error!(error = %e, "WebSocket connection failed");
                        STTError::NetworkError(format!("WebSocket connection failed: {e}"))
                    })?;

            tracing::debug!("WebSocket connected");
            let (mut ws_sink, mut ws_source) = ws_stream.split();

            // 发送 StartTranscription
            let start_msg = self.build_start_message(&task_id);
            ws_sink
                .send(Message::Text(start_msg.into()))
                .await
                .map_err(|e| {
                    STTError::NetworkError(format!("Failed to send start message: {e}"))
                })?;

            // 等待 TranscriptionStarted 确认
            wait_for_started(&mut ws_source).await?;
            tracing::debug!("STT transcription started");

            // 创建 channel
            let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(STREAMING_CHANNEL_CAPACITY);
            let (event_tx, event_rx) = mpsc::channel::<StreamingSTTEvent>(64);

            let stop_msg = self.build_stop_message(&task_id);

            // 启动后台 task：桥接 audio chunks → WebSocket，接收 STT 事件
            tokio::spawn(async move {
                // 双向桥接循环
                loop {
                    tokio::select! {
                        // 从音频 channel 读取帧，发送到 WebSocket
                        chunk = audio_rx.recv() => {
                            match chunk {
                                Some(chunk) => {
                                    // PCM16 → 字节序列（little-endian）
                                    let bytes = pcm_to_bytes(&chunk.samples);
                                    if ws_sink.send(Message::Binary(bytes.into())).await.is_err() {
                                        tracing::error!("Failed to send audio frame over WebSocket");
                                        let _ = event_tx.send(StreamingSTTEvent::Error(
                                            STTError::NetworkError("WebSocket connection closed".to_string()),
                                        )).await;
                                        break;
                                    }
                                }
                                None => {
                                    // audio_tx 已关闭（录音停止），发送 StopTranscription
                                    tracing::debug!("Audio stream ended, sending StopTranscription");
                                    let _ = ws_sink.send(Message::Text(stop_msg.into())).await;
                                    // 继续接收最终结果，不 break
                                    // 切换到只读模式
                                    drain_ws_events(&mut ws_source, &event_tx).await;
                                    break;
                                }
                            }
                        }
                        // 从 WebSocket 读取 STT 事件
                        msg = ws_source.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    match parse_ws_message(&text) {
                                        ParsedMessage::SttEvent(event) => {
                                            if matches!(event, StreamingSTTEvent::Error(_)) {
                                                let _ = event_tx.send(event).await;
                                                break;
                                            }
                                            let _ = event_tx.send(event).await;
                                        }
                                        ParsedMessage::Completed => break,
                                        ParsedMessage::Ignored => {}
                                    }
                                }
                                Some(Ok(Message::Close(_))) | None => {
                                    tracing::debug!("WebSocket connection closed");
                                    break;
                                }
                                Some(Err(e)) => {
                                    tracing::error!("WebSocket receive error: {e}");
                                    let _ = event_tx.send(StreamingSTTEvent::Error(
                                        STTError::NetworkError(format!("WebSocket error: {e}")),
                                    )).await;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });

            Ok(StreamingSession { audio_tx, event_rx })
        })
    }

    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<bool, STTError>> + Send + '_>> {
        Box::pin(async move {
            // 尝试建立 WebSocket 连接来验证凭证
            let request = build_ws_request(&self.ws_url, self.api_key.expose_secret())?;
            match tokio_tungstenite::connect_async_tls_with_config(request, None, false, None).await
            {
                Ok((ws_stream, _)) => {
                    // 连接成功，立即关闭
                    let (mut sink, _) = ws_stream.split();
                    let _ = sink.close().await;
                    Ok(true)
                }
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                    let code = response.status().as_u16() as u32;
                    match code {
                        401 => Err(STTError::AuthFailed),
                        429 => Err(STTError::RateLimited),
                        _ => Err(STTError::ServerError(code, format!("HTTP {code}"))),
                    }
                }
                Err(e) => Err(STTError::NetworkError(e.to_string())),
            }
        })
    }
}

/// 构建 WebSocket 请求（携带 Authorization header）。
fn build_ws_request(ws_url: &str, api_key: &str) -> Result<http::Request<()>, STTError> {
    http::Request::builder()
        .uri(ws_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Host", extract_host(ws_url))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .body(())
        .map_err(|e| STTError::NetworkError(format!("Failed to build WebSocket request: {e}")))
}

/// 从 URL 提取 host。
fn extract_host(url: &str) -> String {
    url.replace("wss://", "")
        .replace("ws://", "")
        .split('/')
        .next()
        .unwrap_or("dashscope.aliyuncs.com")
        .to_string()
}

/// 等待 TranscriptionStarted 确认。
async fn wait_for_started<S>(ws_source: &mut S) -> Result<(), STTError>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    // 等待最多 10 秒
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(msg) = ws_source.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        let name = json["header"]["name"].as_str().unwrap_or("");
                        match name {
                            "TranscriptionStarted" => return Ok(()),
                            "TaskFailed" => {
                                let message = json["header"]["status_text"]
                                    .as_str()
                                    .unwrap_or("Task failed");
                                let status = json["header"]["status"].as_u64().unwrap_or(0);
                                if status == 40_100_000 {
                                    return Err(STTError::AuthFailed);
                                }
                                return Err(STTError::ServerError(
                                    status as u32,
                                    message.to_string(),
                                ));
                            }
                            _ => continue,
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    return Err(STTError::NetworkError(
                        "WebSocket closed before transcription started".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(STTError::NetworkError(format!("WebSocket error: {e}")));
                }
                _ => continue,
            }
        }
        Err(STTError::NetworkError(
            "WebSocket stream ended unexpectedly".to_string(),
        ))
    })
    .await;

    match timeout {
        Ok(result) => result,
        Err(_) => Err(STTError::Timeout(10)),
    }
}

/// 解析 WebSocket 消息的分类结果。
enum ParsedMessage {
    SttEvent(StreamingSTTEvent),
    Completed,
    Ignored,
}

/// 单次 JSON 解析：同时判断事件类型和提取数据。
fn parse_ws_message(text: &str) -> ParsedMessage {
    let Some(json) = serde_json::from_str::<serde_json::Value>(text).ok() else {
        return ParsedMessage::Ignored;
    };
    let Some(name) = json["header"]["name"].as_str() else {
        return ParsedMessage::Ignored;
    };

    match name {
        "TranscriptionResultChanged" => {
            let result = json["payload"]["result"].as_str().unwrap_or("");
            let index = json["payload"]["index"].as_u64().unwrap_or(0) as u32;
            ParsedMessage::SttEvent(StreamingSTTEvent::Partial {
                text: result.to_string(),
                sentence_index: index,
            })
        }
        "SentenceEnd" => {
            let result = json["payload"]["result"].as_str().unwrap_or("");
            let index = json["payload"]["index"].as_u64().unwrap_or(0) as u32;
            ParsedMessage::SttEvent(StreamingSTTEvent::Final {
                text: result.to_string(),
                sentence_index: index,
            })
        }
        "TranscriptionCompleted" => ParsedMessage::Completed,
        "TaskFailed" => {
            let message = json["header"]["status_text"]
                .as_str()
                .unwrap_or("Unknown error");
            ParsedMessage::SttEvent(StreamingSTTEvent::Error(STTError::ServerError(
                0,
                message.to_string(),
            )))
        }
        _ => ParsedMessage::Ignored,
    }
}

/// 在音频流结束后，继续读取 WebSocket 事件直到连接关闭。
async fn drain_ws_events<S>(ws_source: &mut S, event_tx: &mpsc::Sender<StreamingSTTEvent>)
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    // 最多等 30 秒
    let timed_out = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while let Some(msg) = ws_source.next().await {
            match msg {
                Ok(Message::Text(text)) => match parse_ws_message(&text) {
                    ParsedMessage::SttEvent(event) => {
                        let _ = event_tx.send(event).await;
                    }
                    ParsedMessage::Completed => break,
                    ParsedMessage::Ignored => {}
                },
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    })
    .await;
    if timed_out.is_err() {
        tracing::warn!("drain_ws_events timed out after 30s");
    }
}

/// PCM16 samples → little-endian 字节序列。
fn pcm_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_to_bytes() {
        let samples = vec![0x0100i16, -1i16];
        let bytes = pcm_to_bytes(&samples);
        assert_eq!(bytes, vec![0x00, 0x01, 0xFF, 0xFF]);
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("wss://dashscope.aliyuncs.com/api-ws/v1/inference/"),
            "dashscope.aliyuncs.com"
        );
        assert_eq!(extract_host("ws://localhost:8080/ws"), "localhost:8080");
    }

    #[test]
    fn test_parse_ws_message_partial() {
        let json = r#"{"header":{"name":"TranscriptionResultChanged"},"payload":{"result":"你好","index":0}}"#;
        let result = parse_ws_message(json);
        assert!(matches!(
            result,
            ParsedMessage::SttEvent(StreamingSTTEvent::Partial {
                text,
                sentence_index: 0
            }) if text == "你好"
        ));
    }

    #[test]
    fn test_parse_ws_message_final() {
        let json = r#"{"header":{"name":"SentenceEnd"},"payload":{"result":"你好世界","index":1}}"#;
        let result = parse_ws_message(json);
        assert!(matches!(
            result,
            ParsedMessage::SttEvent(StreamingSTTEvent::Final {
                text,
                sentence_index: 1
            }) if text == "你好世界"
        ));
    }

    #[test]
    fn test_parse_ws_message_completed() {
        let json = r#"{"header":{"name":"TranscriptionCompleted"}}"#;
        assert!(matches!(parse_ws_message(json), ParsedMessage::Completed));
    }

    #[test]
    fn test_parse_ws_message_failed() {
        let json = r#"{"header":{"name":"TaskFailed","status_text":"Auth error"}}"#;
        let result = parse_ws_message(json);
        assert!(matches!(
            result,
            ParsedMessage::SttEvent(StreamingSTTEvent::Error(_))
        ));
    }

    #[test]
    fn test_new_provider_empty_key_fails() {
        let result = DashScopeStreamingProvider::new(String::new(), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_provider_with_defaults() {
        let provider = DashScopeStreamingProvider::new("test-key".to_string(), None, None).unwrap();
        assert_eq!(provider.name(), "dashscope-streaming");
        assert_eq!(provider.ws_url, DEFAULT_WS_URL);
        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_new_provider_with_custom_url() {
        let provider = DashScopeStreamingProvider::new(
            "key".to_string(),
            Some("wss://custom.endpoint/ws".to_string()),
            Some("custom-model".to_string()),
        )
        .unwrap();
        assert_eq!(provider.ws_url, "wss://custom.endpoint/ws");
        assert_eq!(provider.model, "custom-model");
    }

    #[test]
    fn test_build_start_message_format() {
        let provider = DashScopeStreamingProvider::new("test-key".to_string(), None, None).unwrap();
        let msg = provider.build_start_message("task-123");
        let json: serde_json::Value = serde_json::from_str(&msg).unwrap();

        assert_eq!(json["header"]["name"], "StartTranscription");
        assert_eq!(json["header"]["task_id"], "task-123");
        assert_eq!(json["header"]["namespace"], "SpeechTranscriber");
        assert_eq!(json["payload"]["format"], "pcm");
        assert_eq!(json["payload"]["sample_rate"], 16000);
        assert!(
            json["payload"]["enable_intermediate_result"]
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_build_stop_message_format() {
        let provider = DashScopeStreamingProvider::new("test-key".to_string(), None, None).unwrap();
        let msg = provider.build_stop_message("task-123");
        let json: serde_json::Value = serde_json::from_str(&msg).unwrap();

        assert_eq!(json["header"]["name"], "StopTranscription");
        assert_eq!(json["header"]["task_id"], "task-123");
    }
}
