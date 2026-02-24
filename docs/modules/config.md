# Configuration

## 模块职责

配置模块管理应用程序的所有设置项，提供 JSON 文件持久化、默认值生成、Provider 预设选择等功能。配置文件遵循 XDG 目录规范，存储在用户的 config 目录下。

**源文件:** `crates/tingyuxuan-core/src/config.rs`

---

## 关键类型定义

### AppConfig（顶层配置结构）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub shortcuts: ShortcutConfig,
    pub language: LanguageConfig,
    pub stt: STTConfig,
    pub llm: LLMConfig,
    pub cache: CacheConfig,
    #[serde(default)]
    pub user_dictionary: Vec<String>,
}
```

### GeneralConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `auto_launch` | `bool` | `true` | 开机自启动 |
| `sound_feedback` | `bool` | `true` | 操作音效反馈 |
| `floating_bar_position` | `FloatingBarPosition` | `BottomCenter` | 悬浮栏位置 |

### FloatingBarPosition

```rust
pub enum FloatingBarPosition {
    BottomCenter,    // serde: "bottom_center"
    FollowCursor,    // serde: "follow_cursor"
    Fixed,           // serde: "fixed"
}
```

### ShortcutConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `dictate` | `String` | `"ctrl+shift+d"` | 听写快捷键 |
| `translate` | `String` | `"ctrl+shift+t"` | 翻译快捷键 |
| `ai_assistant` | `String` | `"ctrl+shift+a"` | AI 助手快捷键 |
| `cancel` | `String` | `"escape"` | 取消快捷键 |

### LanguageConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `primary` | `String` | `"auto"` | 主要语言 / 自动检测 |
| `translation_target` | `String` | `"en"` | 翻译目标语言 |
| `variant` | `Option<String>` | `None` | 语言变体 |

### STTConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `provider` | `STTProviderType` | `Whisper` | STT 服务提供商 |
| `api_key_ref` | `String` | `""` | API Key 引用（`@keyref:` 前缀表示从 keyring 读取） |
| `base_url` | `Option<String>` | `None` | 自定义 API 地址（留空使用 provider 默认值） |
| `model` | `Option<String>` | `Some("whisper-1")` | 模型名称 |

**STTProviderType 枚举:** `Whisper` (`"whisper"`), `DashScopeASR` (`"dashscope_asr"`), `Custom` (`"custom"`)

### LLMConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `provider` | `LLMProviderType` | `OpenAI` | LLM 服务提供商 |
| `api_key_ref` | `String` | `""` | API Key 引用 |
| `base_url` | `Option<String>` | `None` | 自定义 API 地址 |
| `model` | `String` | `"gpt-4o-mini"` | 模型名称 |

**LLMProviderType 枚举:** `OpenAI` (`"openai"`), `DashScope` (`"dashscope"`), `Volcengine` (`"volcengine"`), `Custom` (`"custom"`)

### CacheConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `audio_retention_hours` | `u64` | `24` | 音频缓存保留时间（小时） |
| `failed_retention_days` | `u64` | `7` | 失败记录保留天数 |
| `max_cache_size_mb` | `u64` | `500` | 最大缓存大小（MB） |

### ProviderPreset

```rust
pub struct ProviderPreset {
    pub name: String,
    pub llm_base_url: String,
    pub llm_models: Vec<String>,
    pub stt_provider: STTProviderType,
    pub stt_base_url: Option<String>,
    pub stt_model: Option<String>,
}
```

---

## Public API

### AppConfig 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `default()` | `fn default() -> Self` | 返回带有所有默认值的配置实例 |
| `config_dir()` | `fn config_dir() -> Result<PathBuf, ConfigError>` | 返回配置目录路径 |
| `config_path()` | `fn config_path() -> Result<PathBuf, ConfigError>` | 返回配置文件完整路径 |
| `data_dir()` | `fn data_dir() -> Result<PathBuf, ConfigError>` | 返回数据目录路径 |
| `load()` | `fn load() -> Result<Self, ConfigError>` | 从文件加载配置。文件不存在时返回默认配置 |
| `save(&self)` | `fn save(&self) -> Result<(), ConfigError>` | 将配置保存为 JSON 文件。自动创建父目录 |
| `llm_base_url(&self)` | `fn llm_base_url(&self) -> String` | 获取 LLM provider 的 base URL（优先使用自定义值，否则返回 provider 默认值） |
| `stt_base_url(&self)` | `fn stt_base_url(&self) -> String` | 获取 STT provider 的 base URL |

### ProviderPreset 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `all()` | `fn all() -> Vec<ProviderPreset>` | 返回所有内置 provider 预设 |

### 内置 Provider 预设（3 个）

| 名称 | LLM Base URL | LLM 模型 | STT Provider |
|------|-------------|---------|--------------|
| 阿里云 DashScope | `https://dashscope.aliyuncs.com/compatible-mode/v1` | qwen-turbo, qwen-plus, qwen-max | DashScopeASR (qwen2-audio-instruct) |
| 火山引擎 (豆包) | `https://ark.cn-beijing.volces.com/api/v3` | doubao-1-5-pro-256k, doubao-1-5-lite-32k | Whisper |
| OpenAI | `https://api.openai.com/v1` | gpt-4o, gpt-4o-mini | Whisper (whisper-1) |

