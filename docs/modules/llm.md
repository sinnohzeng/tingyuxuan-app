# LLM 模块（多模态大语言模型处理）

## 模块职责

LLM 模块负责将编码后的音频数据和上下文信息通过多模态大语言模型一步完成语音识别与文本处理，
包括听写清理、翻译、AI 助手和文本编辑四种模式。通过 `LLMProvider` trait 抽象支持多种后端，
使用 OpenAI 兼容的 Chat Completions API（含多模态扩展）。核心实现 `MultimodalProvider`
发送音频 base64 + 上下文 system prompt，通过 SSE 流式响应解析获取最终文本。
连接测试已升级为**真实音频多模态探测**（非纯文本 ping）。

---

## 核心类型定义

### LLMProvider trait

```rust
/// Trait that all LLM providers must implement.
pub trait LLMProvider: Send + Sync {
    /// Human-readable name of this provider (e.g. "Multimodal").
    fn name(&self) -> &str;

    /// Process the input through the LLM and return the result.
    fn process<'a>(
        &'a self,
        input: &'a ProcessingInput,
    ) -> Pin<Box<dyn Future<Output = Result<LLMResult, LLMError>> + Send + 'a>>;

    /// Verify that the provider's credentials and endpoint are reachable.
    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<bool, LLMError>> + Send + '_>>;
}
```

### ProcessingMode

```rust
/// The processing mode determines which LLM prompt template is used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingMode {
    Dictate,      // 清理语音输入为书面文字
    Translate,    // 翻译为目标语言
    AiAssistant,  // 自由形式 AI 助手
    Edit,         // 根据语音指令修改选中文本
}
```

### ProcessingInput

```rust
/// 多模态处理输入 — 包含编码后的音频和上下文。
pub struct ProcessingInput {
    pub mode: ProcessingMode,
    pub audio: EncodedAudio,         // MP3/WAV base64 编码音频
    pub context: InputContext,        // 统一上下文模型
    pub target_language: Option<String>,
    pub user_dictionary: Vec<String>,
}
```

> **与旧版 `LLMInput` 的区别：** 旧版接收 `raw_transcript: String`（STT 已转录的文本），新版接收 `audio: EncodedAudio`（原始音频），由多模态 LLM 一步完成识别和处理。

### LLMResult

```rust
/// Result returned from LLM processing.
pub struct LLMResult {
    pub processed_text: String,
    pub tokens_used: Option<u32>,
}
```

### MultimodalProvider

```rust
/// 多模态 LLM provider — 发送音频+上下文，一步完成识别和润色。
pub struct MultimodalProvider {
    client: Client,       // reqwest，timeout 60s
    api_key: ApiKey,      // 安全 API Key 封装
    base_url: String,     // 如 "https://dashscope.aliyuncs.com/compatible-mode/v1"
    model: String,        // 如 "qwen3-omni-flash"
}
```

### LLMError

```rust
#[derive(Error, Debug)]
pub enum LLMError {
    #[error("Network timeout")]
    Timeout,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed (HTTP 401): check your API key")]
    AuthFailed,
    #[error("Rate limited (HTTP 429): try again later")]
    RateLimited,
    #[error("Server error (HTTP {0}): {1}")]
    ServerError(u32, String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Provider not configured")]
    NotConfigured,
    #[error("HTTP client error: {0}")]
    HttpClientError(String),
}
```

---

## 公开 API

### LLMProvider trait 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `name()` | `fn name(&self) -> &str` | 返回 provider 名称（`"Multimodal"`） |
| `process()` | `fn process(&self, input: &ProcessingInput) -> Future<Result<LLMResult, LLMError>>` | 将音频+上下文发送给多模态 LLM，SSE 流式解析返回处理后的文本 |
| `test_connection()` | `fn test_connection(&self) -> Future<Result<bool, LLMError>>` | 发送短音频多模态请求验证连接、认证与音频能力 |

