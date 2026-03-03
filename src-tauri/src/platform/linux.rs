use std::process::Command;
use std::time::Duration;

use super::error::PlatformError;
use super::{ContextDetector, TextInjector, run_with_timeout};
use tingyuxuan_core::context::InputContext;

/// Detects the current display server type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

pub fn detect_display_server() -> DisplayServer {
    detect_from_session_type()
        .or_else(detect_from_display_vars)
        .unwrap_or(DisplayServer::Unknown)
}

fn detect_from_session_type() -> Option<DisplayServer> {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok()?;
    match session_type.to_lowercase().as_str() {
        "x11" => Some(DisplayServer::X11),
        "wayland" => Some(DisplayServer::Wayland),
        _ => None,
    }
}

fn detect_from_display_vars() -> Option<DisplayServer> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Some(DisplayServer::Wayland);
    }
    std::env::var("DISPLAY").ok().map(|_| DisplayServer::X11)
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
    use std::io::Write;

    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PlatformError::ToolNotFound {
            tool: format!("xclip: {e}"),
        })?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| PlatformError::ClipboardError("xclip stdin not available".into()))?
        .write_all(text.as_bytes())
        .map_err(|e| PlatformError::ClipboardError(format!("Failed to write to xclip: {e}")))?;

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
    let output = Command::new("wl-paste").output().ok().and_then(|o| {
        if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).to_string())
        } else {
            None
        }
    });
    Ok(output)
}

fn clipboard_write_wayland(text: &str) -> Result<(), PlatformError> {
    use std::io::Write;

    let mut child = Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PlatformError::ToolNotFound {
            tool: format!("wl-copy: {e}"),
        })?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| PlatformError::ClipboardError("wl-copy stdin not available".into()))?
        .write_all(text.as_bytes())
        .map_err(|e| PlatformError::ClipboardError(format!("Failed to write to wl-copy: {e}")))?;

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

// inject_via_clipboard 已提取到 mod.rs 作为跨平台共享函数
use super::inject_via_clipboard;

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
        if text.len() > 200 {
            return self.inject_clipboard_mode(&text);
        }
        self.inject_direct_mode(&text)
    }
}

impl LinuxTextInjector {
    fn inject_clipboard_mode(&self, text: &str) -> Result<(), PlatformError> {
        match self.display {
            DisplayServer::X11 => inject_via_clipboard(
                text,
                clipboard_read_x11,
                clipboard_write_x11,
                simulate_paste_x11,
            ),
            DisplayServer::Wayland => inject_via_clipboard(
                text,
                clipboard_read_wayland,
                clipboard_write_wayland,
                simulate_paste_wayland,
            ),
            DisplayServer::Unknown => type_with_tool("ydotool", &["type", "--", text], "ydotool"),
        }
    }

    fn inject_direct_mode(&self, text: &str) -> Result<(), PlatformError> {
        match self.display {
            DisplayServer::X11 => {
                type_with_tool("xdotool", &["type", "--clearmodifiers", "--", text], "xdotool")
            }
            DisplayServer::Wayland => type_with_tool("wtype", &["--", text], "wtype"),
            DisplayServer::Unknown => type_with_tool("ydotool", &["type", "--", text], "ydotool"),
        }
    }
}

