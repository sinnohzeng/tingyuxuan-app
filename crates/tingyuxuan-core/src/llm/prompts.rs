use crate::context::{InputContext, InputFieldType};
use crate::llm::provider::ProcessingMode;

/// UTF-8 安全截断：在 `max_bytes` 以内找到最近的 char boundary。
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// 语气枚举，替代旧的字符串匹配
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Casual,     // 聊天、即时通讯
    Formal,     // 邮件、文档
    Technical,  // 代码、开发工具
    Structured, // 笔记类应用：Obsidian、Notion 等
    Neutral,    // 默认：未知应用，纯文本输出
}

/// Build the system prompt for multimodal audio input processing.
pub fn build_multimodal_system_prompt(
    mode: &ProcessingMode,
    context: &InputContext,
    dictionary: &[String],
    target_language: Option<&str>,
) -> String {
    match mode {
        ProcessingMode::Dictate => build_dictate_system(context, dictionary),
        ProcessingMode::Translate => build_translate_system(target_language),
        ProcessingMode::AiAssistant => build_ai_assistant_system(context),
        ProcessingMode::Edit => build_edit_system(context),
    }
}

// ---------------------------------------------------------------------------
// Dictate mode
// ---------------------------------------------------------------------------

fn build_dictate_system(context: &InputContext, dictionary: &[String]) -> String {
    let dictionary_hint = format_dictionary_hint(dictionary);
    let context_block = format_rich_context(context);
    let tone_hint = format_tone_hint(context);

    format!(
        "你是一个智能语音输入助手。请听取用户的语音录音，将其转换为清晰、规范的书面文字。\n\n\
         规则：\n\
         1. 去除所有填充词（嗯、啊、那个、um、uh、like、you know 等）\n\
         2. 如果用户在说话中途改口或修正，只保留最终意图的表达\n\
         3. 去除不必要的重复词句\n\
         4. 自动补充合适的标点符号\n\
         5. 保持用户原本的表达意图和核心内容，不要添加额外信息\n\
         6. 如果用户说了「第一」「第二」或「首先」「其次」等顺序词，自动生成有序列表\n\
         7. 如果用户使用并列结构（如「有以下几点」），自动生成无序列表\n\
         8. 只输出整理后的文本，不要附加解释或说明\n\
         {dictionary_hint}\n\
         {context_block}\n\
         {tone_hint}"
    )
}

// ---------------------------------------------------------------------------
// Translate mode
// ---------------------------------------------------------------------------

fn build_translate_system(target_language: Option<&str>) -> String {
    let target = target_language.unwrap_or("en");

    format!(
        "你是一个专业翻译助手。请听取用户的语音录音，将其翻译为{target}。\n\n\
         规则：\n\
         1. 先理解语音内容（忽略填充词、重复），再翻译\n\
         2. 保持原文的语气和风格\n\
         3. 只输出翻译后的文本，不要附加解释"
    )
}

// ---------------------------------------------------------------------------
// AI Assistant mode
// ---------------------------------------------------------------------------

fn build_ai_assistant_system(context: &InputContext) -> String {
    let context_block = format_rich_context(context);

    format!(
        "你是一个智能助手。用户通过语音录音发送了一个请求，请理解其意图并给出简洁、实用的回复。\n\n\
         规则：\n\
         1. 直接理解用户语音的核心意图（录音可能包含填充词和重复）\n\
         2. 给出直接、可操作的回答\n\
         3. 回复应简洁，适合直接插入到用户正在编辑的文档中\n\
         {context_block}"
    )
}

// ---------------------------------------------------------------------------
// Edit mode
// ---------------------------------------------------------------------------

fn build_edit_system(context: &InputContext) -> String {
    let selected = context.selected_text.as_deref().unwrap_or("");

    format!(
        "你是一个文本编辑助手。用户选中了一段文本，并通过语音录音给出了修改指令。\n\n\
         规则：\n\
         1. 理解用户语音中的修改指令（可能包含填充词）\n\
         2. 对选中的文本执行相应修改\n\
         3. 只输出修改后的文本，不要附加解释\n\n\
         选中的文本：\n{selected}"
    )
}

// ---------------------------------------------------------------------------
// 上下文格式化
// ---------------------------------------------------------------------------

