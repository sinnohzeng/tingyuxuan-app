use crate::llm::provider::{LLMInput, ProcessingMode};

/// Build the (system_message, user_message) pair for the given processing mode.
pub fn build_prompt(mode: &ProcessingMode, input: &LLMInput) -> (String, String) {
    match mode {
        ProcessingMode::Dictate => build_dictate_prompt(input),
        ProcessingMode::Translate => build_translate_prompt(input),
        ProcessingMode::AiAssistant => build_ai_assistant_prompt(input),
        ProcessingMode::Edit => build_edit_prompt(input),
    }
}

// ---------------------------------------------------------------------------
// Dictate mode  (main MVP prompt from PRD)
// ---------------------------------------------------------------------------

fn build_dictate_prompt(input: &LLMInput) -> (String, String) {
    let dictionary_hint = format_dictionary_hint(&input.user_dictionary);
    let context_hint = format_context_hint(input.current_app.as_deref());
    let tone_hint = format_tone_hint(input.current_app.as_deref());

    let system = format!(
        "你是一个智能语音输入助手。请将以下语音转写的原始文本整理为清晰、规范的书面文字。\n\n\
         规则：\n\
         1. 去除所有填充词（嗯、啊、那个、um、uh、like、you know 等）\n\
         2. 如果用户在说话中途改口或修正，只保留最终意图的表达\n\
         3. 去除不必要的重复词句\n\
         4. 自动补充合适的标点符号\n\
         5. 保持用户原本的表达意图和核心内容，不要添加额外信息\n\
         6. 如果用户说了「第一」「第二」或「首先」「其次」等顺序词，自动生成有序列表\n\
         7. 如果用户使用并列结构（如「有以下几点」），自动生成无序列表\n\
         {dictionary_hint}\n\
         {context_hint}\n\
         {tone_hint}"
    );

    let user = input.raw_transcript.clone();
    (system, user)
}

// ---------------------------------------------------------------------------
// Translate mode  (template only, not used in MVP pipeline)
// ---------------------------------------------------------------------------

fn build_translate_prompt(input: &LLMInput) -> (String, String) {
    let target = input.target_language.as_deref().unwrap_or("en");

    let system = format!(
        "你是一个专业翻译助手。请将用户的语音转写文本翻译为{target}。\n\n\
         规则：\n\
         1. 先整理原始语音转写（去除填充词、重复），再翻译\n\
         2. 保持原文的语气和风格\n\
         3. 只输出翻译后的文本，不要附加解释"
    );

    let user = input.raw_transcript.clone();
    (system, user)
}

// ---------------------------------------------------------------------------
// AI Assistant mode  (template only)
// ---------------------------------------------------------------------------

fn build_ai_assistant_prompt(input: &LLMInput) -> (String, String) {
    let context_hint = format_context_hint(input.current_app.as_deref());

    let system = format!(
        "你是一个智能助手。用户通过语音输入了一个请求，请理解其意图并给出简洁、实用的回复。\n\n\
         规则：\n\
         1. 先理解用户的核心意图（语音转写可能包含填充词和重复）\n\
         2. 给出直接、可操作的回答\n\
         3. 回复应简洁，适合直接插入到用户正在编辑的文档中\n\
         {context_hint}"
    );

    let user = input.raw_transcript.clone();
    (system, user)
}

// ---------------------------------------------------------------------------
// Edit mode  (template only)
// ---------------------------------------------------------------------------

