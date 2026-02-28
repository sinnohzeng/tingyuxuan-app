use crate::error::ConfigError;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current config version.  Bump this when the config schema changes.
const CURRENT_CONFIG_VERSION: u32 = 1;

/// Main application configuration.
///
/// Android 端只传 `stt`/`llm`/`language` 字段，其余用 `#[serde(default)]`
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
    pub stt: STTConfig,
    pub llm: LLMConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub user_dictionary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub auto_launch: bool,
    pub sound_feedback: bool,
    pub floating_bar_position: FloatingBarPosition,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_launch: true,
            sound_feedback: true,
            floating_bar_position: FloatingBarPosition::BottomCenter,
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
pub struct STTConfig {
    pub provider: STTProviderType,
    /// API Key is stored in secure storage; this field holds a reference ID.
    /// Format: "@keyref:stt_api_key" or the actual key for development.
    pub api_key_ref: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum STTProviderType {
    /// DashScope Paraformer 流式识别（WebSocket）— MVP 唯一 STT 选项。
    #[serde(rename = "dashscope_streaming")]
    DashScopeStreaming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: LLMProviderType,
    /// API Key reference (see STTConfig.api_key_ref).
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
    /// 历史记录保留天数（流式架构不再写音频文件）。
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
            stt: STTConfig {
                provider: STTProviderType::DashScopeStreaming,
                api_key_ref: String::new(),
                base_url: None,
                model: Some("paraformer-realtime-v2".to_string()),
            },
            llm: LLMConfig {
                provider: LLMProviderType::OpenAI,
                api_key_ref: String::new(),
                base_url: None,
                model: "gpt-4o-mini".to_string(),
            },
            cache: CacheConfig {
                history_retention_days: 30,
                audio_retention_hours: None,
                failed_retention_days: None,
                max_cache_size_mb: None,
            },
            user_dictionary: Vec::new(),
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
            // Future migrations: if config.config_version < 2 { migrate_v1_to_v2(&mut config); }

            config.save()?;
        }

        Ok(config)
    }

    /// Migrate from v0 (no version field) to v1.
    ///
    /// v0 → v1 is a baseline migration — the only change is setting the
    /// version number.  All new fields added since v0 are handled by
    /// `#[serde(default)]`.
    fn migrate_v0_to_v1(config: &mut Self) {
        config.config_version = 1;
    }

    /// Save config to file.
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
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

    /// Get the default base URL for the configured STT provider.
    pub fn stt_base_url(&self) -> String {
        if let Some(ref url) = self.stt.base_url {
            return url.clone();
        }
        match self.stt.provider {
            STTProviderType::DashScopeStreaming => {
                "wss://dashscope.aliyuncs.com/api-ws/v1/inference/".to_string()
            }
        }
    }
}

/// Provider presets for quick configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub name: String,
    pub llm_provider: LLMProviderType,
    pub llm_base_url: String,
    pub llm_models: Vec<String>,
    pub stt_provider: STTProviderType,
    pub stt_base_url: Option<String>,
    pub stt_model: Option<String>,
}

impl ProviderPreset {
    pub fn all() -> Vec<ProviderPreset> {
        vec![
            ProviderPreset {
                name: "阿里云 DashScope".to_string(),
                llm_provider: LLMProviderType::DashScope,
                llm_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                llm_models: vec![
                    "qwen-turbo".to_string(),
                    "qwen-plus".to_string(),
                    "qwen-max".to_string(),
                ],
                stt_provider: STTProviderType::DashScopeStreaming,
                stt_base_url: None,
                stt_model: Some("paraformer-realtime-v2".to_string()),
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
        assert_eq!(config.llm.model, "gpt-4o-mini");
        assert_eq!(config.shortcuts.dictate, "alt_right");
        assert_eq!(config.cache.history_retention_days, 30);
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.llm.model, config.llm.model);
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
        assert_eq!(loaded.llm.model, "gpt-4o-mini");
    }

    #[test]
    fn test_llm_base_url_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.llm_base_url(), "https://api.openai.com/v1");

        let mut config2 = AppConfig::default();
        config2.llm.provider = LLMProviderType::DashScope;
        assert_eq!(
            config2.llm_base_url(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
    }

    #[test]
    fn test_config_backward_compat_no_dictionary() {
        // Old config files won't have user_dictionary — should deserialize with empty Vec.
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
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "阿里云 DashScope");
    }

    #[test]
    fn test_default_config_version() {
        let config = AppConfig::default();
        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
    }

    #[test]
    fn test_old_config_deserializes_with_version_zero() {
        // Simulate a config file from before version tracking.
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
        assert!(json.contains("\"config_version\":1"));
    }

    #[test]
    fn test_migration_v0_to_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Write a v0 config (no config_version field).
        let v0_json = serde_json::to_string_pretty(&serde_json::json!({
            "general": { "auto_launch": true, "sound_feedback": true, "floating_bar_position": "bottom_center" },
            "shortcuts": { "dictate": "ctrl+shift+d", "translate": "ctrl+shift+t", "ai_assistant": "ctrl+shift+a", "cancel": "escape" },
            "language": { "primary": "zh", "translation_target": "en", "variant": null },
            "stt": { "provider": "dashscope_streaming", "api_key_ref": "test-key", "base_url": null, "model": "paraformer-realtime-v2" },
            "llm": { "provider": "openai", "api_key_ref": "test-key", "base_url": null, "model": "gpt-4o" },
            "cache": { "audio_retention_hours": 48, "failed_retention_days": 7, "max_cache_size_mb": 500 }
        })).unwrap();
        std::fs::write(&path, &v0_json).unwrap();

        // Manually load and migrate (can't use load_with_migration because it
        // uses config_path() which depends on ProjectDirs).
        let contents = std::fs::read_to_string(&path).unwrap();
        let mut config: AppConfig = serde_json::from_str(&contents).unwrap();
        assert_eq!(config.config_version, 0);

        AppConfig::migrate_v0_to_v1(&mut config);
        assert_eq!(config.config_version, 1);
        // Original values preserved.
        assert_eq!(config.language.primary, "zh");
        assert_eq!(config.llm.model, "gpt-4o");
        // Old config has audio_retention_hours (ignored) but no history_retention_days → serde default 30.
        assert_eq!(config.cache.history_retention_days, 30);
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
        // Android ConfigStore.buildConfigJson() 只提供 stt/llm/language/user_dictionary，
        // general/shortcuts/cache 应使用 serde(default) 默认值。
        let json = r#"{
            "stt": { "provider": "dashscope_streaming", "api_key_ref": "test-key" },
            "llm": { "provider": "openai", "api_key_ref": "test-key", "model": "gpt-4o-mini" },
            "language": { "primary": "zh", "translation_target": "en" },
            "user_dictionary": []
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        // 默认值应已填充。
        assert!(config.general.auto_launch);
        assert_eq!(config.shortcuts.dictate, "alt_right");
        assert_eq!(config.cache.history_retention_days, 30);
    }
}
