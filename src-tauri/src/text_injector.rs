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

/// Injects text at the current cursor position.
///
/// For short text (<200 chars), types directly.
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
        // Direct typing for short text
        Command::new("xdotool")
            .args(["type", "--clearmodifiers", "--", text])
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
        Command::new("wtype")
            .args(["--", text])
            .output()
            .map_err(|e| format!("wtype failed: {}", e))?;
        Ok(())
    }
}

fn inject_ydotool(text: &str) -> Result<(), String> {
    Command::new("ydotool")
        .args(["type", "--", text])
        .output()
        .map_err(|e| format!("ydotool type failed: {}", e))?;
    Ok(())
}