fn build_edit_prompt(input: &LLMInput) -> (String, String) {
    let selected = input.selected_text.as_deref().unwrap_or("");

    let system = "你是一个文本编辑助手。用户选中了一段文本，并通过语音给出了修改指令。\n\n\
         规则：\n\
         1. 理解用户的语音修改指令（可能包含填充词）\n\
         2. 对选中的文本执行相应修改\n\
         3. 只输出修改后的文本，不要附加解释"
        .to_string();

    let user = format!(
        "选中的文本：\n{selected}\n\n语音指令：\n{}",
        input.raw_transcript
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format the user dictionary hint for inclusion in the system prompt.
/// Returns an empty string when the dictionary is empty.
pub fn format_dictionary_hint(words: &[String]) -> String {
    if words.is_empty() {
        return String::new();
    }
    format!(
        "用户自定义词典（优先使用这些词汇的正确写法）：{}",
        words.join("、")
    )
}

/// Format a context hint based on the currently active application.
/// Returns an empty string when no app name is provided.
pub fn format_context_hint(app_name: Option<&str>) -> String {
    match app_name {
        Some(name) if !name.is_empty() => {
            format!("当前用户正在使用的应用：{name}")
        }
        _ => String::new(),
    }
}

/// Detect the appropriate tone based on the active application name.
fn detect_tone(app_name: &str) -> &'static str {
    let lower = app_name.to_lowercase();
    if contains_any(
        &lower,
        &[
            "slack", "discord", "telegram", "wechat", "微信", "dingtalk", "钉钉", "teams",
        ],
    ) {
        "casual" // IM/chat → conversational
    } else if contains_any(&lower, &["mail", "outlook", "thunderbird", "邮", "foxmail"]) {
        "formal" // Email → formal
    } else if contains_any(
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
        ],
    ) {
        "technical" // IDE/terminal → preserve technical terms
    } else if contains_any(
        &lower,
        &["notion", "obsidian", "logseq", "typora", "bear", "joplin"],
    ) {
        "structured" // Note-taking → structured markdown
    } else {
        "neutral"
    }
}

