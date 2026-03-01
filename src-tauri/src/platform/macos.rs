use std::process::Command;
use std::time::Duration;

use super::error::PlatformError;
use super::{ContextDetector, TextInjector, inject_via_clipboard};
use tingyuxuan_core::context::InputContext;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

/// 直接输入与剪贴板注入的分界长度（字节）。
/// 短文本（<= 200 字节）通过 CGEvent 直接输入，不扰动剪贴板。
const DIRECT_INPUT_THRESHOLD: usize = 200;

/// CGEvent 单次可携带的最大 UTF-16 code unit 数。
/// `CGEventKeyboardSetUnicodeString` 的硬限制为 20。
const MAX_UNICODE_PER_EVENT: usize = 20;

/// Cmd+C 处理延迟 — 等待目标应用响应（仅 AX fallback 路径）。
const CMD_C_PROCESS_DELAY: Duration = Duration::from_millis(50);

/// 上下文采集超时（仅 AX 查询 fallback 时使用）。
#[allow(dead_code)]
const CONTEXT_TIMEOUT: Duration = Duration::from_millis(200);

// macOS virtual key codes (from Events.h / HIToolbox)
const KVK_ANSI_V: CGKeyCode = 0x09;
const KVK_ANSI_C: CGKeyCode = 0x08;

// ---------------------------------------------------------------------------
// AXUIElement FFI 绑定 — macOS Accessibility API
// ---------------------------------------------------------------------------

mod ax {
    use std::ffi::c_void;

    pub type AXError = i32;
    pub const AX_ERROR_SUCCESS: AXError = 0;
    pub type AXUIElementRef = *mut c_void;

    // CFTypeRef / CFStringRef 都是 *const c_void
    pub type CFTypeRef = *const c_void;
    pub type CFStringRef = *const c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        pub fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        pub fn AXIsProcessTrusted() -> bool;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        pub fn CGPreflightListenEventAccess() -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub fn CFRelease(cf: CFTypeRef);
        pub fn CFGetTypeID(cf: CFTypeRef) -> u64;
        pub fn CFStringGetTypeID() -> u64;
    }

    /// RAII wrapper for CFTypeRef — 自动调用 CFRelease。
    pub struct OwnedCFRef(pub *mut c_void);

    impl Drop for OwnedCFRef {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CFRelease(self.0 as CFTypeRef);
                }
            }
        }
    }

    /// 从 AXUIElement 读取字符串属性。
    ///
    /// 使用 core_foundation::string::CFString 进行属性名构造和结果提取，
    /// 避免手动 UTF-8/UTF-16 转换。
    pub fn ax_get_string_attr(element: AXUIElementRef, attr_name: &str) -> Option<String> {
        use core_foundation::base::TCFType;
        use core_foundation::string::CFString;

        if element.is_null() {
            return None;
        }

        let attr_cf = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null();

        let err = unsafe {
            AXUIElementCopyAttributeValue(
                element,
                attr_cf.as_concrete_TypeRef() as CFStringRef,
                &mut value,
            )
        };

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let _owned = OwnedCFRef(value as *mut c_void);

        // 验证返回值是 CFString 类型
        let type_id = unsafe { CFGetTypeID(value) };
        let string_type_id = unsafe { CFStringGetTypeID() };
        if type_id != string_type_id {
            return None;
        }

        // 安全地转换为 CFString（不增加引用计数，OwnedCFRef 负责释放）
        let cf_str: CFString =
            unsafe { TCFType::wrap_under_get_rule(value as core_foundation::string::CFStringRef) };
        let result = cf_str.to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// 从 AXUIElement 读取子元素引用属性（返回 AXUIElementRef）。
    pub fn ax_get_element_attr(element: AXUIElementRef, attr_name: &str) -> Option<OwnedCFRef> {
        use core_foundation::base::TCFType;
        use core_foundation::string::CFString;

        if element.is_null() {
            return None;
        }

        let attr_cf = CFString::new(attr_name);
        let mut value: CFTypeRef = std::ptr::null();

        let err = unsafe {
            AXUIElementCopyAttributeValue(
                element,
                attr_cf.as_concrete_TypeRef() as CFStringRef,
                &mut value,
            )
        };

        if err != AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        Some(OwnedCFRef(value as *mut c_void))
    }
}