fn type_with_tool(cmd: &str, args: &[&str], tool: &str) -> Result<(), PlatformError> {
    Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| PlatformError::InjectionFailed(format!("{tool} type failed: {e}")))?;
    Ok(())
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

    /// 上下文采集超时（每个子进程调用）。
    const CTX_TIMEOUT: Duration = Duration::from_millis(200);

    /// 获取活动窗口标题（_NET_WM_NAME）
    fn get_window_title(&self) -> Option<String> {
        match self.display {
            DisplayServer::X11 => run_with_timeout(
                Command::new("xdotool").args(["getactivewindow", "getwindowname"]),
                Self::CTX_TIMEOUT,
            ),
            DisplayServer::Wayland => run_with_timeout(
                Command::new("wlrctl").args(["toplevel", "find", "focused"]),
                Self::CTX_TIMEOUT,
            ),
            DisplayServer::Unknown => None,
        }
    }

    /// 获取应用名称（WM_CLASS）
    fn get_app_name(&self) -> Option<String> {
        match self.display {
            DisplayServer::X11 => run_with_timeout(
                Command::new("xdotool").args(["getactivewindow", "getwindowclassname"]),
                Self::CTX_TIMEOUT,
            )
            .or_else(|| self.get_window_title()),
            DisplayServer::Wayland => self.get_window_title(),
            DisplayServer::Unknown => None,
        }
    }

    /// 读取 PRIMARY selection（选中文本）
    fn get_selected_text(&self) -> Option<String> {
        match self.display {
            DisplayServer::X11 => run_with_timeout(
                Command::new("xclip").args(["-selection", "primary", "-o"]),
                Self::CTX_TIMEOUT,
            ),
            DisplayServer::Wayland => run_with_timeout(
                Command::new("wl-paste").args(["--primary"]),
                Self::CTX_TIMEOUT,
            ),
            DisplayServer::Unknown => None,
        }
    }

    /// 读取 CLIPBOARD（剪贴板内容）
    fn get_clipboard_text(&self) -> Option<String> {
        match self.display {
            DisplayServer::X11 => clipboard_read_x11().ok().flatten(),
            DisplayServer::Wayland => clipboard_read_wayland().ok().flatten(),
            DisplayServer::Unknown => None,
        }
    }
}

impl ContextDetector for LinuxContextDetector {
    fn collect_context(&self) -> InputContext {
        let _span = tracing::info_span!("collect_context", platform = "linux").entered();

        // 各项采集独立并行执行（每项最多 200ms），总耗时从最坏 800ms 降至 ~200ms
        let (clipboard_text, selected_text, app_name, window_title) = std::thread::scope(|s| {
            let h_clip = s.spawn(|| self.get_clipboard_text());
            let h_sel = s.spawn(|| self.get_selected_text());
            let h_app = s.spawn(|| self.get_app_name());
            let h_title = s.spawn(|| self.get_window_title());

            (
                h_clip.join().ok().flatten(),
                h_sel.join().ok().flatten(),
                h_app.join().ok().flatten(),
                h_title.join().ok().flatten(),
            )
        });

        InputContext {
            app_name,
            window_title,
            clipboard_text,
            selected_text,
            // 以下字段在 Linux 桌面端暂不采集
            app_package: None,
            browser_url: None, // 需浏览器扩展，后续迭代
            input_field_type: None,
            input_hint: None,
            editor_action: None,
            surrounding_text: None,
            screen_text: None, // 需 AT-SPI2，后续迭代
        }
    }
}

// ---------------------------------------------------------------------------
// 权限检测
// ---------------------------------------------------------------------------

/// 检测 Linux 平台权限状态。
///
/// Linux 上只需检测麦克风权限（通过 cpal 探测设备）。
/// 辅助功能和输入监控概念不适用 Linux，始终返回 Granted。
pub fn check_permissions() -> super::PermissionReport {
    use tingyuxuan_core::audio::recorder::AudioRecorder;

    let mic = match AudioRecorder::probe_microphone() {
        Ok(()) => super::PermissionState::Granted,
        Err(_) => super::PermissionState::Denied,
    };
    super::PermissionReport {
        all_granted: mic == super::PermissionState::Granted,
        microphone: mic,
        accessibility: super::PermissionState::Granted,
        input_monitoring: super::PermissionState::Granted,
    }
}

/// 打开 Linux 音频设置。
pub fn open_permission_settings_for(_target: Option<&str>) {
    // 优先尝试 pavucontrol（PulseAudio 控制），回退到 GNOME 设置
    if Command::new("pavucontrol").spawn().is_err() {
        let _ = Command::new("gnome-control-center").arg("sound").spawn();
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

        // SAFETY: This test runs single-threaded; no concurrent env access.
        unsafe {
            std::env::remove_var("XDG_SESSION_TYPE");
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("DISPLAY");
        }

        let result = detect_display_server();
        assert_eq!(result, DisplayServer::Unknown);

        // Restore env vars.
        // SAFETY: This test runs single-threaded; no concurrent env access.
        unsafe {
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
}
