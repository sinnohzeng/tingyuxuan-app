# LLM 模块（大语言模型处理）

## 模块职责

LLM 模块负责将 STT 转录的原始文本通过大语言模型进行后处理，包括听写清理、翻译、AI 助手和文本编辑四种模式。通过 OpenAI 兼容的 Chat Completions API 支持多种后端（OpenAI、DashScope、Volcengine 等），并包含完整的 prompt 模板系统和上下文感知的语气检测。

---

## 核心类型定义

### LLMProvider trait

```rust
/// Trait that all LLM providers must implement.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Human-readable name of this provider (e.g. "OpenAI", "DashScope").
    fn name(&self) -> &str;

    /// Process the input through the LLM and return the result.
    async fn process(&self, input: &LLMInput) -> Result<LLMResult, LLMError>;

    /// Verify that the provider's credentials and endpoint are reachable.
    async fn test_connection(&self) -> Result<bool, LLMError>;
}
```

### ProcessingMode

```rust
/// The processing mode determines which LLM prompt template is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingMode {
    /// Clean up voice transcript into polished written text.
    Dictate,
    /// Translate the transcript into the target language.
    Translate,
    /// Use the transcript as a free-form AI assistant query.
    AiAssistant,
    /// Edit/refine already-selected text based on the voice instruction.
    Edit,
}
```

### LLMInput

```rust
/// Input to the LLM processing step.
#[derive(Debug, Clone)]
pub struct LLMInput {
    /// Which processing pipeline to use.
    pub mode: ProcessingMode,
    /// The raw transcript from STT.
    pub raw_transcript: String,
    /// Target language code for Translate mode (e.g. "en", "ja").
    pub target_language: Option<String>,
    /// Text currently selected in the user's application (for Edit mode).
    pub selected_text: Option<String>,
    /// Name of the currently focused application (for context hints).
    pub current_app: Option<String>,
    /// User-defined dictionary terms to improve recognition.
    pub user_dictionary: Vec<String>,
}
```

### LLMResult

```rust
/// Result returned from LLM processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResult {
    /// The processed / cleaned-up text.
    pub processed_text: String,
    /// Total tokens consumed (if the provider reports it).
    pub tokens_used: Option<u32>,
}
```

### OpenAICompatProvider