/// 将所有非空字段组装为结构化上下文块，完全透传原值
pub fn format_rich_context(ctx: &InputContext) -> String {
    let mut parts = Vec::new();

    if let Some(ref name) = ctx.app_name
        && !name.is_empty()
    {
        parts.push(format!("当前应用：{name}"));
    }
    if let Some(ref title) = ctx.window_title
        && !title.is_empty()
    {
        parts.push(format!("窗口标题：{title}"));
    }
    if let Some(ref url) = ctx.browser_url
        && !url.is_empty()
    {
        parts.push(format!("浏览器URL：{url}"));
    }
    if let Some(ref ft) = ctx.input_field_type {
        parts.push(format!("输入框类型：{ft}"));
    }
    if let Some(ref hint) = ctx.input_hint
        && !hint.is_empty()
    {
        parts.push(format!("输入提示：{hint}"));
    }
    if let Some(ref surrounding) = ctx.surrounding_text
        && !surrounding.is_empty()
    {
        let truncated = if surrounding.len() > 500 {
            format!("{}...", truncate_utf8(surrounding, 500))
        } else {
            surrounding.clone()
        };
        parts.push(format!("周围文本：{truncated}"));
    }
    if let Some(ref clip) = ctx.clipboard_text
        && !clip.is_empty()
    {
        let truncated = if clip.len() > 200 {
            format!("{}...", truncate_utf8(clip, 200))
        } else {
            clip.clone()
        };
        parts.push(format!("剪贴板：{truncated}"));
    }
    if let Some(ref screen) = ctx.screen_text
        && !screen.is_empty()
    {
        let truncated = if screen.len() > 500 {
            format!("{}...", truncate_utf8(screen, 500))
        } else {
            screen.clone()
        };
        parts.push(format!("屏幕文本：{truncated}"));
    }

    if parts.is_empty() {
        return String::new();
    }

    format!("上下文信息：\n{}", parts.join("\n"))
}

