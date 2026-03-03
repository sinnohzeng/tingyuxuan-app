mod error;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub use error::PlatformError;

use tingyuxuan_core::context::InputContext;

/// Text injection trait — inject text at the current cursor position.
pub trait TextInjector: Send + Sync {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError>;
}

/// Context detection trait — 采集当前输入上下文。
///
/// 整体超时 200ms，单项失败不阻塞其他字段。
pub trait ContextDetector: Send + Sync {
    fn collect_context(&self) -> InputContext;
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

// ---------------------------------------------------------------------------
// 跨平台共享工具函数
// ---------------------------------------------------------------------------

/// Run a command with a timeout. Returns None on failure or timeout.
///
/// 使用轮询方式等待子进程完成（10ms 间隔），超时后 kill。
/// `std::process::Child` 尚未稳定 `wait_timeout`，这是可移植的回退方案。
#[cfg(target_os = "linux")]
pub(crate) fn run_with_timeout(
    cmd: &mut std::process::Command,
    timeout: std::time::Duration,
) -> Option<String> {
    let mut child = spawn_subprocess(cmd)?;

    let start = std::time::Instant::now();
    loop {
        if let Some(done) = poll_subprocess(&mut child)? {
            return done;
        }
        if start.elapsed() > timeout {
            kill_subprocess(&mut child, timeout);
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn spawn_subprocess(cmd: &mut std::process::Command) -> Option<std::process::Child> {
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}

#[cfg(target_os = "linux")]
fn poll_subprocess(child: &mut std::process::Child) -> Option<Option<Option<String>>> {
    match child.try_wait() {
        Ok(Some(status)) => Some(Some(read_subprocess_output(child, status.success()))),
        Ok(None) => Some(None),
        Err(_) => None,
    }
}

#[cfg(target_os = "linux")]
fn read_subprocess_output(child: &mut std::process::Child, success: bool) -> Option<String> {
    use std::io::Read;

    if !success {
        return None;
    }

    let mut buffer = String::new();
    child.stdout.as_mut()?.read_to_string(&mut buffer).ok()?;
    let text = buffer.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(target_os = "linux")]
fn kill_subprocess(child: &mut std::process::Child, timeout: std::time::Duration) {
    let _ = child.kill();
    let _ = child.wait();
    tracing::warn!("Subprocess timed out after {:?}", timeout);
}

/// 剪贴板恢复延迟 — 等待目标应用处理粘贴后再恢复原始剪贴板内容。
#[cfg(any(target_os = "linux", target_os = "macos"))]
const CLIPBOARD_RESTORE_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// 剪贴板注入通用流程: save → write → paste → restore。
///
/// Linux 和 macOS 都使用这个函数指针模式将平台特定的剪贴板操作组合起来。
/// Windows 使用 Win32 API 直接实现，不走此路径。
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn inject_via_clipboard(
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
        std::thread::sleep(CLIPBOARD_RESTORE_DELAY);
        if let Err(e) = write_fn(&original) {
            tracing::warn!("Clipboard restore failed: {e}");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 平台统一接口类型
// ---------------------------------------------------------------------------

/// 快捷键显示标签（用于托盘菜单等 UI 展示）。
#[allow(dead_code)] // cancel 字段在 tray 中暂未单独显示
pub struct ShortcutLabels {
    pub dictate: &'static str,
    pub translate: &'static str,
    pub ai_assistant: &'static str,
    pub cancel: &'static str,
}

/// 平台权限状态（macOS 细分为辅助功能 + 输入监控两个独立权限）。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // 仅在 macOS 上使用，但类型定义需跨平台可用
pub enum PermissionStatus {
    /// 所有必要权限已授予
    Granted,
    /// 仅缺少辅助功能权限
    AccessibilityRequired,
    /// 仅缺少输入监控权限
    InputMonitoringRequired,
    /// 两项权限都需要
    BothRequired,
}

#[allow(dead_code)]
impl PermissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::AccessibilityRequired => "accessibility_required",
            Self::InputMonitoringRequired => "input_monitoring_required",
            Self::BothRequired => "both_required",
        }
    }
}

/// 全平台权限检测报告。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionReport {
    /// 所有必要权限是否已全部授予。
    pub all_granted: bool,
    /// 麦克风权限（全平台）。
    pub microphone: PermissionState,
    /// 辅助功能权限（仅 macOS，其他平台始终 Granted）。
    pub accessibility: PermissionState,
    /// 输入监控权限（仅 macOS，其他平台始终 Granted）。
    pub input_monitoring: PermissionState,
}

/// 单项权限的状态。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Unknown 暂未使用，保留以兼容未来平台
pub enum PermissionState {
    Granted,
    Denied,
    Unknown,
}

/// 获取当前平台的快捷键显示标签。
#[allow(dead_code)] // 预留给托盘/设置页展示快捷键文案
pub fn shortcut_labels() -> ShortcutLabels {
    #[cfg(target_os = "macos")]
    {
        macos::shortcut_labels()
    }

    #[cfg(not(target_os = "macos"))]
    {
        ShortcutLabels {
            dictate: "RAlt",
            translate: "Shift+RAlt",
            ai_assistant: "Alt+Space",
            cancel: "Esc",
        }
    }
}

// ---------------------------------------------------------------------------
// Compile-time type aliases — zero overhead, no Box/dyn
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub type PlatformInjector = linux::LinuxTextInjector;
#[cfg(target_os = "linux")]
pub type PlatformDetector = linux::LinuxContextDetector;

#[cfg(target_os = "macos")]
pub type PlatformInjector = macos::MacOSTextInjector;
#[cfg(target_os = "macos")]
pub type PlatformDetector = macos::MacOSContextDetector;

#[cfg(target_os = "windows")]
pub type PlatformInjector = windows::WindowsTextInjector;
#[cfg(target_os = "windows")]
pub type PlatformDetector = windows::WindowsContextDetector;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Unsupported platform: only Linux, macOS, and Windows are supported for desktop builds"
);

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
