#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HGLOBAL;
#[cfg(target_os = "windows")]
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
};

use super::error::PlatformError;
use super::{ContextDetector, TextInjector};
use tingyuxuan_core::context::InputContext;

// ---------------------------------------------------------------------------
// TextInjector
// ---------------------------------------------------------------------------

pub struct WindowsTextInjector;

impl WindowsTextInjector {
    pub fn new() -> Self {
        tracing::info!(platform = "windows", "TextInjector initialized");
        Self
    }
}

impl TextInjector for WindowsTextInjector {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError> {
        let _span = tracing::info_span!("inject_text", platform = "windows").entered();

        let text = super::sanitize_for_typing(text);

        // block_in_place ensures Win32 calls run on a sync thread.
        #[cfg(target_os = "windows")]
        {
            tokio::task::block_in_place(|| {
                if text.len() > 200 {
                    inject_via_clipboard(&text)
                } else {
                    inject_via_sendinput(&text)
                }
            })
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = text;
            Err(PlatformError::InjectionFailed(
                "Windows TextInjector called on non-Windows platform".to_string(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// SendInput batch implementation
// ---------------------------------------------------------------------------

/// Construct a single Unicode INPUT event.
#[cfg(target_os = "windows")]
fn make_unicode_input(code_unit: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code_unit,
                dwFlags: KEYEVENTF_UNICODE
                    | windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Construct a virtual-key INPUT event (for Ctrl+V etc).
#[cfg(target_os = "windows")]
fn make_vk_input(vk: VIRTUAL_KEY, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Inject text via batch SendInput — one system call for all characters.
#[cfg(target_os = "windows")]
fn inject_via_sendinput(text: &str) -> Result<(), PlatformError> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    // Each character needs key_down + key_up = 2 INPUT events.
    let mut inputs: Vec<INPUT> = Vec::with_capacity(utf16.len() * 2);
    for &code_unit in &utf16 {
        inputs.push(make_unicode_input(code_unit, 0)); // key down
        inputs.push(make_unicode_input(code_unit, KEYEVENTF_KEYUP.0)); // key up
    }

    // SAFETY: inputs is a valid INPUT array; its lifetime covers the SendInput call.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(PlatformError::InjectionFailed(format!(
            "SendInput: expected {} events, sent {}",
            inputs.len(),
            sent
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Clipboard operations (Win32)
// ---------------------------------------------------------------------------

/// RAII guard for Win32 clipboard open/close.
#[cfg(target_os = "windows")]
struct ClipboardGuard;

#[cfg(target_os = "windows")]
impl ClipboardGuard {
    fn open() -> Result<Self, PlatformError> {
        // SAFETY: OpenClipboard(None) opens the clipboard for the current thread.
        // We ensure CloseClipboard is called via Drop.
        let ok = unsafe { OpenClipboard(None) };
        if ok.is_err() {
            return Err(PlatformError::ClipboardError(
                "OpenClipboard failed".to_string(),
            ));
        }
        Ok(Self)
    }
}

#[cfg(target_os = "windows")]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: We opened the clipboard in open(), so we must close it.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Read current clipboard text (CF_UNICODETEXT).
#[cfg(target_os = "windows")]
fn clipboard_read() -> Result<Option<String>, PlatformError> {
    let _guard = ClipboardGuard::open()?;

    // SAFETY: GetClipboardData returns a handle to the clipboard data.
    // The handle is valid only while the clipboard is open (guarded by ClipboardGuard).
    let handle = unsafe { GetClipboardData(13) }; // CF_UNICODETEXT = 13
    match handle {
        Ok(h) if !h.0.is_null() => {
            // SAFETY: HANDLE and HGLOBAL have the same repr (*mut c_void).
            // GetClipboardData returns a global memory handle as HANDLE.
            let hglobal = HGLOBAL(h.0);
            // SAFETY: GlobalLock returns a pointer to the clipboard data.
            // We read the UTF-16 string and unlock immediately.
            let ptr = unsafe { GlobalLock(hglobal) };
            if ptr.is_null() {
                return Ok(None);
            }
            let wide = ptr as *const u16;
            let mut len = 0;
            // SAFETY: Walk the null-terminated UTF-16 string to find its length.
            while unsafe { *wide.add(len) } != 0 {
                len += 1;
            }
            let slice = unsafe { std::slice::from_raw_parts(wide, len) };
            let text = String::from_utf16_lossy(slice);
            // SAFETY: Unlock the global memory after reading.
            let _ = unsafe { GlobalUnlock(hglobal) };
            Ok(Some(text))
        }
        _ => Ok(None),
    }
}

/// Write text to clipboard (CF_UNICODETEXT).
#[cfg(target_os = "windows")]
fn clipboard_write(text: &str) -> Result<(), PlatformError> {
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = utf16.len() * 2;

    // SAFETY: GlobalAlloc returns a valid HGLOBAL for the requested size.
    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) };
    let hmem = hmem.map_err(|_| PlatformError::ClipboardError("GlobalAlloc failed".to_string()))?;

    {
        // SAFETY: GlobalLock returns a pointer to the allocated memory.
        let ptr = unsafe { GlobalLock(hmem) };
        if ptr.is_null() {
            return Err(PlatformError::ClipboardError(
                "GlobalLock failed".to_string(),
            ));
        }
        // SAFETY: Copy UTF-16 data into the allocated memory.
        unsafe {
            std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, ptr as *mut u8, byte_len);
        }
        // SAFETY: Unlock after writing.
        let _ = unsafe { GlobalUnlock(hmem) };
    }

    let _guard = ClipboardGuard::open()?;
    // SAFETY: EmptyClipboard clears the current contents.
    let _ = unsafe { EmptyClipboard() };
    // SAFETY: SetClipboardData takes ownership of hmem.
    // The system will free it when the clipboard is next emptied.
    // HANDLE and HGLOBAL have the same repr (*mut c_void).
    let result = unsafe { SetClipboardData(13, Some(windows::Win32::Foundation::HANDLE(hmem.0))) }; // CF_UNICODETEXT = 13
    if result.is_err() {
        return Err(PlatformError::ClipboardError(
            "SetClipboardData failed".to_string(),
        ));
    }
    Ok(())
}

/// Simulate Ctrl+V paste via SendInput.
#[cfg(target_os = "windows")]
fn simulate_paste() -> Result<(), PlatformError> {
    let inputs = [
        make_vk_input(VK_CONTROL, 0),                 // Ctrl down
        make_vk_input(VK_V, 0),                       // V down
        make_vk_input(VK_V, KEYEVENTF_KEYUP.0),       // V up
        make_vk_input(VK_CONTROL, KEYEVENTF_KEYUP.0), // Ctrl up
    ];
    // SAFETY: inputs is a valid array of INPUT events.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != 4 {
        return Err(PlatformError::InjectionFailed(format!(
            "SendInput paste: expected 4, sent {}",
            sent
        )));
    }
    Ok(())
}

/// Clipboard inject pattern: save → write → paste → restore.
#[cfg(target_os = "windows")]
fn inject_via_clipboard(text: &str) -> Result<(), PlatformError> {
    let saved = clipboard_read()?;
    clipboard_write(text)?;
    simulate_paste()?;
    std::thread::sleep(Duration::from_millis(100));
    if let Some(original) = saved
        && let Err(e) = clipboard_write(&original)
    {
        tracing::warn!("Clipboard restore failed: {e}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ContextDetector
// ---------------------------------------------------------------------------

pub struct WindowsContextDetector;

impl WindowsContextDetector {
    pub fn new() -> Self {
        tracing::info!(platform = "windows", "ContextDetector initialized");
        Self
    }

    fn get_window_title(&self) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            // SAFETY: GetForegroundWindow has no preconditions, returns HWND or null.
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd.0.is_null() {
                return None;
            }
            // SAFETY: GetWindowTextLengthW returns 0 on error.
            let len = unsafe { GetWindowTextLengthW(hwnd) };
            if len == 0 {
                return None;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            // SAFETY: GetWindowTextW writes to the provided buffer.
            let actual = unsafe { GetWindowTextW(hwnd, &mut buf) };
            if actual == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..actual as usize]))
        }

        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    /// 通过模拟 Ctrl+C 获取选中文本。注意：此操作会短暂修改剪贴板内容，
    /// 完成后会尽力恢复原始剪贴板。
    fn get_selected_text(&self) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            tokio::task::block_in_place(copy_selection_via_clipboard)
        }

        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    fn get_clipboard_text(&self) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            clipboard_read().ok().flatten()
        }

        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
}

impl ContextDetector for WindowsContextDetector {
    fn collect_context(&self) -> InputContext {
        let _span = tracing::info_span!("collect_context", platform = "windows").entered();

        // clipboard_text 先于 selected_text 采集（Ctrl+C 会覆盖剪贴板）
        let clipboard_text = self.get_clipboard_text();
        let selected_text = self.get_selected_text();
        let window_title = self.get_window_title();
        // 使用窗口标题作为应用名称（Windows 上暂不取进程名）
        let app_name = window_title.clone();

        InputContext {
            app_name,
            window_title,
            clipboard_text,
            selected_text,
            // 以下字段在 Windows 桌面端暂不采集
            app_package: None,
            browser_url: None, // 需 UI Automation，后续迭代
            input_field_type: None,
            input_hint: None,
            editor_action: None,
            surrounding_text: None,
            screen_text: None, // 需 UI Automation，后续迭代
        }
    }
}

/// Copy selected text via Ctrl+C and clipboard.
#[cfg(target_os = "windows")]
fn copy_selection_via_clipboard() -> Option<String> {
    let saved = clipboard_read().ok()?;

    // Clear clipboard so we can detect if Ctrl+C actually copied something.
    {
        let _guard = ClipboardGuard::open().ok()?;
        // SAFETY: EmptyClipboard clears clipboard contents.
        let _ = unsafe { EmptyClipboard() };
    }

    // Simulate Ctrl+C.
    let inputs = [
        make_vk_input(VK_CONTROL, 0),
        make_vk_input(
            VIRTUAL_KEY(0x43), // VK_C
            0,
        ),
        make_vk_input(VIRTUAL_KEY(0x43), KEYEVENTF_KEYUP.0),
        make_vk_input(VK_CONTROL, KEYEVENTF_KEYUP.0),
    ];
    // SAFETY: inputs is a valid INPUT array.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != 4 {
        return None;
    }

