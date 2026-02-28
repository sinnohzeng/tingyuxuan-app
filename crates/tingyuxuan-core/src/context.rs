use serde::{Deserialize, Serialize};
use std::fmt;

/// 输入框类型枚举，避免魔法字符串
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputFieldType {
    Email,
    Chat,
    Code,
    Search,
    Url,
    Multiline,
    Text,
    /// 前向兼容：未知的输入框类型不会导致整体反序列化失败。
    #[serde(other)]
    Unknown,
}

impl fmt::Display for InputFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputFieldType::Email => write!(f, "邮件"),
            InputFieldType::Chat => write!(f, "聊天"),
            InputFieldType::Code => write!(f, "代码"),
            InputFieldType::Search => write!(f, "搜索"),
            InputFieldType::Url => write!(f, "网址"),
            InputFieldType::Multiline => write!(f, "多行文本"),
            InputFieldType::Text => write!(f, "文本"),
            InputFieldType::Unknown => write!(f, "未知"),
        }
    }
}

/// 编辑器动作枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorAction {
    Send,
    Search,
    Go,
    Done,
    Next,
    Unspecified,
    /// 前向兼容：未知的编辑器动作不会导致整体反序列化失败。
    #[serde(other)]
    Unknown,
}

/// 统一上下文模型，对标 Typeless 的上下文采集范围
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputContext {
    // 应用信息
    pub app_name: Option<String>,
    pub app_package: Option<String>,
    pub window_title: Option<String>,

    // 浏览器信息
    pub browser_url: Option<String>,

    // 输入框信息（桌面端为 None，仅 Android 通过 EditorInfo 获取）
    pub input_field_type: Option<InputFieldType>,
    pub input_hint: Option<String>,
    pub editor_action: Option<EditorAction>,

    // 文本上下文
    pub surrounding_text: Option<String>,
    pub selected_text: Option<String>,
    pub clipboard_text: Option<String>,
    pub screen_text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_all_none() {
        let ctx = InputContext::default();
        assert!(ctx.app_name.is_none());
        assert!(ctx.selected_text.is_none());
        assert!(ctx.input_field_type.is_none());
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let ctx = InputContext {
            app_name: Some("Firefox".to_string()),
            app_package: Some("org.mozilla.firefox".to_string()),
            window_title: Some("GitHub - tingyuxuan".to_string()),
            browser_url: None,
            input_field_type: Some(InputFieldType::Chat),
            input_hint: Some("输入消息...".to_string()),
            editor_action: Some(EditorAction::Send),
            surrounding_text: Some("前面的文字".to_string()),
            selected_text: Some("选中的文字".to_string()),
            clipboard_text: Some("剪贴板内容".to_string()),
            screen_text: None,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: InputContext = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.app_name, ctx.app_name);
        assert_eq!(parsed.app_package, ctx.app_package);
        assert_eq!(parsed.input_field_type, ctx.input_field_type);
        assert_eq!(parsed.editor_action, ctx.editor_action);
        assert_eq!(parsed.selected_text, ctx.selected_text);
    }

    #[test]
    fn test_input_field_type_serde() {
        let json = r#""email""#;
        let parsed: InputFieldType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, InputFieldType::Email);

        let json = r#""chat""#;
        let parsed: InputFieldType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, InputFieldType::Chat);
    }

    #[test]
    fn test_editor_action_serde() {
        let json = r#""send""#;
        let parsed: EditorAction = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, EditorAction::Send);
    }

    #[test]
    fn test_deserialize_partial_context() {
        let json = r#"{"app_name": "Slack"}"#;
        let ctx: InputContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.app_name.as_deref(), Some("Slack"));
        assert!(ctx.window_title.is_none());
        assert!(ctx.input_field_type.is_none());
    }
}
