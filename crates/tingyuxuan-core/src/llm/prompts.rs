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
        "你是「听语轩」语音输入引擎。你的任务是先识别音频，再输出可直接粘贴的书面文本。\n\n\
         硬性规则（必须全部满足）：\n\
         1. 只输出最终正文，不要加任何解释、前缀、后缀或免责声明\n\
         2. 严禁输出模板句或系统话术，例如「请开始录音」「我需要将语音内容转换为书面文字」\n\
         3. 去除口头填充词（嗯、啊、那个、um、uh、you know 等）和无意义重复\n\
         4. 用户中途改口时，只保留最终意图，不保留自我纠正痕迹\n\
         5. 补全合理标点，使文本自然、可读、可直接发送\n\
         6. 不编造信息，不补充未说出的事实\n\
         7. 出现「第一/第二/首先/其次」等顺序词时，自动整理成有序列表\n\
         8. 出现「有以下几点/包括」等并列提示时，自动整理成无序列表\n\
         9. 专有名词优先采用用户词典写法\n\
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
        "你是专业口语转写翻译引擎。先准确识别音频，再将内容翻译为 {target}。\n\n\
         硬性规则：\n\
         1. 只输出翻译后的最终文本，不要解释翻译策略\n\
         2. 去除填充词和重复后再翻译，保留原始语义和语气\n\
         3. 若原文有列表结构，译文保持对应结构\n\
         4. 严禁输出模板句（例如「请开始录音」）"
    )
}

// ---------------------------------------------------------------------------
// AI Assistant mode
// ---------------------------------------------------------------------------

fn build_ai_assistant_system(context: &InputContext) -> String {
    let context_block = format_rich_context(context);

    format!(
        "你是高效 AI 助手。用户通过语音提出请求，请直接给出可执行答案。\n\n\
         硬性规则：\n\
         1. 先理解语音中的真实意图，忽略填充词和重复\n\
         2. 回答要简洁、具体、可操作，默认以可直接粘贴文本输出\n\
         3. 严禁输出模板句（例如「请开始录音」）\n\
         {context_block}"
    )
}

// ---------------------------------------------------------------------------
// Edit mode
// ---------------------------------------------------------------------------

fn build_edit_system(context: &InputContext) -> String {
    let selected = context.selected_text.as_deref().unwrap_or("");

    format!(
        "你是文本编辑助手。用户选中了一段文本，并通过语音给出修改指令。\n\n\
         硬性规则：\n\
         1. 仅根据语音指令修改选中文本，不扩写无关内容\n\
         2. 只输出修改后的最终文本，不要解释过程\n\
         3. 严禁输出模板句（例如「请开始录音」）\n\n\
         选中的文本：\n{selected}"
    )
}

// ---------------------------------------------------------------------------
// 上下文格式化
// ---------------------------------------------------------------------------

/// 将所有非空字段组装为结构化上下文块，完全透传原值
pub fn format_rich_context(ctx: &InputContext) -> String {
    let mut parts = Vec::new();
    push_string_field(&mut parts, "当前应用", ctx.app_name.as_deref());
    push_string_field(&mut parts, "窗口标题", ctx.window_title.as_deref());
    push_string_field(&mut parts, "浏览器URL", ctx.browser_url.as_deref());
    push_display_field(&mut parts, "输入框类型", ctx.input_field_type.as_ref());
    push_string_field(&mut parts, "输入提示", ctx.input_hint.as_deref());
    push_truncated_field(&mut parts, "周围文本", ctx.surrounding_text.as_deref(), 500);
    push_truncated_field(&mut parts, "剪贴板", ctx.clipboard_text.as_deref(), 200);
    push_truncated_field(&mut parts, "屏幕文本", ctx.screen_text.as_deref(), 500);

    if parts.is_empty() {
        return String::new();
    }

    format!("上下文信息：\n{}", parts.join("\n"))
}

