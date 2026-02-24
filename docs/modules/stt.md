# STT 模块（语音转文字）

## 模块职责

STT 模块负责将音频文件转录为文字。通过 trait 抽象支持多种语音识别后端（OpenAI Whisper、阿里云 DashScope Qwen-ASR、自定义 Whisper 兼容服务），并提供统一的工厂函数按配置创建对应的 provider 实例。

---

## 核心类型定义

### STTProvider trait

```rust
/// Trait for speech-to-text providers.
#[async_trait]
pub trait STTProvider: Send + Sync {
    /// Returns the name of this provider.
    fn name(&self) -> &str;

    /// Transcribe the audio file at the given path.
    async fn transcribe(
        &self,
        audio_path: &Path,
        options: &STTOptions,
    ) -> Result<STTResult, STTError>;

    /// Test that the provider connection and credentials are valid.
    async fn test_connection(&self) -> Result<bool, STTError>;
}
```

### STTOptions

```rust
/// Options for speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTOptions {
    /// Language code: "auto", "en", "zh", etc.
    pub language: Option<String>,
    /// Vocabulary hints to improve recognition accuracy.
    pub prompt: Option<String>,
}
```

### STTResult

```rust
/// Result of a speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTResult {
    /// The transcribed text.
    pub text: String,
    /// Detected or specified language code.
    pub language: String,
    /// Duration of the audio in seconds.
    pub duration_seconds: f64,
}
```

### STTError

```rust
#[derive(Error, Debug)]
pub enum STTError {
    #[error("Network timeout (>{0}s)")]
    Timeout(u64),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed (HTTP 401): check your API key")]
    AuthFailed,
    #[error("Rate limited (HTTP 429): try again later")]
    RateLimited,
    #[error("Server error (HTTP {0}): {1}")]
    ServerError(u16, String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Provider not configured")]
    NotConfigured,
    #[error("Unsupported audio format")]
    UnsupportedFormat,
}
```

### WhisperProvider

```rust
/// OpenAI Whisper API compatible speech-to-text provider.
pub struct WhisperProvider {
    client: Client,
    api_key: String,
    base_url: String,  // 默认: "https://api.openai.com/v1"
    model: String,      // 默认: "whisper-1"
}
```

### DashScopeASRProvider

```rust
/// Alibaba Cloud DashScope Qwen-ASR speech-to-text provider.
pub struct DashScopeASRProvider {
    client: Client,
    api_key: String,
    base_url: String,  // 默认: "https://dashscope.aliyuncs.com/compatible-mode/v1"
    model: String,      // 默认: "qwen2-audio-instruct"
}
```

---

## 公开 API

### Provider trait 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `name()` | `fn name(&self) -> &str` | 返回 provider 名称标识（`"whisper"` 或 `"dashscope_asr"`） |
| `transcribe()` | `async fn transcribe(&self, audio_path: &Path, options: &STTOptions) -> Result<STTResult, STTError>` | 转录音频文件 |
| `test_connection()` | `async fn test_connection(&self) -> Result<bool, STTError>` | 验证 API 密钥和连接可用性 |

### WhisperProvider

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self` | 创建实例，timeout 15s |
| `transcribe()` | 见 trait | Multipart upload 到 `/audio/transcriptions`，支持 language 和 prompt（vocabulary hints）参数 |
| `test_connection()` | 见 trait | 通过 `GET /models` 端点验证 |

### DashScopeASRProvider

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self` | 创建实例，timeout 15s |
| `transcribe()` | 见 trait | 使用 Chat Completions API，音频以 base64 编码嵌入 `input_audio` content part |
| `test_connection()` | 见 trait | 发送简单文本 chat completion（`max_tokens: 1`）验证 |

### 工厂函数

```rust
/// Create an STT provider from the given configuration.
///
/// The `api_key` parameter should contain the resolved API key (not the key reference).
pub fn create_stt_provider(
    config: &STTConfig,
    api_key: String,
) -> Result<Box<dyn STTProvider>, STTError>
```

