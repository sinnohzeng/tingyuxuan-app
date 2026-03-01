use crate::error::ConfigError;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current config version.  Bump this when the config schema changes.
/// v2: 移除 STT 配置，切换到多模态管线。
const CURRENT_CONFIG_VERSION: u32 = 2;

/// Main application configuration.
///
/// Android 端只传 `llm`/`language` 字段，其余用 `#[serde(default)]`
/// 填充默认值，避免反序列化失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version — used to detect and migrate old config files.
    /// Defaults to 0 for configs written before version tracking existed.
    #[serde(default)]
    pub config_version: u32,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
    pub language: LanguageConfig,
    pub llm: LLMConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub user_dictionary: Vec<String>,
    // 向后兼容：忽略旧配置中的 stt 字段。
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    stt: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub auto_launch: bool,
    pub sound_feedback: bool,
    pub floating_bar_position: FloatingBarPosition,
    /// 关闭主窗口时最小化到托盘而非退出。
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_launch: true,
            sound_feedback: true,
            floating_bar_position: FloatingBarPosition::BottomCenter,
            minimize_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FloatingBarPosition {
    #[serde(rename = "bottom_center")]
    BottomCenter,
    #[serde(rename = "follow_cursor")]
    FollowCursor,
    #[serde(rename = "fixed")]
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub dictate: String,
    pub translate: String,
    pub ai_assistant: String,
    pub cancel: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            dictate: "alt_right".to_string(),
            translate: "shift+alt_right".to_string(),
            ai_assistant: "alt+space".to_string(),
            cancel: "escape".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub primary: String,
    pub translation_target: String,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: LLMProviderType,
    /// API Key reference (stored in secure storage).
    /// Format: "@keyref:llm_api_key" or the actual key for development.
    pub api_key_ref: String,
    pub base_url: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMProviderType {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "dashscope")]
    DashScope,
    #[serde(rename = "volcengine")]
    Volcengine,
    #[serde(rename = "custom")]
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 历史记录保留天数。
    #[serde(default = "default_history_retention_days")]
    pub history_retention_days: u64,
    // 向后兼容：忽略旧配置中的已移除字段。
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    audio_retention_hours: Option<u64>,
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    failed_retention_days: Option<u64>,
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    max_cache_size_mb: Option<u64>,
}

fn default_history_retention_days() -> u64 {
    30
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            history_retention_days: 30,
            audio_retention_hours: None,
            failed_retention_days: None,
            max_cache_size_mb: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            general: GeneralConfig {
                auto_launch: true,
                sound_feedback: true,
                floating_bar_position: FloatingBarPosition::BottomCenter,
                minimize_to_tray: true,
            },
            shortcuts: ShortcutConfig {
                dictate: "alt_right".to_string(),
                translate: "shift+alt_right".to_string(),
                ai_assistant: "alt+space".to_string(),
                cancel: "escape".to_string(),
            },
            language: LanguageConfig {
                primary: "auto".to_string(),
                translation_target: "en".to_string(),
                variant: None,
            },
            llm: LLMConfig {
                provider: LLMProviderType::DashScope,
                api_key_ref: String::new(),
                base_url: None,
                model: "qwen3-omni-flash".to_string(),
            },
            cache: CacheConfig {
                history_retention_days: 30,
                audio_retention_hours: None,
                failed_retention_days: None,
                max_cache_size_mb: None,
            },
            user_dictionary: Vec::new(),
            stt: None,
        }
    }
}