### MultimodalProvider

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(api_key: String, base_url: String, model: String) -> Result<Self, LLMError>` | 创建实例。timeout 60s，连接池 4 idle/host |
| `test_multimodal_audio_connection()` | `async fn test_multimodal_audio_connection(&self) -> Result<bool, LLMError>` | 发送 20ms 静音音频探测模型真实多模态能力 |

### Prompt 系统（`prompts.rs`）

| 函数 | 签名 | 说明 |
|------|------|------|
| `build_multimodal_system_prompt()` | `fn build_multimodal_system_prompt(mode, context, dictionary, target_language) -> String` | 构建多模态 system prompt（指导 LLM 如何处理音频输入） |
| `format_dictionary_hint()` | `fn format_dictionary_hint(words: &[String]) -> String` | 将用户词典格式化为提示文本 |
| `format_rich_context()` | `fn format_rich_context(ctx: &InputContext) -> String` | 将应用/窗口/URL/选中文本等上下文格式化为结构化提示 |

### 语气检测映射

| 应用类别 | 匹配关键词 | 语气 | 提示效果 |
|----------|-----------|------|---------|
| 即时通讯 | Slack, Discord, Telegram, WeChat/微信, DingTalk/钉钉, Teams | `casual` | 保持口语化和轻松的表达风格 |
| 邮件 | Mail, Outlook, Thunderbird, Foxmail | `formal` | 使用正式、专业的书面表达 |
| 开发工具 | Code, IntelliJ, Vim, Neovim, Terminal, iTerm, WezTerm, Alacritty, Emacs | `technical` | 保留技术术语和代码词汇的原始写法 |
| 笔记应用 | Notion, Obsidian, Logseq, Typora, Bear, Joplin | `structured` | 使用 Markdown 格式组织内容 |
| 其他 | -- | `neutral` | 无额外语气提示 |

---

## 支持的 Provider

| Provider | 模型 | 说明 |
|----------|------|------|
| **阿里云 DashScope** | qwen3-omni-flash（推荐）、qwen-omni-turbo | 主力 provider，低延迟、高性价比 |
| **OpenAI** | gpt-4o-audio-preview | 备选 provider，支持音频输入的 GPT-4o |

> **重要：** 所有 provider 均要求 `stream=true`（Qwen-Omni 系列强制要求 SSE 流式响应）。

---

## SSE 流式响应解析

请求体中 `stream: true` + `stream_options: { include_usage: true }`，响应为 `text/event-stream` 格式：

```
data: {"choices":[{"delta":{"content":"你好"}}]}

data: {"choices":[{"delta":{"content":"，世界。"}}]}

data: {"choices":[{"delta":{}}],"usage":{"total_tokens":42}}

data: [DONE]
```

解析逻辑：逐行读取 `data:` 前缀的行，反序列化为 `SSEChunk`，拼接所有 `delta.content`，提取最终 `usage.total_tokens`。

---

## 错误处理策略

### HTTP 状态码映射

```rust
match status_code {
    401 => LLMError::AuthFailed,
    429 => LLMError::RateLimited,
    _ => LLMError::ServerError(status_code, body_text),
}
```

### 用户操作映射

```rust
impl LLMError {
    pub fn user_action(&self) -> UserAction {
        match self {
            LLMError::AuthFailed => UserAction::CheckApiKey,
            LLMError::RateLimited => UserAction::WaitAndRetry,
            _ => UserAction::RetryOrCancel,
        }
    }
}
```

### 关键设计

- **Temperature 固定 0.3：** 偏低的温度值确保输出稳定且接近确定性
- **60 秒超时：** 多模态请求含音频 base64，处理时间显著长于纯文本（旧版 15s），需更长超时
- **SSE 流式解析：** 非逐事件 streaming，而是全量接收后逐行解析（简化实现）
- **连接池复用：** `pool_max_idle_per_host(4)` 保持 4 个空闲连接，减少 TLS 握手开销
- **ApiKey 安全封装：** 通过 `ApiKey::new()` + `expose_secret()` 避免 API Key 意外日志泄露

---

## 测试覆盖

共 **约 27 个** 单元测试：

### 多模态 Provider 测试（`multimodal.rs`，7 个 wiremock 集成测试）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `process_success_sse` | SSE 流式响应完整解析，拼接文本和 token 用量 |
| `process_auth_failure` | 401 响应映射为 `AuthFailed` |
| `process_rate_limited` | 429 响应映射为 `RateLimited` |
| `process_server_error` | 500 响应映射为 `ServerError` |
| `process_empty_sse_response` | 空 SSE 流返回 `InvalidResponse` |
| `test_connection_success` | 多模态音频连接测试成功验证 |
| `test_request_body_contains_audio` | 请求体包含音频数据验证 |

### Prompt 测试（`prompts.rs`）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_dictate_prompt_basic` | Dictate 模式基本 prompt 结构 |
| `test_dictate_prompt_with_dictionary` | 用户词典注入 |
| `test_dictate_prompt_with_context` | 活跃应用上下文注入 |
| `test_translate_prompt` | Translate 模式 + 指定目标语言 |
| `test_ai_assistant_prompt` | AiAssistant 模式 prompt 结构 |
| `test_edit_prompt` | Edit 模式包含选中文本 |

### 语气检测和 Provider trait 测试

覆盖所有 5 种语气映射、ProcessingMode 的 Display/FromStr 往返一致性。

---

## 已知限制

1. **非真流式输出** -- SSE 响应为全量接收后逐行解析，前端暂不支持逐 token 显示
2. **固定温度值** -- Temperature 硬编码为 `0.3`，无法按模式或用户偏好调整
3. **无 Prompt 版本管理** -- Prompt 模板硬编码在源码中，无法进行 A/B 测试或动态更新
4. **语气检测为简单字符串匹配** -- 基于应用名称的 `contains` 检查，可能误匹配
5. **无 token 用量限制** -- 不检查或限制单次请求的 token 消耗
6. **音频 base64 内存开销** -- 300 秒录音的 base64 约 12.8 MB，全部在请求体中发送