/// Build a tone-specific hint for the LLM system prompt.
fn format_tone_hint(app_name: Option<&str>) -> String {
    let tone = match app_name {
        Some(name) if !name.is_empty() => detect_tone(name),
        _ => return String::new(),
    };

    match tone {
        "casual" => "语气提示：用户正在聊天应用中，请保持口语化和轻松的表达风格。".to_string(),
        "formal" => "语气提示：用户正在写邮件，请使用正式、专业的书面表达。".to_string(),
        "technical" => {
            "语气提示：用户正在使用开发工具，请保留技术术语和代码相关词汇的原始写法。"
                .to_string()
        }
        "structured" => {
            "语气提示：用户正在使用笔记应用，适当使用 Markdown 格式（标题、列表、粗体等）来组织内容。"
                .to_string()
        }
        _ => String::new(),
    }
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

    fn make_input(mode: ProcessingMode, transcript: &str) -> LLMInput {
        LLMInput {
            mode,
            raw_transcript: transcript.to_string(),
            target_language: None,
            selected_text: None,
            current_app: None,
            user_dictionary: Vec::new(),
        }
    }

    #[test]
    fn test_dictate_prompt_basic() {
        let input = make_input(ProcessingMode::Dictate, "嗯 那个 今天天气不错");
        let (system, user) = build_prompt(&input.mode, &input);

        assert!(system.contains("智能语音输入助手"));
        assert!(system.contains("去除所有填充词"));
        assert_eq!(user, "嗯 那个 今天天气不错");
    }

    #[test]
    fn test_dictate_prompt_with_dictionary() {
        let input = LLMInput {
            mode: ProcessingMode::Dictate,
            raw_transcript: "test".to_string(),
            target_language: None,
            selected_text: None,
            current_app: None,
            user_dictionary: vec!["TingYuXuan".to_string(), "Rust".to_string()],
        };
        let (system, _user) = build_prompt(&input.mode, &input);

        assert!(system.contains("TingYuXuan"));
        assert!(system.contains("Rust"));
        assert!(system.contains("用户自定义词典"));
    }

    #[test]
    fn test_dictate_prompt_with_context() {
        let input = LLMInput {
            mode: ProcessingMode::Dictate,
            raw_transcript: "test".to_string(),
            target_language: None,
            selected_text: None,
            current_app: Some("Visual Studio Code".to_string()),
            user_dictionary: Vec::new(),
        };
        let (system, _user) = build_prompt(&input.mode, &input);

        assert!(system.contains("Visual Studio Code"));
        assert!(system.contains("当前用户正在使用的应用"));
    }

    #[test]
    fn test_translate_prompt() {
        let input = LLMInput {
            mode: ProcessingMode::Translate,
            raw_transcript: "你好世界".to_string(),
            target_language: Some("en".to_string()),
            selected_text: None,
            current_app: None,
            user_dictionary: Vec::new(),
        };
        let (system, user) = build_prompt(&input.mode, &input);

        assert!(system.contains("翻译"));
        assert!(system.contains("en"));
        assert_eq!(user, "你好世界");
    }

    #[test]
    fn test_translate_prompt_default_language() {
        let input = make_input(ProcessingMode::Translate, "hello");
        let (system, _user) = build_prompt(&input.mode, &input);
        // Should default to "en"
        assert!(system.contains("en"));
    }

    #[test]
    fn test_ai_assistant_prompt() {
        let input = make_input(ProcessingMode::AiAssistant, "帮我写一个函数");
        let (system, user) = build_prompt(&input.mode, &input);

        assert!(system.contains("智能助手"));
        assert_eq!(user, "帮我写一个函数");
    }

    #[test]
    fn test_edit_prompt() {
        let input = LLMInput {
            mode: ProcessingMode::Edit,
            raw_transcript: "把这段改成英文".to_string(),
            target_language: None,
            selected_text: Some("你好世界".to_string()),
            current_app: None,
            user_dictionary: Vec::new(),
        };
        let (system, user) = build_prompt(&input.mode, &input);

        assert!(system.contains("文本编辑助手"));
        assert!(user.contains("你好世界"));
        assert!(user.contains("把这段改成英文"));
    }

    #[test]
    fn test_edit_prompt_no_selected_text() {
        let input = make_input(ProcessingMode::Edit, "修改一下");
        let (_system, user) = build_prompt(&input.mode, &input);

        assert!(user.contains("选中的文本"));
        assert!(user.contains("修改一下"));
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
    fn test_format_context_hint_none() {
        assert_eq!(format_context_hint(None), "");
    }

    #[test]
    fn test_format_context_hint_empty() {
        assert_eq!(format_context_hint(Some("")), "");
    }

    #[test]
    fn test_format_context_hint_with_app() {
        let hint = format_context_hint(Some("Firefox"));
        assert!(hint.contains("Firefox"));
    }

    // -- Tone detection tests -------------------------------------------------

    #[test]
    fn test_detect_tone_chat() {
        assert_eq!(detect_tone("Slack - general"), "casual");
        assert_eq!(detect_tone("Discord"), "casual");
        assert_eq!(detect_tone("微信"), "casual");
        assert_eq!(detect_tone("DingTalk"), "casual");
    }

    #[test]
    fn test_detect_tone_email() {
        assert_eq!(detect_tone("Outlook"), "formal");
        assert_eq!(detect_tone("Thunderbird Mail"), "formal");
        assert_eq!(detect_tone("Foxmail"), "formal");
    }

    #[test]
    fn test_detect_tone_dev() {
        assert_eq!(detect_tone("Visual Studio Code"), "technical");
        assert_eq!(detect_tone("IntelliJ IDEA"), "technical");
        assert_eq!(detect_tone("vim"), "technical");
        assert_eq!(detect_tone("Alacritty"), "technical");
    }

    #[test]
    fn test_detect_tone_notes() {
        assert_eq!(detect_tone("Notion"), "structured");
        assert_eq!(detect_tone("Obsidian"), "structured");
        assert_eq!(detect_tone("Logseq"), "structured");
    }

    #[test]
    fn test_detect_tone_unknown() {
        assert_eq!(detect_tone("Firefox"), "neutral");
        assert_eq!(detect_tone("Random App"), "neutral");
    }

    #[test]
    fn test_format_tone_hint_none() {
        assert_eq!(format_tone_hint(None), "");
    }

    #[test]
    fn test_format_tone_hint_chat() {
        let hint = format_tone_hint(Some("Slack - general"));
        assert!(hint.contains("口语化"));
    }

    #[test]
    fn test_format_tone_hint_formal() {
        let hint = format_tone_hint(Some("Outlook"));
        assert!(hint.contains("正式"));
    }

    #[test]
    fn test_dictate_prompt_with_list_rules() {
        let input = make_input(ProcessingMode::Dictate, "test");
        let (system, _) = build_prompt(&input.mode, &input);
        assert!(system.contains("有序列表"));
        assert!(system.contains("无序列表"));
    }

    #[test]
    fn test_dictate_prompt_with_tone() {
        let input = LLMInput {
            mode: ProcessingMode::Dictate,
            raw_transcript: "test".to_string(),
            target_language: None,
            selected_text: None,
            current_app: Some("Slack - general".to_string()),
            user_dictionary: Vec::new(),
        };
        let (system, _) = build_prompt(&input.mode, &input);
        assert!(system.contains("口语化"));
    }
}