impl AppConfig {
    /// Returns the platform-appropriate config directory.
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "tingyuxuan", "TingYuXuan") {
            Ok(proj_dirs.config_dir().to_path_buf())
        } else {
            Err(ConfigError::NoDirFound)
        }
    }

    /// Returns the path to the config file.
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Returns the platform-appropriate data directory for audio cache, history, etc.
    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "tingyuxuan", "TingYuXuan") {
            Ok(proj_dirs.data_dir().to_path_buf())
        } else {
            Err(ConfigError::NoDirFound)
        }
    }

    /// Load config from file. Returns default if file doesn't exist.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Load config with automatic migration from older versions.
    ///
    /// If the config file has an older `config_version`, this method applies
    /// incremental migrations (v0→v1, v1→v2, …), backs up the old file, and
    /// saves the migrated config.  Returns `Ok(Self::default())` if no config
    /// file exists yet.
    pub fn load_with_migration() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let mut config: Self = serde_json::from_str(&contents)?;

        if config.config_version < CURRENT_CONFIG_VERSION {
            // Back up the old config before migrating.
            let backup_path = path.with_extension(format!("v{}.json.bak", config.config_version));
            let _ = std::fs::copy(&path, &backup_path);
            tracing::info!(
                "Config migration: v{} → v{} (backup at {})",
                config.config_version,
                CURRENT_CONFIG_VERSION,
                backup_path.display()
            );

            // Apply incremental migrations.
            if config.config_version < 1 {
                Self::migrate_v0_to_v1(&mut config);
            }
            if config.config_version < 2 {
                Self::migrate_v1_to_v2(&mut config);
            }

            config.save()?;
        }

        Ok(config)
    }

    /// Migrate from v0 (no version field) to v1.
    fn migrate_v0_to_v1(config: &mut Self) {
        config.config_version = 1;
    }

    /// Migrate from v1 to v2: 移除 STT 配置，切换默认模型到 qwen3-omni-flash。
    fn migrate_v1_to_v2(config: &mut Self) {
        config.config_version = 2;
        config.stt = None;
        // 更新默认模型（如果还是旧值）。
        if config.llm.model == "gpt-4o-mini" {
            config.llm.model = "qwen3-omni-flash".to_string();
            config.llm.provider = LLMProviderType::DashScope;
            config.llm.base_url = None;
        }
    }

    /// Save config to file (write-to-temp + rename 原子写入)。
    ///
    /// 先写入临时文件，成功后 rename 覆盖目标文件，
    /// 避免写入中断导致配置文件损坏。
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;

        // 写入同目录的临时文件，确保和目标文件在同一文件系统上。
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &contents)?;
        std::fs::rename(&tmp_path, &path).map_err(|e| {
            // rename 失败时清理临时文件。
            let _ = std::fs::remove_file(&tmp_path);
            e
        })?;

        Ok(())
    }

    /// Get the default base URL for the configured LLM provider.
    pub fn llm_base_url(&self) -> String {
        if let Some(ref url) = self.llm.base_url {
            return url.clone();
        }
        match self.llm.provider {
            LLMProviderType::OpenAI => "https://api.openai.com/v1".to_string(),
            LLMProviderType::DashScope => {
                "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
            }
            LLMProviderType::Volcengine => "https://ark.cn-beijing.volces.com/api/v3".to_string(),
            LLMProviderType::Custom => self
                .llm
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string()),
        }
    }
}

/// Provider presets for quick configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub name: String,
    pub provider: LLMProviderType,
    pub base_url: String,
    pub models: Vec<String>,
}

impl ProviderPreset {
    pub fn all() -> Vec<ProviderPreset> {
        vec![
            ProviderPreset {
                name: "阿里云 Qwen3-Omni Flash（推荐）".to_string(),
                provider: LLMProviderType::DashScope,
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                models: vec![
                    "qwen3-omni-flash".to_string(),
                    "qwen-omni-turbo".to_string(),
                ],
            },
            ProviderPreset {
                name: "OpenAI GPT-4o Audio".to_string(),
                provider: LLMProviderType::OpenAI,
                base_url: "https://api.openai.com/v1".to_string(),
                models: vec!["gpt-4o-audio-preview".to_string()],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.llm.model, "qwen3-omni-flash");
        assert_eq!(config.shortcuts.dictate, "alt_right");
        assert_eq!(config.cache.history_retention_days, 30);
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.llm.model, config.llm.model);
        // STT 字段不应出现在序列化结果中。
        assert!(!json.contains("\"stt\""));
    }