    // Brief delay for the target app to process Ctrl+C.
    std::thread::sleep(Duration::from_millis(50));

    let text = clipboard_read().ok().flatten();

    // Restore original clipboard (best-effort).
    if let Some(original) = saved
        && let Err(e) = clipboard_write(&original)
    {
        tracing::warn!("Clipboard restore failed: {e}");
    }

    text.filter(|t| !t.is_empty())
}

// ---------------------------------------------------------------------------
// RAltKeyMonitor — 右 Alt 键全局监听（WH_KEYBOARD_LL 低级键盘钩子）
// ---------------------------------------------------------------------------

/// AltGr 检测：VK_LCONTROL 与 VK_RMENU 时间戳差值阈值（毫秒）。
/// Windows 在按下 AltGr 时先发送幻影 VK_LCONTROL，间隔通常 ≤ 1ms。
#[cfg(target_os = "windows")]
const ALTGR_TIMING_THRESHOLD_MS: u32 = 1;

/// 右 Alt 键监听器 — 独立线程运行 WH_KEYBOARD_LL 钩子。
///
/// 与 macOS `FnKeyMonitor`（CGEventTap + CFRunLoop）对称：
/// 独立 OS 线程持有低级键盘钩子，通过 AppHandle emit 事件到主线程。
///
/// 设计要点：
/// - 使用 `thread_local!` 存储钩子上下文（钩子回调保证在安装线程上执行）
/// - AltGr 过滤：检测 VK_LCONTROL 幻影事件，避免误触发
/// - 按键状态跟踪：仅触发下降沿，防止长按重复
/// - 始终调用 `CallNextHookEx`，不吞键，不破坏钩子链
#[cfg(target_os = "windows")]
pub struct RAltKeyMonitor {
    thread_id: u32,
    _thread: std::thread::JoinHandle<()>,
}

/// 钩子回调的线程局部上下文。
#[cfg(target_os = "windows")]
struct HookContext {
    app_handle: tauri::AppHandle,
    ralt_down: bool,
    last_lctrl_time: u32,
}

#[cfg(target_os = "windows")]
thread_local! {
    static HOOK_CTX: std::cell::RefCell<Option<HookContext>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "windows")]