**Provider 类型映射：**

| `STTProviderType` | 实际实现 | 说明 |
|-------------------|----------|------|
| `Whisper` | `WhisperProvider` | OpenAI Whisper API |
| `DashScopeASR` | `DashScopeASRProvider` | 阿里云 DashScope Qwen-ASR |
| `Custom` | `WhisperProvider` | Whisper 兼容的自定义服务，使用自定义 `base_url` |

---

## 错误处理策略

### HTTP 状态码映射

Whisper 和 DashScope provider 使用相同的映射逻辑：

```rust
fn map_http_error(status: reqwest::StatusCode, body: &str) -> STTError {
    match status.as_u16() {
        401 => STTError::AuthFailed,
        429 => STTError::RateLimited,
        code if code >= 500 => STTError::ServerError(code, body.to_string()),
        code => STTError::ServerError(code, body.to_string()),
    }
}
```

### 用户操作映射

```rust
impl STTError {
    pub fn user_action(&self) -> UserAction {
        match self {
            STTError::AuthFailed => UserAction::CheckApiKey,      // → 前往设置页
            STTError::RateLimited => UserAction::WaitAndRetry,    // → 自动延迟重试
            _ => UserAction::RetryOrQueue,                        // → [重试] [稍后处理]
        }
    }
}
```

### 具体错误场景

- **超时：** `reqwest` 的 `is_timeout()` 检测 -> `STTError::Timeout(15)`
- **网络错误：** 非超时的 reqwest 错误 -> `STTError::NetworkError(msg)`
- **文件读取失败：** `tokio::fs::read` 失败 -> `STTError::NetworkError(msg)`（复用 NetworkError 包装 IO 错误）
- **JSON 解析失败：** `serde_json` 反序列化失败 -> `STTError::InvalidResponse(msg)`
- **响应体读取失败：** response body 读取失败 -> `STTError::InvalidResponse(msg)`
- **API key 为空：** 工厂函数检查 -> `STTError::NotConfigured`
- **DashScope 无 choices：** 响应中 choices 数组为空 -> `STTError::InvalidResponse("No choices in response")`

---

## 测试覆盖

当前测试为 trait 级别和响应解析测试。由于 `WhisperProvider` 和 `DashScopeASRProvider` 的核心逻辑依赖外部 HTTP 调用，目前没有通过 wiremock 进行集成测试。

**已有测试：**
- STTProvider trait 定义的编译期验证（Send + Sync bound）
- STTOptions / STTResult 的序列化与反序列化
- 工厂函数 `create_stt_provider` 在各 provider 类型下的构造验证
- 空 API key 返回 `NotConfigured` 错误

**计划中（Phase 4 Step 2）：**
- wiremock 模拟 Whisper API 的 multipart 上传和 JSON 响应
- wiremock 模拟 DashScope Chat Completions 请求和响应
- 各 HTTP 错误码的映射验证
- 超时场景模拟

---

## 已知限制

1. **无语言自动检测报告** -- Whisper API 基础响应（`response_format: json`）不返回检测到的语言，`STTResult.language` 直接使用用户传入的值或 `"auto"`
2. **DashScope 无音频时长提取** -- DashScope Chat Completions 响应不包含音频时长信息，`duration_seconds` 固定为 `0.0`
3. **Whisper 无音频时长** -- Whisper 基础 JSON 响应也不包含时长，`duration_seconds` 固定为 `0.0`
4. **无 wiremock 集成测试** -- Provider 的 HTTP 交互逻辑尚未通过 mock server 覆盖
5. **DashScope 使用 Chat Completions 协议** -- 作为 ASR 使用时将完整音频 base64 编码放入请求体，对大音频文件有较高内存占用
6. **单一超时配置** -- 所有请求统一 15 秒超时，无法针对大文件或慢网络单独配置
7. **无流式转录** -- 所有 provider 均为一次性上传完整音频文件，不支持流式或分片转录