### 存储路径

使用 `directories` crate 的 `ProjectDirs::from("com", "tingyuxuan", "TingYuXuan")`：

| 用途 | Linux 路径 |
|------|-----------|
| 配置文件 | `~/.config/tingyuxuan/TingYuXuan/config.json` |
| 数据目录 | `~/.local/share/tingyuxuan/TingYuXuan/` |

---

## 错误处理策略

使用自定义错误类型 `ConfigError`（定义在 `error.rs`）：

```rust
pub enum ConfigError {
    IoError(#[from] std::io::Error),       // 文件读写失败
    JsonError(#[from] serde_json::Error),  // JSON 序列化/反序列化失败
    NoDirFound,                             // 无法确定 XDG 目录
}
```

- `load()` 在文件不存在时静默返回默认配置（`Ok(Self::default())`），文件存在但格式错误时返回 `JsonError`
- `save()` 使用 `create_dir_all` 确保目录存在，写入使用 `serde_json::to_string_pretty` 保证可读性
- `user_dictionary` 字段使用 `#[serde(default)]` 保证向后兼容性（旧配置文件中不存在此字段时反序列化为空 Vec）

---

## 测试覆盖

共 **12 个单元测试**，位于 `config.rs` 的 `#[cfg(test)] mod tests`：

| 测试 | 覆盖内容 |
|------|---------|
| `test_default_config` | 验证默认值正确性（model、shortcut、cache 参数） |
| `test_config_serialization` | JSON 序列化/反序列化往返一致性 |
| `test_config_save_load` | 使用 `tempfile` 写入后重新读取，验证持久化正确性 |
| `test_llm_base_url_defaults` | 验证不同 LLM provider 返回正确的默认 base URL |
| `test_config_backward_compat_no_dictionary` | 旧格式 JSON（无 `user_dictionary` 字段）能正常反序列化 |
| `test_config_with_dictionary` | 包含 `user_dictionary` 的配置能正确序列化/反序列化 |
| `test_provider_presets` | 验证预设数量（3 个）和名称正确性 |
| `test_default_config_version` | 默认配置版本号为 CURRENT_CONFIG_VERSION |
| `test_old_config_deserializes_with_version_zero` | 无 config_version 字段的旧 JSON 反序列化为 version=0 |
| `test_serialization_includes_version` | 序列化输出包含 config_version 字段 |
| `test_migration_v0_to_v1` | v0→v1 迁移正确设置版本号且保留所有原始值 |
| `test_config_version_roundtrip` | 版本号序列化/反序列化往返一致 |

---

## 配置版本管理（Phase 4 Step 5）

### 机制

- `config_version` 字段（`#[serde(default)]`）：旧配置文件无此字段时默认为 0
- `load_with_migration()` 方法：检测版本 → 逐版本迁移 → 备份旧配置 → 保存
- 备份文件命名：`config.v0.json.bak`
- 当前版本：1

### 迁移链

| 迁移 | 说明 |
|------|------|
| v0 → v1 | 基线迁移：设置 `config_version = 1`。新字段由 `#[serde(default)]` 处理 |

后续版本升级时只需添加 `migrate_vN_to_vN+1()` 函数并更新 `CURRENT_CONFIG_VERSION` 常量。

---

## 已知局限性

1. ~~**无配置版本管理**~~ -- **已修复 (Phase 4 Step 5)**：添加 `config_version` 字段和增量迁移框架
2. ~~**无迁移框架**~~ -- **已修复 (Phase 4 Step 5)**：`load_with_migration()` 提供增量迁移和自动备份
3. **保存时无验证**: `save()` 不验证配置值的合法性（如快捷键格式、URL 格式、数值范围等）
4. **无文件锁**: 多进程同时读写配置文件时可能产生竞争条件
5. **Custom provider fallback**: `LLMProviderType::Custom` 在 `base_url` 为 `None` 时 fallback 到 `http://localhost:11434/v1`（Ollama 默认地址），`STTProviderType::Custom` fallback 到 `http://localhost:8080`，这些 fallback 值是硬编码的
6. **api_key_ref 双重语义**: 该字段既可以存储 `@keyref:` 前缀的 keyring 引用，也可以直接存储明文 API key（开发环境）。明文 key 会被写入 JSON 配置文件