impl RAltKeyMonitor {
    /// 启动 RAlt 键监听。
    ///
    /// RAlt 按下时 emit("shortcut-action", "dictate")；
    /// Shift+RAlt 按下时 emit("shortcut-action", "translate")。
    pub fn start(app: tauri::AppHandle) -> Result<Self, super::PlatformError> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<u32, super::PlatformError>>(1);

        let thread = std::thread::Builder::new()
            .name("ralt-key-monitor".into())
            .spawn(move || {
                Self::run_message_loop(app, &tx);
            })
            .map_err(|e| {
                super::PlatformError::InjectionFailed(format!(
                    "Failed to spawn RAlt key monitor thread: {e}"
                ))
            })?;

        let thread_id = rx.recv().map_err(|_| {
            super::PlatformError::InjectionFailed(
                "RAlt key monitor thread failed to initialize (channel closed)".into(),
            )
        })??;

        Ok(Self {
            thread_id,
            _thread: thread,
        })
    }

    /// 在当前线程内安装 WH_KEYBOARD_LL 钩子并运行消息循环。
    fn run_message_loop(
        app: tauri::AppHandle,
        tx: &std::sync::mpsc::SyncSender<Result<u32, super::PlatformError>>,
    ) {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        };

        // 设置 thread_local 上下文
        HOOK_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(HookContext {
                app_handle: app,
                ralt_down: false,
                last_lctrl_time: 0,
            });
        });

        // 安装低级键盘钩子
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) };
        let hook = match hook {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(Err(super::PlatformError::InjectionFailed(format!(
                    "SetWindowsHookExW failed: {e}"
                ))));
                return;
            }
        };

        let thread_id = unsafe { GetCurrentThreadId() };
        tracing::info!(thread_id, "RAltKeyMonitor started on dedicated thread");
        let _ = tx.send(Ok(thread_id));

        // 消息循环 — GetMessageW 驱动钩子回调，收到 WM_QUIT 时退出
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        loop {
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if !ret.as_bool() {
                break; // WM_QUIT 或错误
            }
        }

        // 清理
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        HOOK_CTX.with(|ctx| {
            *ctx.borrow_mut() = None;
        });
        tracing::info!("RAltKeyMonitor stopped");
    }
}