/// 综合判断语气
pub fn detect_tone(ctx: &InputContext) -> Tone {
    if let Some(tone) = tone_from_input_field(ctx.input_field_type.as_ref()) {
        return tone;
    }
    collect_tone_sources(ctx)
        .into_iter()
        .find_map(|source| classify_source_tone(&source.to_lowercase()))
        .unwrap_or(Tone::Neutral)
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

fn push_string_field(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(v) = value.filter(|s| !s.is_empty()) {
        parts.push(format!("{label}：{v}"));
    }
}

fn push_display_field<T: std::fmt::Display>(
    parts: &mut Vec<String>,
    label: &str,
    value: Option<&T>,
) {
    if let Some(v) = value {
        parts.push(format!("{label}：{v}"));
    }
}

fn push_truncated_field(parts: &mut Vec<String>, label: &str, value: Option<&str>, limit: usize) {
    if let Some(v) = value.filter(|s| !s.is_empty()) {
        parts.push(format!("{label}：{}", truncate_with_ellipsis(v, limit)));
    }
}

fn truncate_with_ellipsis(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    format!("{}...", truncate_utf8(value, limit))
}

fn tone_from_input_field(field_type: Option<&InputFieldType>) -> Option<Tone> {
    match field_type {
        Some(InputFieldType::Email) => Some(Tone::Formal),
        Some(InputFieldType::Chat) => Some(Tone::Casual),
        Some(InputFieldType::Code) => Some(Tone::Technical),
        _ => None,
    }
}

fn collect_tone_sources(ctx: &InputContext) -> Vec<&str> {
    [
        ctx.app_name.as_deref(),
        ctx.window_title.as_deref(),
        ctx.browser_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn classify_source_tone(lower: &str) -> Option<Tone> {
    [
        (
            Tone::Casual,
            &[
                "slack", "discord", "telegram", "wechat", "微信", "dingtalk", "钉钉", "teams",
            ][..],
        ),
        (
            Tone::Formal,
            &["mail", "outlook", "thunderbird", "邮", "foxmail"][..],
        ),
        (
            Tone::Technical,
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
            ][..],
        ),
        (
            Tone::Structured,
            &["notion", "obsidian", "logseq", "typora", "bear", "joplin"][..],
        ),
    ]
    .into_iter()
    .find_map(|(tone, keywords)| contains_any(lower, keywords).then_some(tone))
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
        let system = build_multimodal_system_prompt(&ProcessingMode::Dictate, &ctx, &[], None);
        assert!(system.contains("语音输入引擎"));
        assert!(system.contains("硬性规则"));
        assert!(system.contains("填充词"));
    }

    #[test]
    fn test_dictate_prompt_with_dictionary() {
        let ctx = InputContext::default();
        let dict = vec!["TingYuXuan".to_string(), "Rust".to_string()];
        let system = build_multimodal_system_prompt(&ProcessingMode::Dictate, &ctx, &dict, None);
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
        let system = build_multimodal_system_prompt(&ProcessingMode::Dictate, &ctx, &[], None);
        assert!(system.contains("Visual Studio Code"));
        assert!(system.contains("main.rs - tingyuxuan"));
    }

    #[test]
    fn test_translate_prompt() {
        let ctx = InputContext::default();
        let system =
            build_multimodal_system_prompt(&ProcessingMode::Translate, &ctx, &[], Some("en"));
        assert!(system.contains("翻译"));
        assert!(system.contains("en"));
    }

    #[test]
    fn test_translate_prompt_default_language() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(&ProcessingMode::Translate, &ctx, &[], None);
        assert!(system.contains("en"));
    }

    #[test]
    fn test_ai_assistant_prompt() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(&ProcessingMode::AiAssistant, &ctx, &[], None);
        assert!(system.contains("AI 助手"));
        assert!(system.contains("可执行答案"));
    }

    #[test]
    fn test_edit_prompt() {
        let ctx = InputContext {
            selected_text: Some("你好世界".to_string()),
            ..Default::default()
        };
        let system = build_multimodal_system_prompt(&ProcessingMode::Edit, &ctx, &[], None);
        assert!(system.contains("文本编辑助手"));
        assert!(system.contains("你好世界"));
        assert!(system.contains("语音"));
    }

    #[test]
    fn test_edit_prompt_no_selected_text() {
        let ctx = InputContext::default();
        let system = build_multimodal_system_prompt(&ProcessingMode::Edit, &ctx, &[], None);
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
        let system = build_multimodal_system_prompt(&ProcessingMode::Dictate, &ctx, &[], None);
        assert!(system.contains("有序列表"));
        assert!(system.contains("无序列表"));
    }

    #[test]
    fn test_dictate_prompt_with_tone() {
        let ctx = InputContext {
            app_name: Some("Slack - general".to_string()),
            ..Default::default()
        };
        let system = build_multimodal_system_prompt(&ProcessingMode::Dictate, &ctx, &[], None);
        assert!(system.contains("口语化"));
    }
}
