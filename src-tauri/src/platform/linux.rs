use std::process::Command;

use super::error::PlatformError;
use super::{ContextDetector, TextInjector};

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

// ---------------------------------------------------------------------------
// Internal clipboard primitives — shared pattern with Windows impl
// ---------------------------------------------------------------------------

fn clipboard_read_x11() -> Result<Option<String>, PlatformError> {
    let output = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            }
        });
    Ok(output)
}

fn clipboard_write_x11(text: &str) -> Result<(), PlatformError> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PlatformError::ToolNotFound {
            tool: format!("xclip: {e}"),
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(text.as_bytes()).map_err(|e| {
            PlatformError::ClipboardError(format!("Failed to write to xclip: {e}"))
        })?;
    }
    child
        .wait()
        .map_err(|e| PlatformError::ClipboardError(format!("xclip failed: {e}")))?;
    Ok(())
}

fn simulate_paste_x11() -> Result<(), PlatformError> {
    Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .output()
        .map_err(|e| PlatformError::InjectionFailed(format!("xdotool paste failed: {e}")))?;
    Ok(())
}

fn clipboard_read_wayland() -> Result<Option<String>, PlatformError> {
    let output = Command::new("wl-paste")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            }
        });
    Ok(output)
}

fn clipboard_write_wayland(text: &str) -> Result<(), PlatformError> {
    let mut child = Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PlatformError::ToolNotFound {
            tool: format!("wl-copy: {e}"),
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(text.as_bytes()).map_err(|e| {
            PlatformError::ClipboardError(format!("Failed to write to wl-copy: {e}"))
        })?;
    }
    child
        .wait()
        .map_err(|e| PlatformError::ClipboardError(format!("wl-copy failed: {e}")))?;
    Ok(())
}

fn simulate_paste_wayland() -> Result<(), PlatformError> {
    Command::new("wtype")
        .args(["-M", "ctrl", "v", "-m", "ctrl"])
        .output()
        .map_err(|e| PlatformError::InjectionFailed(format!("wtype paste failed: {e}")))?;
    Ok(())
}

/// Clipboard inject pattern shared across display servers:
/// save → write → paste → restore.
fn inject_via_clipboard(
    text: &str,
    read_fn: fn() -> Result<Option<String>, PlatformError>,
    write_fn: fn(&str) -> Result<(), PlatformError>,
    paste_fn: fn() -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    let saved = read_fn()?;
    write_fn(text)?;
    paste_fn()?;

    // Restore clipboard after a brief delay.
    if let Some(original) = saved {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = write_fn(&original); // best-effort restore
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// TextInjector
// ---------------------------------------------------------------------------

pub struct LinuxTextInjector {
    display: DisplayServer,
}

impl LinuxTextInjector {
    pub fn new() -> Self {
        let ds = detect_display_server();
        tracing::info!(platform = "linux", display_server = ?ds, "TextInjector initialized");
        Self { display: ds }
    }
}

impl TextInjector for LinuxTextInjector {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError> {
        let _span = tracing::info_span!("inject_text", platform = "linux").entered();

        let text = super::sanitize_for_typing(text);
        let use_clipboard = text.len() > 200;

        match self.display {
            DisplayServer::X11 => {
                if use_clipboard {
                    inject_via_clipboard(
                        &text,
                        clipboard_read_x11,
                        clipboard_write_x11,
                        simulate_paste_x11,
                    )
                } else {
                    Command::new("xdotool")
                        .args(["type", "--clearmodifiers", "--", &text])
                        .output()
                        .map_err(|e| {
                            PlatformError::InjectionFailed(format!("xdotool type failed: {e}"))
                        })?;
                    Ok(())
                }
            }
            DisplayServer::Wayland => {
                if use_clipboard {
                    inject_via_clipboard(
                        &text,
                        clipboard_read_wayland,
                        clipboard_write_wayland,
                        simulate_paste_wayland,
                    )
                } else {
                    Command::new("wtype")
                        .args(["--", &text])
                        .output()
                        .map_err(|e| {
                            PlatformError::InjectionFailed(format!("wtype failed: {e}"))
                        })?;
                    Ok(())
                }
            }
            DisplayServer::Unknown => {
                // ydotool as universal fallback.
                Command::new("ydotool")
                    .args(["type", "--", &text])
                    .output()
                    .map_err(|e| {
                        PlatformError::InjectionFailed(format!("ydotool type failed: {e}"))
                    })?;
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ContextDetector
// ---------------------------------------------------------------------------

pub struct LinuxContextDetector {
    display: DisplayServer,
}

impl LinuxContextDetector {
    pub fn new() -> Self {
        let display = detect_display_server();
        Self { display }
    }
}

impl ContextDetector for LinuxContextDetector {
    fn get_active_window_name(&self) -> Option<String> {
        let _span = tracing::info_span!("get_active_window", platform = "linux").entered();

        match self.display {
            DisplayServer::X11 => {
                let output = Command::new("xdotool")
                    .args(["getactivewindow", "getwindowname"])
                    .output()
                    .ok()?;
                if output.status.success() {
                    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                } else {
                    None
                }
            }
            DisplayServer::Wayland => {
                let output = Command::new("wlrctl")
                    .args(["toplevel", "find", "focused"])
                    .output()
                    .ok()?;
                if output.status.success() {
                    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                } else {
                    None
                }
            }
            DisplayServer::Unknown => None,
        }
    }

    fn get_selected_text(&self) -> Option<String> {
        let _span = tracing::info_span!("get_selected_text", platform = "linux").entered();

        match self.display {
            DisplayServer::X11 => {
                // Read X11 PRIMARY selection (automatically set on text selection).
                let output = Command::new("xclip")
                    .args(["-selection", "primary", "-o"])
                    .output()
                    .ok()?;
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                } else {
                    None
                }
            }
            DisplayServer::Wayland => {
                let output = Command::new("wl-paste")
                    .args(["--primary"])
                    .output()
                    .ok()?;
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                } else {
                    None
                }
            }
            DisplayServer::Unknown => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