    #[test]
    fn test_config_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: AppConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.llm.model, "qwen3-omni-flash");
    }

    #[test]
    fn test_llm_base_url_defaults() {
        let config = AppConfig::default();
        assert_eq!(
            config.llm_base_url(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );

        let mut config2 = AppConfig::default();
        config2.llm.provider = LLMProviderType::OpenAI;
        assert_eq!(config2.llm_base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn test_config_backward_compat_with_stt() {
        // 旧配置文件包含 stt 字段 — 应能正常反序列化。
        let json = r#"{
            "general": { "auto_launch": true, "sound_feedback": true, "floating_bar_position": "bottom_center" },
            "shortcuts": { "dictate": "ctrl+shift+d", "translate": "ctrl+shift+t", "ai_assistant": "ctrl+shift+a", "cancel": "escape" },
            "language": { "primary": "auto", "translation_target": "en", "variant": null },
            "stt": { "provider": "dashscope_streaming", "api_key_ref": "", "base_url": null, "model": "paraformer-realtime-v2" },
            "llm": { "provider": "openai", "api_key_ref": "", "base_url": null, "model": "gpt-4o-mini" },
            "cache": { "audio_retention_hours": 24, "failed_retention_days": 7, "max_cache_size_mb": 500 }
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.user_dictionary.is_empty());
        assert_eq!(config.llm.model, "gpt-4o-mini");
    }

    #[test]
    fn test_config_with_dictionary() {
        let config = AppConfig {
            user_dictionary: vec!["TingYuXuan".to_string(), "Rust".to_string()],
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_dictionary.len(), 2);
        assert_eq!(parsed.user_dictionary[0], "TingYuXuan");
    }

    #[test]
    fn test_provider_presets() {
        let presets = ProviderPreset::all();
        assert_eq!(presets.len(), 2);
        assert_eq!(presets[0].name, "阿里云 Qwen3-Omni Flash（推荐）");
        assert_eq!(presets[1].name, "OpenAI GPT-4o Audio");
    }

    #[test]
    fn test_default_config_version() {
        let config = AppConfig::default();
        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.config_version, 2);
    }

    #[test]
    fn test_old_config_deserializes_with_version_zero() {
        let json = r#"{
            "general": { "auto_launch": true, "sound_feedback": true, "floating_bar_position": "bottom_center" },
            "shortcuts": { "dictate": "ctrl+shift+d", "translate": "ctrl+shift+t", "ai_assistant": "ctrl+shift+a", "cancel": "escape" },
            "language": { "primary": "auto", "translation_target": "en", "variant": null },
            "stt": { "provider": "dashscope_streaming", "api_key_ref": "", "base_url": null, "model": "paraformer-realtime-v2" },
            "llm": { "provider": "openai", "api_key_ref": "", "base_url": null, "model": "gpt-4o-mini" },
            "cache": { "audio_retention_hours": 24, "failed_retention_days": 7, "max_cache_size_mb": 500 }
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.config_version, 0);
    }

    #[test]
    fn test_serialization_includes_version() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"config_version\":2"));
    }

    #[test]
    fn test_migration_v0_to_v2() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Write a v0 config (no config_version field).
        let v0_json = serde_json::to_string_pretty(&serde_json::json!({
            "general": { "auto_launch": true, "sound_feedback": true, "floating_bar_position": "bottom_center" },
            "shortcuts": { "dictate": "ctrl+shift+d", "translate": "ctrl+shift+t", "ai_assistant": "ctrl+shift+a", "cancel": "escape" },
            "language": { "primary": "zh", "translation_target": "en", "variant": null },
            "stt": { "provider": "dashscope_streaming", "api_key_ref": "test-key", "base_url": null, "model": "paraformer-realtime-v2" },
            "llm": { "provider": "openai", "api_key_ref": "test-key", "base_url": null, "model": "gpt-4o-mini" },
            "cache": { "audio_retention_hours": 48, "failed_retention_days": 7, "max_cache_size_mb": 500 }
        }))
        .unwrap();
        std::fs::write(&path, &v0_json).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let mut config: AppConfig = serde_json::from_str(&contents).unwrap();
        assert_eq!(config.config_version, 0);

        AppConfig::migrate_v0_to_v1(&mut config);
        assert_eq!(config.config_version, 1);

        AppConfig::migrate_v1_to_v2(&mut config);
        assert_eq!(config.config_version, 2);
        // 默认模型已更新。
        assert_eq!(config.llm.model, "qwen3-omni-flash");
        // 原始值保留。
        assert_eq!(config.language.primary, "zh");
        assert_eq!(config.cache.history_retention_days, 30);
    }

    #[test]
    fn test_migration_preserves_custom_model() {
        let mut config = AppConfig::default();
        config.config_version = 1;
        config.llm.model = "gpt-4o".to_string();
        config.llm.provider = LLMProviderType::OpenAI;

        AppConfig::migrate_v1_to_v2(&mut config);
        // 自定义模型不应被覆盖。
        assert_eq!(config.llm.model, "gpt-4o");
    }

    #[test]
    fn test_config_version_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.config_version, config.config_version);
    }

    #[test]
    fn test_android_minimal_config_deserializes() {
        // Android 只提供 llm/language/user_dictionary，
        // general/shortcuts/cache 应使用 serde(default) 默认值。
        let json = r#"{
            "llm": { "provider": "dashscope", "api_key_ref": "test-key", "model": "qwen3-omni-flash" },
            "language": { "primary": "zh", "translation_target": "en" },
            "user_dictionary": []
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.general.auto_launch);
        assert_eq!(config.shortcuts.dictate, "alt_right");
        assert_eq!(config.cache.history_retention_days, 30);
    }
}
