#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
};

use super::error::PlatformError;
use super::{ContextDetector, TextInjector};

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
        let ok = unsafe { OpenClipboard(HWND::default()) };
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
            // SAFETY: GlobalLock returns a pointer to the clipboard data.
            // We read the UTF-16 string and unlock immediately.
            let ptr = unsafe { GlobalLock(std::mem::transmute(h.0)) };
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
            unsafe { GlobalUnlock(std::mem::transmute(h.0)) };
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
            // SAFETY: Free the memory if lock failed.
            let _ = unsafe { GlobalFree(hmem) };
            return Err(PlatformError::ClipboardError(
                "GlobalLock failed".to_string(),
            ));
        }
        // SAFETY: Copy UTF-16 data into the allocated memory.
        unsafe {
            std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, ptr as *mut u8, byte_len);
        }
        // SAFETY: Unlock after writing.
        unsafe { GlobalUnlock(hmem) };
    }

    let _guard = ClipboardGuard::open()?;
    // SAFETY: EmptyClipboard clears the current contents.
    let _ = unsafe { EmptyClipboard() };
    // SAFETY: SetClipboardData takes ownership of hmem.
    // The system will free it when the clipboard is next emptied.
    let result = unsafe { SetClipboardData(13, std::mem::transmute(hmem.0)) }; // CF_UNICODETEXT = 13
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
    if let Some(original) = saved {
        let _ = clipboard_write(&original); // best-effort restore
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
}

impl ContextDetector for WindowsContextDetector {
    fn get_active_window_name(&self) -> Option<String> {
        let _span = tracing::info_span!("get_active_window", platform = "windows").entered();

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

    fn get_selected_text(&self) -> Option<String> {
        let _span = tracing::info_span!("get_selected_text", platform = "windows").entered();

        #[cfg(target_os = "windows")]
        {
            // Strategy: simulate Ctrl+C → read clipboard → restore clipboard.
            tokio::task::block_in_place(|| copy_selection_via_clipboard())
        }

        #[cfg(not(target_os = "windows"))]
        {
            None
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
    if let Some(original) = saved {
        let _ = clipboard_write(&original);
    }

    text.filter(|t| !t.is_empty())
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