// ---------------------------------------------------------------------------
// 剪贴板操作 — arboard（原生 NSPasteboard）
// ---------------------------------------------------------------------------

fn clipboard_read() -> Result<Option<String>, PlatformError> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| PlatformError::ClipboardError(format!("Failed to access clipboard: {e}")))?;
    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => Ok(Some(text)),
        Ok(_) => Ok(None),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(PlatformError::ClipboardError(format!(
            "Clipboard read failed: {e}"
        ))),
    }
}

fn clipboard_write(text: &str) -> Result<(), PlatformError> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| PlatformError::ClipboardError(format!("Failed to access clipboard: {e}")))?;
    clipboard
        .set_text(text)
        .map_err(|e| PlatformError::ClipboardError(format!("Clipboard write failed: {e}")))
}

// ---------------------------------------------------------------------------
// CGEvent 直接文本输入 — 短文本不经过剪贴板
// ---------------------------------------------------------------------------

/// 通过 CGEvent 直接输入 Unicode 文本（不经过剪贴板）。
///
/// 每个 CGEvent 最多携带 20 个 UTF-16 code unit（`CGEventKeyboardSetUnicodeString` 限制）。
/// 长文本自动分块，每块发送 key-down + key-up 事件对。
fn type_text_directly(text: &str) -> Result<(), PlatformError> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PlatformError::InjectionFailed("Failed to create CGEventSource".into()))?;

    let utf16: Vec<u16> = text.encode_utf16().collect();

    for chunk in utf16.chunks(MAX_UNICODE_PER_EVENT) {
        let key_down = CGEvent::new_keyboard_event(source.clone(), 0, true).map_err(|_| {
            PlatformError::InjectionFailed("Failed to create key down event".into())
        })?;
        key_down.set_string_from_utf16_unchecked(chunk);
        key_down.post(CGEventTapLocation::HID);

        let key_up = CGEvent::new_keyboard_event(source.clone(), 0, false)
            .map_err(|_| PlatformError::InjectionFailed("Failed to create key up event".into()))?;
        key_up.post(CGEventTapLocation::HID);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CGEvent 按键模拟 — 用于 Cmd+V 粘贴和 Cmd+C 复制
// ---------------------------------------------------------------------------

/// 通过 CGEvent 模拟 Cmd+V 粘贴。
fn simulate_cmd_v() -> Result<(), PlatformError> {
    simulate_key_with_cmd(KVK_ANSI_V)
}

/// 通过 CGEvent 模拟 Cmd+C 复制（用于获取选中文本的 fallback 路径）。
fn simulate_cmd_c() -> Result<(), PlatformError> {
    simulate_key_with_cmd(KVK_ANSI_C)
}

/// 模拟 Cmd+key 按键组合。
fn simulate_key_with_cmd(keycode: CGKeyCode) -> Result<(), PlatformError> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PlatformError::InjectionFailed("Failed to create CGEventSource".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .map_err(|_| PlatformError::InjectionFailed("Failed to create key down event".into()))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, keycode, false)
        .map_err(|_| PlatformError::InjectionFailed("Failed to create key up event".into()))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

// ---------------------------------------------------------------------------
// TextInjector — 短文本直接输入 / 长文本剪贴板注入
// ---------------------------------------------------------------------------

pub struct MacOSTextInjector;

impl MacOSTextInjector {
    pub fn new() -> Self {
        tracing::info!(platform = "macos", "TextInjector initialized");
        Self
    }
}

impl TextInjector for MacOSTextInjector {
    fn inject_text(&self, text: &str) -> Result<(), PlatformError> {
        let _span = tracing::info_span!("inject_text", platform = "macos").entered();

        let text = super::sanitize_for_typing(text);

        if text.len() <= DIRECT_INPUT_THRESHOLD {
            // 短文本：CGEvent 直接输入，不扰动剪贴板
            type_text_directly(&text)
        } else {
            // 长文本：剪贴板注入（save → write → Cmd+V → restore）
            inject_via_clipboard(&text, clipboard_read, clipboard_write, simulate_cmd_v)
        }
    }
}

// ---------------------------------------------------------------------------
// ContextDetector — AXUIElement 原生上下文采集
// ---------------------------------------------------------------------------

pub struct MacOSContextDetector;

impl MacOSContextDetector {
    pub fn new() -> Self {
        tracing::info!(
            platform = "macos",
            "ContextDetector initialized (AXUIElement)"
        );
        Self
    }

    /// 获取当前前台应用名称。
    /// AXUIElement: system-wide → kAXFocusedApplicationAttribute → kAXTitleAttribute
    fn get_app_name(&self) -> Option<String> {
        let system_wide = unsafe { ax::AXUIElementCreateSystemWide() };
        if system_wide.is_null() {
            return None;
        }
        let _sys = ax::OwnedCFRef(system_wide);
        let focused_app = ax::ax_get_element_attr(system_wide, "AXFocusedApplication")?;
        ax::ax_get_string_attr(focused_app.0, "AXTitle")
    }

    /// 获取当前前台窗口标题。
    /// AXUIElement: focused app → kAXFocusedWindowAttribute → kAXTitleAttribute
    fn get_window_title(&self) -> Option<String> {
        let system_wide = unsafe { ax::AXUIElementCreateSystemWide() };
        if system_wide.is_null() {
            return None;
        }
        let _sys = ax::OwnedCFRef(system_wide);
        let focused_app = ax::ax_get_element_attr(system_wide, "AXFocusedApplication")?;
        let focused_window = ax::ax_get_element_attr(focused_app.0, "AXFocusedWindow")?;
        ax::ax_get_string_attr(focused_window.0, "AXTitle")
    }

    /// 读取剪贴板内容（唯一需要读剪贴板的场景）。
    fn get_clipboard_text(&self) -> Option<String> {
        clipboard_read().ok().flatten()
    }

    /// 获取选中文本 — 优先 AXUIElement 直接读取，失败则 fallback 到 Cmd+C。
    ///
    /// AXUIElement 路径：focused app → kAXFocusedUIElementAttribute → kAXSelectedTextAttribute
    /// 完全不碰剪贴板，消除了与 get_clipboard_text 之间的竞态。
    fn get_selected_text(&self) -> Option<String> {
        // 首先尝试 AXUIElement 直接读取
        if let Some(text) = self.get_selected_text_via_ax() {
            return Some(text);
        }

        // Fallback: 模拟 Cmd+C（仅在 AX 查询失败时使用）
        tracing::debug!("AX selected text query failed, falling back to Cmd+C");
        self.get_selected_text_via_cmd_c()
    }

    /// AXUIElement 直接读取选中文本。
    fn get_selected_text_via_ax(&self) -> Option<String> {
        let system_wide = unsafe { ax::AXUIElementCreateSystemWide() };
        if system_wide.is_null() {
            return None;
        }
        let _sys = ax::OwnedCFRef(system_wide);
        let focused_app = ax::ax_get_element_attr(system_wide, "AXFocusedApplication")?;
        let focused_elem = ax::ax_get_element_attr(focused_app.0, "AXFocusedUIElement")?;
        ax::ax_get_string_attr(focused_elem.0, "AXSelectedText")
    }

    /// Fallback: 通过模拟 Cmd+C 获取选中文本。
    /// 流程: 保存剪贴板 → 清空 → Cmd+C → 读取 → 恢复。
    fn get_selected_text_via_cmd_c(&self) -> Option<String> {
        let saved = clipboard_read().ok()?;

        // 清空剪贴板
        let _ = clipboard_write("");

        // 模拟 Cmd+C
        simulate_cmd_c().ok()?;

        // 等待目标应用处理
        std::thread::sleep(CMD_C_PROCESS_DELAY);

        let text = clipboard_read().ok().flatten();

        // 恢复原始剪贴板（尽力而为）
        if let Some(original) = saved
            && let Err(e) = clipboard_write(&original)
        {
            tracing::warn!("Clipboard restore failed: {e}");
        }

        text.filter(|t| !t.is_empty())
    }
}

impl ContextDetector for MacOSContextDetector {
    fn collect_context(&self) -> InputContext {
        let _span = tracing::info_span!("collect_context", platform = "macos").entered();

        // AXUIElement 查询极快（<1ms 每个），全同步执行。
        // 仅 clipboard_text 可能有 IO 开销（arboard 访问 NSPasteboard），但仍然 <1ms。
        let app_name = self.get_app_name();
        let window_title = self.get_window_title();
        let selected_text = self.get_selected_text();
        let clipboard_text = self.get_clipboard_text();

        InputContext {
            app_name,
            window_title,
            clipboard_text,
            selected_text,
            // 以下字段在 macOS 桌面端暂不采集
            app_package: None,
            browser_url: None, // 需浏览器扩展，后续迭代
            input_field_type: None,
            input_hint: None,
            editor_action: None,
            surrounding_text: None,
            screen_text: None, // AXUIElement 已就绪，后续迭代可用
        }
    }
}

// ---------------------------------------------------------------------------
// FnKeyMonitor — Fn 键全局监听（CGEventTap + CFRunLoop）
// ---------------------------------------------------------------------------

/// Fn 键监听器 — 独立线程运行 CGEventTap + CFRunLoop。
///
/// 遵循 RecorderActor 的 handle/thread 分离模式：
/// 独立 OS 线程持有 CGEventTap，通过 AppHandle emit 事件到主线程。
///
/// Fn 键使用 Toggle 语义（单击开始/单击结束），与 Linux/Windows 的 RAlt 行为一致。
///
/// 改进点（相比旧实现）：
/// - Barrier 同步替代 sleep(50ms)，保证线程就绪
/// - 正确存储监听线程的 CFRunLoop（非主线程的）
/// - Drop 时调用 run_loop.stop() 优雅停止
/// - 自动检测 TapDisabledByTimeout/TapDisabledByUserInput 并重新启用
pub struct FnKeyMonitor {
    /// 监听线程的 CFRunLoop 引用，用于 Drop 时停止
    run_loop: std::sync::Arc<std::sync::Mutex<Option<core_foundation::runloop::CFRunLoop>>>,
    _thread: std::thread::JoinHandle<()>,
}

impl FnKeyMonitor {
    /// Fn flag 在 CGEventFlags 中的位掩码 (NX_SECONDARYFNMASK)
    const FN_FLAG_MASK: u64 = 0x0080_0000;

    /// 启动 Fn 键监听。Fn 按下时 emit("shortcut-action", "dictate")。
    ///
    /// 需要 Input Monitoring 权限（系统设置 > 隐私与安全性 > 输入监控）。
    ///
    /// 注意：CGEventTap 和 CFRunLoopSource 包含原始指针（!Send），
    /// 必须在监听线程内创建，不能跨线程移动。通过 channel 传递初始化结果。
    pub fn start(app: tauri::AppHandle) -> Result<Self, PlatformError> {
        use std::sync::{Arc, Mutex};

        // 在线程间传递 RunLoop 引用
        let run_loop_holder: Arc<Mutex<Option<core_foundation::runloop::CFRunLoop>>> =
            Arc::new(Mutex::new(None));
        let run_loop_for_thread = run_loop_holder.clone();

        // Channel: 线程初始化成功/失败后通知调用者
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), PlatformError>>(1);

        let thread = std::thread::Builder::new()
            .name("fn-key-monitor".into())
            .spawn(move || {
                // CGEventTap 是 !Send，必须在监听线程内创建
                if let Err(e) = Self::run_event_loop(app, run_loop_for_thread, &tx) {
                    let _ = tx.send(Err(e));
                }
            })
            .map_err(|e| {
                PlatformError::InjectionFailed(format!(
                    "Failed to spawn Fn key monitor thread: {e}"
                ))
            })?;

        // 等待线程初始化完成
        let result = rx.recv().map_err(|_| {
            PlatformError::InjectionFailed(
                "Fn key monitor thread failed to initialize (channel closed)".into(),
            )
        })?;
        result?;

        Ok(Self {
            run_loop: run_loop_holder,
            _thread: thread,
        })
    }

    /// 在当前线程内创建 CGEventTap 并运行 CFRunLoop。
    ///
    /// 所有 !Send 资源（CGEventTap、CFRunLoopSource）在此函数内创建和消费，
    /// 不跨线程边界。
    fn run_event_loop(
        app: tauri::AppHandle,
        run_loop_holder: std::sync::Arc<
            std::sync::Mutex<Option<core_foundation::runloop::CFRunLoop>>,
        >,
        tx: &std::sync::mpsc::SyncSender<Result<(), PlatformError>>,
    ) -> Result<(), PlatformError> {
        use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
        use core_graphics::event::{
            CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        };
        use std::sync::atomic::{AtomicBool, Ordering};

        let fn_pressed = std::sync::Arc::new(AtomicBool::new(false));
        let fn_pressed_clone = fn_pressed.clone();
        let fn_flag_mask = Self::FN_FLAG_MASK;

        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::FlagsChanged],
            move |_proxy, event_type, event| {
                // 系统可能因超时或安全模式禁用 tap，记录日志。
                // CGEventType 未实现 PartialEq，需通过 u32 比较。
                // kCGEventTapDisabledByTimeout = 0xFFFFFFFE
                // kCGEventTapDisabledByUserInput = 0xFFFFFFFF
                let event_type_raw = event_type as u32;
                if event_type_raw == 0xFFFFFFFE || event_type_raw == 0xFFFFFFFF {
                    tracing::warn!(
                        "CGEventTap disabled by system (type=0x{:X}), will auto-recover",
                        event_type_raw
                    );
                    return None;
                }

                let flags = event.get_flags().bits();
                let fn_now = (flags & fn_flag_mask) != 0;
                let fn_was = fn_pressed_clone.load(Ordering::Relaxed);

                if fn_now && !fn_was {
                    fn_pressed_clone.store(true, Ordering::Relaxed);
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::handle_shortcut_action(&handle, "dictate").await;
                    });
                } else if !fn_now && fn_was {
                    fn_pressed_clone.store(false, Ordering::Relaxed);
                }

                None
            },
        )
        .map_err(|_| {
            PlatformError::InjectionFailed(
                "Failed to create CGEventTap for Fn key. \
                 Ensure Input Monitoring permission is granted."
                    .into(),
            )
        })?;

        let run_loop_source = tap.mach_port.create_runloop_source(0).map_err(|_| {
            PlatformError::InjectionFailed(
                "Failed to create run loop source for Fn key monitor".into(),
            )
        })?;

        let run_loop = CFRunLoop::get_current();

        // 存储 RunLoop 引用供 Drop 使用
        {
            let mut holder = run_loop_holder.lock().unwrap();
            *holder = Some(run_loop.clone());
        }

        run_loop.add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });
        tap.enable();
        tracing::info!("FnKeyMonitor started on dedicated thread");

        // 通知调用者：初始化成功
        let _ = tx.send(Ok(()));

        CFRunLoop::run_current();
        tracing::info!("FnKeyMonitor stopped");

        Ok(())
    }
}