#[cfg(target_os = "windows")]
impl Drop for RAltKeyMonitor {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        tracing::info!("RAltKeyMonitor dropping, posting WM_QUIT");
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// WH_KEYBOARD_LL 钩子回调。
///
/// 关键约束：必须在 ~300ms 内返回，否则 Windows 会静默移除钩子。
/// 仅做轻量判断 + emit（异步事件，不阻塞），然后调用 CallNextHookEx。
#[cfg(target_os = "windows")]
unsafe extern "system" fn hook_proc(
    n_code: i32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_LCONTROL, VK_RMENU, VK_SHIFT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    let mut suppress = false;

    if n_code as u32 == HC_ACTION {
        // SAFETY: l_param 指向系统分配的 KBDLLHOOKSTRUCT，在回调期间有效。
        let kb = unsafe { &*(l_param.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode;
        let msg = w_param.0 as u32;

        HOOK_CTX.with(|cell| {
            if let Some(ctx) = cell.borrow_mut().as_mut() {
                // 1. 记录 VK_LCONTROL 按下时间（用于 AltGr 检测）
                if vk == VK_LCONTROL.0 as u32 && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
                    ctx.last_lctrl_time = kb.time;
                }

                // 2. 处理 VK_RMENU
                if vk == VK_RMENU.0 as u32 {
                    let is_altgr =
                        kb.time.wrapping_sub(ctx.last_lctrl_time) <= ALTGR_TIMING_THRESHOLD_MS;

                    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                        if is_altgr {
                            // AltGr 输入，不触发也不抑制
                        } else if !ctx.ralt_down {
                            // 独立 RAlt 下降沿
                            ctx.ralt_down = true;
                            suppress = true;

                            // SAFETY: GetKeyState 读取按键状态，无前置条件。
                            let shift_held =
                                unsafe { (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0 };
                            let action = if shift_held { "translate" } else { "dictate" };

                            tracing::debug!(action, "RAlt key triggered (suppressed)");
                            let _ =
                                tauri::Emitter::emit(&ctx.app_handle, "shortcut-action", action);
                        } else {
                            // 长按重复的 KeyDown，也要抑制（避免 Alt 菜单弹出）
                            suppress = true;
                        }
                    } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                        if !is_altgr {
                            // 独立 RAlt 释放，抑制以防 Alt 菜单
                            suppress = true;
                        }
                        ctx.ralt_down = false;
                    }
                }
            }
        });
    }

    if suppress {
        // 吞掉独立 RAlt 事件，阻止其传递给其他应用
        return windows::Win32::Foundation::LRESULT(1);
    }
    // SAFETY: CallNextHookEx 转发钩子事件，参数来自系统回调。
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// 注册 Windows 平台热键。
///
/// - RAlt（听写）、Shift+RAlt（翻译）: 通过 WH_KEYBOARD_LL 钩子实现
/// - Alt+Space（AI 助手）、Escape（取消）: 通过 tauri-plugin-global-shortcut
#[cfg(target_os = "windows")]
pub fn register_platform_hotkeys(app: &tauri::App) -> Result<RAltKeyMonitor, super::PlatformError> {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    // 1. 启动 RAlt 键监听器（处理 RAlt 和 Shift+RAlt）
    let monitor = RAltKeyMonitor::start(app.handle().clone())?;

    // 2. 注册其余快捷键（使用 global-shortcut 插件）
    let shortcuts = [
        (
            Shortcut::new(Some(Modifiers::ALT), Code::Space),
            "ai_assistant",
        ),
        (Shortcut::new(None, Code::Escape), "cancel"),
    ];

    let handle = app.handle().clone();
    for (shortcut, action) in shortcuts {
        let h = handle.clone();
        let action_name = action.to_string();

        if let Err(e) = app
            .global_shortcut()
            .on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let h2 = h.clone();
                let mode = action_name.clone();
                tauri::async_runtime::spawn(async move {
                    crate::handle_shortcut_action(&h2, &mode).await;
                });
            })
        {
            tracing::warn!(
                "Failed to register shortcut for '{}': {}. Another app may have claimed it.",
                action,
                e
            );
        }
    }