/// 综合判断语气
pub fn detect_tone(ctx: &InputContext) -> Tone {
    // 优先使用 input_field_type（Android 提供精确信息）
    if let Some(ref ft) = ctx.input_field_type {
        match ft {
            InputFieldType::Email => return Tone::Formal,
            InputFieldType::Chat => return Tone::Casual,
            InputFieldType::Code => return Tone::Technical,
            _ => {}
        }
    }

    // 回退到应用名称和 URL 推断
    let sources: Vec<&str> = [
        ctx.app_name.as_deref(),
        ctx.window_title.as_deref(),
        ctx.browser_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    for source in &sources {
        let lower = source.to_lowercase();
        if contains_any(
            &lower,
            &[
                "slack", "discord", "telegram", "wechat", "微信", "dingtalk", "钉钉", "teams",
            ],
        ) {
            return Tone::Casual;
        }
        if contains_any(&lower, &["mail", "outlook", "thunderbird", "邮", "foxmail"]) {
            return Tone::Formal;
        }
        if contains_any(
            &lower,
            &[
                "code",
                "intellij",
                "vim",
                "neovim",
                "terminal",
                "iterm",
                "wezterm",
                "alacritty",
                "emacs",
                "github.com",
                "gitlab.com",
                "stackoverflow.com",
            ],
        ) {
            return Tone::Technical;
        }
        if contains_any(
            &lower,
            &["notion", "obsidian", "logseq", "typora", "bear", "joplin"],
        ) {
            return Tone::Structured;
        }
    }

    Tone::Neutral // 默认：未知应用不注入特定格式提示
}

/// Build a tone-specific hint for the LLM system prompt.
fn format_tone_hint(ctx: &InputContext) -> String {
    match detect_tone(ctx) {
        Tone::Casual => "语气提示：用户正在聊天应用中，请保持口语化和轻松的表达风格。".to_string(),
        Tone::Formal => "语气提示：用户正在写邮件，请使用正式、专业的书面表达。".to_string(),
        Tone::Technical => {
            "语气提示：用户正在使用开发工具，请保留技术术语和代码相关词汇的原始写法。".to_string()
        }
        Tone::Structured => {
            "语气提示：适当使用 Markdown 格式（标题、列表、粗体等）来组织内容。".to_string()
        }
        Tone::Neutral => String::new(), // 不注入任何格式提示
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format the user dictionary hint for inclusion in the system prompt.
pub fn format_dictionary_hint(words: &[String]) -> String {
    if words.is_empty() {
        return String::new();
    }
    format!(
        "用户自定义词典（优先使用这些词汇的正确写法）：{}",
        words.join("、")
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictate_prompt_basic() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Dictate,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("智能语音输入助手"));
        assert!(system.contains("语音录音"));
        assert!(system.contains("去除所有填充词"));
    }

    #[test]
    fn test_dictate_prompt_with_dictionary() {
        let ctx = InputContext::default();
        let dict = vec!["TingYuXuan".to_string(), "Rust".to_string()];
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Dictate,
            &ctx,
            &dict,
            None,
        );
        assert!(system.contains("TingYuXuan"));
        assert!(system.contains("Rust"));
        assert!(system.contains("用户自定义词典"));
    }

    #[test]
    fn test_dictate_prompt_with_rich_context() {
        let ctx = InputContext {
            app_name: Some("Visual Studio Code".to_string()),
            window_title: Some("main.rs - tingyuxuan".to_string()),
            ..Default::default()
        };
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Dictate,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("Visual Studio Code"));
        assert!(system.contains("main.rs - tingyuxuan"));
    }

    #[test]
    fn test_translate_prompt() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Translate,
            &ctx,
            &[],
            Some("en"),
        );
        assert!(system.contains("翻译"));
        assert!(system.contains("en"));
    }

    #[test]
    fn test_translate_prompt_default_language() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Translate,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("en"));
    }

    #[test]
    fn test_ai_assistant_prompt() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::AiAssistant,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("智能助手"));
        assert!(system.contains("语音录音"));
    }

    #[test]
    fn test_edit_prompt() {
        let ctx = InputContext {
            selected_text: Some("你好世界".to_string()),
            ..Default::default()
        };
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Edit,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("文本编辑助手"));
        assert!(system.contains("你好世界"));
        assert!(system.contains("语音录音"));
    }

    #[test]
    fn test_edit_prompt_no_selected_text() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Edit,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("选中的文本"));
    }

    #[test]
    fn test_format_dictionary_hint_empty() {
        assert_eq!(format_dictionary_hint(&[]), "");
    }

    #[test]
    fn test_format_dictionary_hint_with_words() {
        let words = vec!["ABC".to_string(), "XYZ".to_string()];
        let hint = format_dictionary_hint(&words);
        assert!(hint.contains("ABC"));
        assert!(hint.contains("XYZ"));
        assert!(hint.contains("、"));
    }

    #[test]
    fn test_format_rich_context_empty() {
        let ctx = InputContext::default();
        assert_eq!(format_rich_context(&ctx), "");
    }

    #[test]
    fn test_format_rich_context_with_app() {
        let ctx = InputContext {
            app_name: Some("Firefox".to_string()),
            ..Default::default()
        };
        let result = format_rich_context(&ctx);
        assert!(result.contains("Firefox"));
        assert!(result.contains("当前应用"));
    }

    #[test]
    fn test_format_rich_context_multiple_fields() {
        let ctx = InputContext {
            app_name: Some("Chrome".to_string()),
            window_title: Some("Google".to_string()),
            browser_url: Some("https://google.com".to_string()),
            ..Default::default()
        };
        let result = format_rich_context(&ctx);
        assert!(result.contains("Chrome"));
        assert!(result.contains("Google"));
        assert!(result.contains("https://google.com"));
    }

    // -- Tone detection tests -------------------------------------------------

    #[test]
    fn test_detect_tone_from_input_field_type() {
        let ctx = InputContext {
            input_field_type: Some(InputFieldType::Email),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Formal);

        let ctx = InputContext {
            input_field_type: Some(InputFieldType::Chat),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Casual);

        let ctx = InputContext {
            input_field_type: Some(InputFieldType::Code),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Technical);
    }

    #[test]
    fn test_detect_tone_chat_from_app() {
        let ctx = InputContext {
            app_name: Some("Slack - general".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Casual);

        let ctx = InputContext {
            app_name: Some("微信".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Casual);
    }

    #[test]
    fn test_detect_tone_email_from_app() {
        let ctx = InputContext {
            app_name: Some("Outlook".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Formal);
    }

    #[test]
    fn test_detect_tone_dev_from_app() {
        let ctx = InputContext {
            app_name: Some("Visual Studio Code".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Technical);
    }

    #[test]
    fn test_detect_tone_dev_from_url() {
        let ctx = InputContext {
            browser_url: Some("https://github.com/tingyuxuan".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Technical);
    }

    #[test]
    fn test_detect_tone_notes() {
        let ctx = InputContext {
            app_name: Some("Obsidian".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Structured);
    }

    #[test]
    fn test_detect_tone_default() {
        let ctx = InputContext::default();
        assert_eq!(detect_tone(&ctx), Tone::Neutral);
    }

    #[test]
    fn test_detect_tone_input_field_type_overrides_app() {
        let ctx = InputContext {
            app_name: Some("Slack".to_string()),
            input_field_type: Some(InputFieldType::Email),
            ..Default::default()
        };
        assert_eq!(detect_tone(&ctx), Tone::Formal);
    }

    #[test]
    fn test_dictate_prompt_with_list_rules() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Dictate,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("有序列表"));
        assert!(system.contains("无序列表"));
    }

    #[test]
    fn test_dictate_prompt_with_tone() {
        let ctx = InputContext {
            app_name: Some("Slack - general".to_string()),
            ..Default::default()
        };
        let system = build_multimodal_system_prompt(
            &ProcessingMode::Dictate,
            &ctx,
            &[],
            None,
        );
        assert!(system.contains("口语化"));
    }
}
