# Configuration

## 模块职责

配置模块管理应用配置的序列化、加载、迁移与校验，使用 JSON 持久化到用户目录，并保证旧版本配置可平滑升级。

**源文件:** `crates/tingyuxuan-core/src/config.rs`

---

## 关键类型定义

### AppConfig（顶层）

```rust
pub struct AppConfig {
    pub config_version: u32,
    pub general: GeneralConfig,
    pub shortcuts: ShortcutConfig,
    pub language: LanguageConfig,
    pub llm: LLMConfig,
    pub cache: CacheConfig,
    pub audio: AudioConfig,
    pub user_dictionary: Vec<String>,
    #[serde(default, skip_serializing)]
    stt: Option<serde_json::Value>,
}
```

- `stt` 字段仅用于兼容读取旧配置，不再序列化输出。
- 当前配置版本：`2`（`CURRENT_CONFIG_VERSION`）。

### GeneralConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `auto_launch` | `bool` | `true` | 开机自启 |
| `sound_feedback` | `bool` | `true` | 声音反馈 |
| `floating_bar_position` | `FloatingBarPosition` | `BottomCenter` | 悬浮条位置 |
| `minimize_to_tray` | `bool` | `true` | 关闭主窗口时最小化到托盘 |

### ShortcutConfig

| 字段 | 默认值 |
|------|--------|
| `dictate` | `alt_right` |
| `translate` | `shift+alt_right` |
| `ai_assistant` | `alt+space` |
| `cancel` | `escape` |

### LanguageConfig

| 字段 | 类型 | 默认值 |
|------|------|--------|
| `primary` | `String` | `"auto"` |
| `translation_target` | `String` | `"en"` |
| `variant` | `Option<String>` | `None` |

### LLMConfig

| 字段 | 类型 | 默认值 |
|------|------|--------|
| `provider` | `LLMProviderType` | `DashScope` |
| `api_key_ref` | `String` | `""` |
| `base_url` | `Option<String>` | `None` |
| `model` | `String` | `"qwen3-omni-flash"` |

`LLMProviderType`：`openai` / `dashscope` / `volcengine` / `custom`

### CacheConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `history_retention_days` | `u64` | `30` | 历史记录保留天数 |

兼容字段（仅读取，序列化时忽略）：
- `audio_retention_hours`
- `failed_retention_days`
- `max_cache_size_mb`

### AudioConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `input_device_id` | `Option<String>` | `None` | 指定输入设备，`None` 为系统默认 |

---

## Public API

### 路径与读写

| 方法 | 签名 | 说明 |
|------|------|------|
| `config_dir()` | `fn config_dir() -> Result<PathBuf, ConfigError>` | 配置目录 |
| `config_path()` | `fn config_path() -> Result<PathBuf, ConfigError>` | 配置文件路径 |
| `data_dir()` | `fn data_dir() -> Result<PathBuf, ConfigError>` | 数据目录 |
| `load()` | `fn load() -> Result<Self, ConfigError>` | 加载配置；不存在则返回默认值 |
| `load_with_migration()` | `fn load_with_migration() -> Result<Self, ConfigError>` | 自动迁移旧配置 |
| `save()` | `fn save(&self) -> Result<(), ConfigError>` | 校验后原子写入（tmp + rename） |
| `validate()` | `fn validate(&self) -> Result<(), ConfigError>` | 保存前校验 |
| `llm_base_url()` | `fn llm_base_url(&self) -> String` | 返回当前 provider 的 base URL |

### ProviderPreset

| 方法 | 签名 | 说明 |
|------|------|------|
| `all()` | `fn all() -> Vec<ProviderPreset>` | 返回内置 LLM 预设 |

内置预设：
- 阿里云 Qwen3-Omni Flash（推荐）
- OpenAI GPT-4o Audio

---

## 配置迁移

迁移入口：`load_with_migration()`

流程：
1. 读取配置并检查 `config_version`
2. 低版本时备份旧文件（如 `config.v0.json.bak`）
3. 逐步执行迁移并回写配置

版本链：
- `v0 -> v1`: 引入 `config_version`
- `v1 -> v2`: 移除 STT 配置语义，默认模型切换到 `qwen3-omni-flash`

---

## 错误处理

`ConfigError` 负责统一封装：
- IO 错误
- JSON 序列化/反序列化错误
- 目录不可用
- 配置值校验错误

---

## 已知限制

1. 未引入跨进程文件锁，多进程并发写入仍可能竞争。
2. `LLMProviderType::Custom` 在无 `base_url` 时回退到固定默认地址（硬编码）。
3. `api_key_ref` 支持明文与 `@keyref:` 双语义，开发便利性与安全性需权衡。