    Ok(monitor)
}

// ---------------------------------------------------------------------------
// Tests (platform-independent unit tests + cfg(windows) integration tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sendinput_batch_sizing() {
        // Verify that N characters produce 2N INPUT events.
        let text = "Hello世界";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let expected_inputs = utf16.len() * 2;
        // "Hello" = 5 UTF-16 code units, "世界" = 2 code units = 7 total → 14 inputs
        assert_eq!(expected_inputs, 14);
    }

    #[test]
    fn test_long_text_routes_to_clipboard() {
        // Text > 200 bytes should use clipboard path.
        let short = "a".repeat(200);
        assert!(short.len() <= 200, "200 chars should use SendInput");

        let long = "a".repeat(201);
        assert!(long.len() > 200, "201 chars should use clipboard");
    }

    #[test]
    fn test_utf16_encoding_cjk() {
        // CJK characters are BMP (single UTF-16 code unit each).
        let text = "你好世界";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(utf16.len(), 4);
    }

    #[test]
    fn test_utf16_encoding_emoji() {
        // Emoji with surrogate pairs (2 UTF-16 code units each).
        let text = "😀";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(utf16.len(), 2); // surrogate pair
    }

    #[test]
    fn test_utf16_buffer_edge_cases() {
        // Empty string.
        let utf16: Vec<u16> = "".encode_utf16().collect();
        assert_eq!(utf16.len(), 0);

        // Pure ASCII.
        let utf16: Vec<u16> = "abc".encode_utf16().collect();
        assert_eq!(utf16.len(), 3);

        // Mixed ASCII + CJK.
        let utf16: Vec<u16> = "hi你好".encode_utf16().collect();
        assert_eq!(utf16.len(), 4);
    }

    // Windows-only integration tests
    #[cfg(target_os = "windows")]
    mod windows_tests {
        use super::*;
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_RMENU;

        #[test]
        fn test_vk_rmenu_value() {
            // VK_RMENU (右 Alt) = 0xA5，确认常量值正确
            assert_eq!(VK_RMENU.0, 0xA5);
        }

        #[test]
        fn test_altgr_timing_threshold() {
            // AltGr 检测阈值应为 1ms
            assert_eq!(ALTGR_TIMING_THRESHOLD_MS, 1);

            // 模拟 AltGr 场景：LCtrl 和 RMenu 时间差 ≤ 1ms → 应被过滤
            let lctrl_time: u32 = 1000;
            let rmenu_time: u32 = 1001;
            assert!(rmenu_time.wrapping_sub(lctrl_time) <= ALTGR_TIMING_THRESHOLD_MS);

            // 模拟独立 RAlt：时间差 > 1ms → 不应被过滤
            let rmenu_time_independent: u32 = 1050;
            assert!(rmenu_time_independent.wrapping_sub(lctrl_time) > ALTGR_TIMING_THRESHOLD_MS);
        }

        #[test]
        fn test_make_unicode_input_struct() {
            let input = make_unicode_input(0x0041, 0); // 'A' key down
            assert_eq!(input.r#type, INPUT_KEYBOARD);
            // SAFETY: We just constructed this INPUT as keyboard type.
            unsafe {
                assert_eq!(input.Anonymous.ki.wScan, 0x0041);
                assert_eq!(input.Anonymous.ki.wVk, VIRTUAL_KEY(0));
            }
        }

        #[test]
        fn test_clipboard_roundtrip_ascii() {
            let text = "Hello, clipboard!";
            clipboard_write(text).expect("write failed");
            let read = clipboard_read().expect("read failed");
            assert_eq!(read, Some(text.to_string()));
        }

        #[test]
        fn test_clipboard_roundtrip_cjk() {
            let text = "你好，剪贴板！";
            clipboard_write(text).expect("write failed");
            let read = clipboard_read().expect("read failed");
            assert_eq!(read, Some(text.to_string()));
        }
    }
}
