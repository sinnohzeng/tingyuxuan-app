mod error;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

pub use error::PlatformError;

/// Text injection trait — inject text at the current cursor position.
pub trait TextInjector: Send + Sync {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError>;
}

/// Context detection trait — detect active window and selected text.
pub trait ContextDetector: Send + Sync {
    fn get_active_window_name(&self) -> Option<String>;
    fn get_selected_text(&self) -> Option<String>;
}

/// Strip dangerous control characters from text that will be typed directly.
///
/// Retains `\n` (newline) and `\t` (tab) which are legitimate in typed text.
/// Removes all other Unicode control characters (ASCII 0x00–0x1F except 0x0A
/// and 0x09, plus 0x7F DEL, and Unicode categories Cc/Cf beyond basic Latin)
/// that could cause unintended behaviour in target applications.
pub fn sanitize_for_typing(text: &str) -> String {
    text.chars()
        .filter(|c| {
            // Keep newline and tab
            if *c == '\n' || *c == '\t' {
                return true;
            }
            // Remove other control characters (covers both ASCII and Unicode)
            !c.is_control()
        })
        .collect()
}

// Compile-time type aliases — zero overhead, no Box/dyn
#[cfg(target_os = "linux")]
pub type PlatformInjector = linux::LinuxTextInjector;
#[cfg(target_os = "linux")]
pub type PlatformDetector = linux::LinuxContextDetector;

#[cfg(target_os = "windows")]
pub type PlatformInjector = windows::WindowsTextInjector;
#[cfg(target_os = "windows")]
pub type PlatformDetector = windows::WindowsContextDetector;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_preserves_normal_text() {
        assert_eq!(sanitize_for_typing("Hello, 世界!"), "Hello, 世界!");
    }

    #[test]
    fn sanitize_preserves_newlines_and_tabs() {
        assert_eq!(
            sanitize_for_typing("line1\nline2\tend"),
            "line1\nline2\tend"
        );
    }

    #[test]
    fn sanitize_strips_null_and_backspace() {
        assert_eq!(sanitize_for_typing("abc\0def\x08ghi"), "abcdefghi");
    }

    #[test]
    fn sanitize_strips_escape_and_del() {
        assert_eq!(sanitize_for_typing("ab\x1bcd\x7fef"), "abcdef");
    }

    #[test]
    fn sanitize_strips_bell_and_form_feed() {
        assert_eq!(sanitize_for_typing("a\x07b\x0cc"), "abc");
    }

    #[test]
    fn sanitize_strips_unicode_control_characters() {
        // U+200B Zero Width Space, U+200E Left-to-Right Mark, U+FEFF BOM
        // These are Unicode format characters (Cf category) — is_control() returns true
        // for Cc characters only. Cf characters like ZWNBSP/LTR are NOT control chars
        // in Rust's definition. Test for actual Cc characters.
        // U+0085 NEL (Next Line) — a Unicode Cc character
        // U+008A — a Unicode Cc character (C1 control)
        let input = "hello\u{0085}world\u{008A}end";
        let result = sanitize_for_typing(input);
        assert_eq!(result, "helloworldend");
    }
}