impl Drop for FnKeyMonitor {
    fn drop(&mut self) {
        tracing::info!("FnKeyMonitor dropping, stopping RunLoop");
        if let Ok(guard) = self.run_loop.lock()
            && let Some(ref run_loop) = *guard
        {
            run_loop.stop();
        }
    }
}

// ---------------------------------------------------------------------------
// 平台统一接口实现 — macOS
// ---------------------------------------------------------------------------

/// 注册 macOS 平台热键。
///
/// - Fn 键（听写）: 通过 FnKeyMonitor CGEventTap 实现
/// - ⌥T（翻译）、⌃Space（AI 助手）、Escape（取消）: 通过 tauri-plugin-global-shortcut
pub fn register_platform_hotkeys(app: &tauri::App) -> Result<Option<FnKeyMonitor>, PlatformError> {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    // 1. 启动 Fn 键监听器
    let fn_monitor = FnKeyMonitor::start(app.handle().clone())?;

    // 2. 注册其余快捷键（使用 global-shortcut 插件）
    let shortcuts = [
        (Shortcut::new(Some(Modifiers::ALT), Code::KeyT), "translate"),
        (
            Shortcut::new(Some(Modifiers::CONTROL), Code::Space),
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
                    super::super::handle_shortcut_action(&h2, &mode).await;
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

    Ok(Some(fn_monitor))
}

/// macOS 快捷键显示标签。
pub fn shortcut_labels() -> super::ShortcutLabels {
    super::ShortcutLabels {
        dictate: "Fn",
        translate: "⌥T",
        ai_assistant: "⌃Space",
        cancel: "Esc",
    }
}

/// 检查 macOS 平台权限状态 — 原生 API 精确检测。
///
/// - Accessibility: `AXIsProcessTrusted()` (<0.1ms)
/// - Input Monitoring: `CGPreflightListenEventAccess()` (<0.1ms)
pub fn check_permissions() -> super::PermissionStatus {
    let accessibility = unsafe { ax::AXIsProcessTrusted() };
    let input_monitoring = unsafe { ax::CGPreflightListenEventAccess() };

    match (accessibility, input_monitoring) {
        (true, true) => super::PermissionStatus::Granted,
        (false, true) => super::PermissionStatus::AccessibilityRequired,
        (true, false) => super::PermissionStatus::InputMonitoringRequired,
        (false, false) => super::PermissionStatus::BothRequired,
    }
}

/// 打开 macOS 系统偏好设置对应的权限面板。
///
/// - `target == Some("input_monitoring")` → 输入监控面板
/// - 其他 → 辅助功能面板（默认）
pub fn open_permission_settings_for(target: Option<&str>) {
    let pane = match target {
        Some("input_monitoring") => "Privacy_ListenEvent",
        _ => "Privacy_Accessibility",
    };
    let url = format!("x-apple.systempreferences:com.apple.preference.security?{pane}");
    let _ = Command::new("open").arg(&url).spawn();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_input_threshold_consistent_with_linux() {
        // 与 Linux inject_text 中的 200 字节阈值保持一致
        assert_eq!(DIRECT_INPUT_THRESHOLD, 200);
    }

    #[test]
    fn test_max_unicode_per_event() {
        // CGEventKeyboardSetUnicodeString 的硬限制
        assert_eq!(MAX_UNICODE_PER_EVENT, 20);
    }

    #[test]
    fn test_delays_configured() {
        // 验证延迟值合理（非零且在预期范围内）
        assert_eq!(CMD_C_PROCESS_DELAY.as_millis(), 50);
    }

    #[test]
    fn test_utf16_chunking_ascii() {
        // ASCII: 每个字符 1 个 UTF-16 code unit
        let text = "Hello, World!";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let chunks: Vec<&[u16]> = utf16.chunks(MAX_UNICODE_PER_EVENT).collect();
        assert_eq!(chunks.len(), 1); // 13 chars, fits in 1 chunk
    }

    #[test]
    fn test_utf16_chunking_cjk() {
        // CJK: 每个字符 1 个 UTF-16 code unit（BMP 内）
        let text = "你好世界这是一段较长的中文文本测试啊啊啊啊啊啊啊";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let chunks: Vec<&[u16]> = utf16.chunks(MAX_UNICODE_PER_EVENT).collect();
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= MAX_UNICODE_PER_EVENT);
        }
    }

    #[test]
    fn test_utf16_chunking_emoji() {
        // Emoji: 有些需要 2 个 UTF-16 code unit（surrogate pair）
        let text = "🎤🎵🎶🎸🎹🎺🎻🎼🎷🪗🪕";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let chunks: Vec<&[u16]> = utf16.chunks(MAX_UNICODE_PER_EVENT).collect();
        assert!(chunks.len() >= 2); // 11 emoji × 2 code units = 22, needs 2 chunks
        for chunk in &chunks {
            assert!(chunk.len() <= MAX_UNICODE_PER_EVENT);
        }
    }

    #[test]
    fn test_utf16_chunking_mixed() {
        // 混合文本: ASCII + CJK + emoji
        let text = "Hello 你好 🎤";
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let chunks: Vec<&[u16]> = utf16.chunks(MAX_UNICODE_PER_EVENT).collect();
        // "Hello 你好 🎤" = 6 + 2 + 1 + 2 = 11 code units
        assert_eq!(chunks.len(), 1);
    }

    // macOS 专用集成测试
    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;

        #[test]
        fn test_clipboard_roundtrip() {
            clipboard_write("tingyuxuan_test_clipboard").unwrap();
            let read = clipboard_read().unwrap();
            assert_eq!(read, Some("tingyuxuan_test_clipboard".to_string()));
        }

        #[test]
        fn test_clipboard_roundtrip_cjk() {
            clipboard_write("你好世界🎤").unwrap();
            let read = clipboard_read().unwrap();
            assert_eq!(read, Some("你好世界🎤".to_string()));
        }

        #[test]
        fn test_type_text_directly_no_panic() {
            // CGEvent 直接输入 — 需要 Accessibility 权限
            let _ = type_text_directly("Hello");
        }

        #[test]
        fn test_type_text_directly_cjk_no_panic() {
            let _ = type_text_directly("你好世界");
        }

        #[test]
        fn test_type_text_directly_emoji_no_panic() {
            let _ = type_text_directly("🎤🎵🎶");
        }

        #[test]
        fn test_get_app_name_no_panic() {
            let detector = MacOSContextDetector::new();
            let _ = detector.get_app_name();
        }

        #[test]
        fn test_get_window_title_no_panic() {
            let detector = MacOSContextDetector::new();
            let _ = detector.get_window_title();
        }

        #[test]
        fn test_get_selected_text_via_ax_no_panic() {
            let detector = MacOSContextDetector::new();
            let _ = detector.get_selected_text_via_ax();
        }

        #[test]
        fn test_collect_context_no_panic() {
            let detector = MacOSContextDetector::new();
            let ctx = detector.collect_context();
            let _ = ctx;
        }

        #[test]
        fn test_check_permissions_no_panic() {
            use crate::platform::PermissionStatus;
            let status = check_permissions();
            // 返回四值之一即可
            assert!(matches!(
                status,
                PermissionStatus::Granted
                    | PermissionStatus::AccessibilityRequired
                    | PermissionStatus::InputMonitoringRequired
                    | PermissionStatus::BothRequired
            ));
        }

        #[test]
        fn test_simulate_cmd_v_no_panic() {
            let _ = simulate_cmd_v();
        }
    }
}
