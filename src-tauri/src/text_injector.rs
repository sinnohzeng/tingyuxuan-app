use std::process::Command;

/// Detects the current display server type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

pub fn detect_display_server() -> DisplayServer {
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "x11" => return DisplayServer::X11,
            "wayland" => return DisplayServer::Wayland,
            _ => {}
        }
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return DisplayServer::Wayland;
    }
    if std::env::var("DISPLAY").is_ok() {
        return DisplayServer::X11;
    }
    DisplayServer::Unknown
}

/// Strip dangerous control characters from text that will be typed directly.
///
/// Retains `\n` (newline) and `\t` (tab) which are legitimate in typed text.
/// Removes all other ASCII control characters (0x00–0x1F except 0x0A and 0x09,
/// plus 0x7F DEL) that could cause unintended behaviour in target applications
/// (e.g. backspace deleting content, escape triggering shortcuts).
fn sanitize_for_typing(text: &str) -> String {
    text.chars()
        .filter(|c| {
            // Keep newline and tab
            if *c == '\n' || *c == '\t' {
                return true;
            }
            // Remove other control characters
            !c.is_control()
        })
        .collect()
}

/// Injects text at the current cursor position.
///
/// For short text (<200 chars), types directly (with control-char sanitization).
/// For long text, uses clipboard paste with save/restore.
pub fn inject_text(text: &str) -> Result<(), String> {
    let display = detect_display_server();
    let use_clipboard = text.len() > 200;

    match display {
        DisplayServer::X11 => inject_x11(text, use_clipboard),
        DisplayServer::Wayland => inject_wayland(text, use_clipboard),
        DisplayServer::Unknown => {
            // Try ydotool as universal fallback
            inject_ydotool(text)
        }
    }
}

fn inject_x11(text: &str, use_clipboard: bool) -> Result<(), String> {
    if use_clipboard {
        // Save current clipboard
        let saved = Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(o.stdout)
                } else {
                    None
                }
            });

        // Set clipboard to our text
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to run xclip: {}", e))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to xclip: {}", e))?;
        }
        child
            .wait()
            .map_err(|e| format!("xclip failed: {}", e))?;

        // Paste with Ctrl+V
        Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .output()
            .map_err(|e| format!("xdotool paste failed: {}", e))?;

        // Restore clipboard after a brief delay
        if let Some(original) = saved {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut child = Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .ok();
            if let Some(ref mut c) = child {
                if let Some(stdin) = c.stdin.as_mut() {
                    use std::io::Write;
                    let _ = stdin.write_all(&original);
                }
                let _ = c.wait();
            }
        }

        Ok(())
    } else {
        // Direct typing for short text — sanitize control characters first
        let safe = sanitize_for_typing(text);
        Command::new("xdotool")
            .args(["type", "--clearmodifiers", "--", &safe])
            .output()
            .map_err(|e| format!("xdotool type failed: {}", e))?;
        Ok(())
    }
}

fn inject_wayland(text: &str, use_clipboard: bool) -> Result<(), String> {
    if use_clipboard {
        // Save current clipboard
        let saved = Command::new("wl-paste")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(o.stdout)
                } else {
                    None
                }
            });

        // Set clipboard
        let mut child = Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to run wl-copy: {}", e))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to wl-copy: {}", e))?;
        }
        child
            .wait()
            .map_err(|e| format!("wl-copy failed: {}", e))?;

        // Paste with Ctrl+V via wtype
        Command::new("wtype")
            .args(["-M", "ctrl", "v", "-m", "ctrl"])
            .output()
            .map_err(|e| format!("wtype paste failed: {}", e))?;

        // Restore clipboard
        if let Some(original) = saved {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut child = Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .ok();
            if let Some(ref mut c) = child {
                if let Some(stdin) = c.stdin.as_mut() {
                    use std::io::Write;
                    let _ = stdin.write_all(&original);
                }
                let _ = c.wait();
            }
        }

        Ok(())
    } else {
        // Direct typing — sanitize control characters first
        let safe = sanitize_for_typing(text);
        Command::new("wtype")
            .args(["--", &safe])
            .output()
            .map_err(|e| format!("wtype failed: {}", e))?;
        Ok(())
    }
}

fn inject_ydotool(text: &str) -> Result<(), String> {
    // ydotool always types directly — sanitize control characters
    let safe = sanitize_for_typing(text);
    Command::new("ydotool")
        .args(["type", "--", &safe])
        .output()
        .map_err(|e| format!("ydotool type failed: {}", e))?;
    Ok(())
}

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
        assert_eq!(sanitize_for_typing("line1\nline2\tend"), "line1\nline2\tend");
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
    fn detect_display_unknown_without_env() {
        // Clear display-related env vars for this test.
        // Note: this test may interfere with other tests if run in parallel,
        // but is safe in the cargo test single-threaded default.
        let saved_session = std::env::var("XDG_SESSION_TYPE").ok();
        let saved_wayland = std::env::var("WAYLAND_DISPLAY").ok();
        let saved_display = std::env::var("DISPLAY").ok();

        std::env::remove_var("XDG_SESSION_TYPE");
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("DISPLAY");

        let result = detect_display_server();
        assert_eq!(result, DisplayServer::Unknown);

        // Restore env vars.
        if let Some(v) = saved_session {
            std::env::set_var("XDG_SESSION_TYPE", v);
        }
        if let Some(v) = saved_wayland {
            std::env::set_var("WAYLAND_DISPLAY", v);
        }
        if let Some(v) = saved_display {
            std::env::set_var("DISPLAY", v);
        }
    }
}