```rust
/// An LLM provider that speaks the OpenAI-compatible chat completions API.
///
/// Works with OpenAI, DashScope, Volcengine (Doubao), and any other provider
/// that implements the `/chat/completions` endpoint.
pub struct OpenAICompatProvider {
    /// Shared HTTP client with connection pooling and keep-alive.
    client: Client,
    /// Bearer token for the `Authorization` header.
    api_key: String,
    /// Base URL without a trailing slash, e.g. `https://api.openai.com/v1`.
    base_url: String,
    /// Model identifier, e.g. `gpt-4o-mini`, `qwen-turbo`.
    model: String,
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
    ServerError(u16, String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Provider not configured")]
    NotConfigured,
}
```

---

## 公开 API

### LLMProvider trait 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `name()` | `fn name(&self) -> &str` | 返回 provider 名称（`"OpenAI-compatible"`） |
| `process()` | `async fn process(&self, input: &LLMInput) -> Result<LLMResult, LLMError>` | 根据 ProcessingMode 构建 prompt 并调用 LLM |
| `test_connection()` | `async fn test_connection(&self) -> Result<bool, LLMError>` | 发送 `"Say hi."` 验证连接 |

### OpenAICompatProvider

| 方法 | 签名 | 说明 |
|------|------|------|
| `new()` | `fn new(api_key: String, base_url: String, model: String) -> Self` | 创建实例。timeout 15s，连接池 4 idle/host |

### Prompt 系统（`prompts.rs`）

| 函数 | 签名 | 说明 |
|------|------|------|
| `build_prompt()` | `fn build_prompt(mode: &ProcessingMode, input: &LLMInput) -> (String, String)` | 构建 (system_message, user_message) 对 |
| `format_dictionary_hint()` | `pub fn format_dictionary_hint(words: &[String]) -> String` | 将用户词典格式化为提示文本 |
| `format_context_hint()` | `pub fn format_context_hint(app_name: Option<&str>) -> String` | 将活跃应用名格式化为上下文提示 |

**内部辅助函数（非 pub）：**

| 函数 | 说明 |
|------|------|
| `build_dictate_prompt()` | Dictate 模式 -- 清理转录、去除填充词、自动标点、识别列表结构 |
| `build_translate_prompt()` | Translate 模式 -- 先整理后翻译，默认目标语言 `"en"` |
| `build_ai_assistant_prompt()` | AiAssistant 模式 -- 理解语音意图，给出简洁回答 |
| `build_edit_prompt()` | Edit 模式 -- 根据语音指令修改选中文本 |
| `detect_tone()` | 根据应用名检测语气风格 |
| `format_tone_hint()` | 将语气风格转为 LLM 提示文本 |

### 语气检测映射

| 应用类别 | 匹配关键词 | 语气 | 提示效果 |
|----------|-----------|------|---------|
| 即时通讯 | Slack, Discord, Telegram, WeChat/微信, DingTalk/钉钉, Teams | `casual` | 保持口语化和轻松的表达风格 |
| 邮件 | Mail, Outlook, Thunderbird, Foxmail | `formal` | 使用正式、专业的书面表达 |
| 开发工具 | Code, IntelliJ, Vim, Neovim, Terminal, iTerm, WezTerm, Alacritty, Emacs | `technical` | 保留技术术语和代码词汇的原始写法 |
| 笔记应用 | Notion, Obsidian, Logseq, Typora, Bear, Joplin | `structured` | 使用 Markdown 格式组织内容 |
| 其他 | -- | `neutral` | 无额外语气提示 |

---

## 错误处理策略

### HTTP 状态码映射

```rust
match status_code {
    401 => LLMError::AuthFailed,
    429 => LLMError::RateLimited,
    500..=599 => LLMError::ServerError(status_code, body_text),
    _ => LLMError::ServerError(status_code, body_text),
}
```

### 用户操作映射

```rust
impl LLMError {
    pub fn user_action(&self) -> UserAction {
        match self {
            LLMError::AuthFailed => UserAction::CheckApiKey,       // → 前往设置页
            LLMError::RateLimited => UserAction::WaitAndRetry,     // → 自动延迟重试
            _ => UserAction::InsertRawOrRetry,                     // → [插入原始转录] [重试处理]
        }
    }
}
```

### 关键设计

- **LLM 失败时的降级策略：** 当 LLM 处理失败但 STT 已成功时，Pipeline 会在 Error 事件中携带 `raw_text`，用户可以选择直接插入未经处理的转录文本
- **Temperature 固定 0.3：** 偏低的温度值确保输出稳定且接近确定性
- **Bearer token 认证：** 使用 `reqwest` 的 `.bearer_auth()` 方法注入 Authorization header
- **连接池复用：** `pool_max_idle_per_host(4)` 保持 4 个空闲连接，减少 TLS 握手开销

---

## 测试覆盖

共 **约 20 个** 单元测试（14 个 prompt 测试 + trait 编译验证 + 语气检测测试）：

### Prompt 测试（prompts.rs）

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_dictate_prompt_basic` | Dictate 模式基本 prompt 结构和内容 |
| `test_dictate_prompt_with_dictionary` | 用户词典注入 |
| `test_dictate_prompt_with_context` | 活跃应用上下文注入 |
| `test_dictate_prompt_with_list_rules` | 有序/无序列表规则包含 |
| `test_dictate_prompt_with_tone` | 语气提示注入（Slack -> 口语化） |
| `test_translate_prompt` | Translate 模式 + 指定目标语言 |
| `test_translate_prompt_default_language` | Translate 模式默认语言 "en" |
| `test_ai_assistant_prompt` | AiAssistant 模式 prompt 结构 |
| `test_edit_prompt` | Edit 模式包含选中文本和语音指令 |
| `test_edit_prompt_no_selected_text` | Edit 模式无选中文本时的处理 |
| `test_format_dictionary_hint_empty` | 空词典返回空字符串 |
| `test_format_dictionary_hint_with_words` | 多词汇以顿号连接 |
| `test_format_context_hint_none` / `_empty` / `_with_app` | 上下文提示的各种边界情况 |

### 语气检测测试

| 测试名称 | 覆盖场景 |
|----------|----------|
| `test_detect_tone_chat` | Slack, Discord, 微信, DingTalk -> casual |
| `test_detect_tone_email` | Outlook, Thunderbird, Foxmail -> formal |
| `test_detect_tone_dev` | VS Code, IntelliJ, Vim, Alacritty -> technical |
| `test_detect_tone_notes` | Notion, Obsidian, Logseq -> structured |
| `test_detect_tone_unknown` | Firefox, Random App -> neutral |
| `test_format_tone_hint_none` / `_chat` / `_formal` | 语气提示文本生成 |

**尚未覆盖：**
- wiremock 集成测试（HTTP 请求/响应模拟）
- Token 用量解析验证
- 并发调用场景

---

## 已知限制

1. **无流式输出** -- 不支持 SSE / streaming，LLM 响应为一次性返回，对长文本处理有延迟体验问题
2. **固定温度值** -- Temperature 硬编码为 `0.3`，无法按模式或用户偏好调整
3. **无 Prompt 版本管理** -- Prompt 模板硬编码在源码中，无法进行 A/B 测试或动态更新
4. **无 wiremock 集成测试** -- Provider 的 HTTP 交互逻辑未通过 mock server 覆盖
5. **单一 Provider 类型** -- 当前只有 `OpenAICompatProvider`，所有后端都假设支持 OpenAI Chat Completions 格式
6. **语气检测为简单字符串匹配** -- 基于应用名称的 `contains` 检查，可能误匹配（如应用名恰好包含 "code" 的非开发工具）
7. **无 token 用量限制** -- 不检查或限制单次请求的 token 消耗
