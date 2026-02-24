use std::process::Command;

use crate::text_injector::{detect_display_server, DisplayServer};

/// Gets the name/title of the currently active window.
pub fn get_active_window_name() -> Option<String> {
    match detect_display_server() {
        DisplayServer::X11 => get_active_window_x11(),
        DisplayServer::Wayland => get_active_window_wayland(),
        DisplayServer::Unknown => None,
    }
}

/// Gets the currently selected text (if any).
pub fn get_selected_text() -> Option<String> {
    match detect_display_server() {
        DisplayServer::X11 => get_selected_text_x11(),
        DisplayServer::Wayland => get_selected_text_wayland(),
        DisplayServer::Unknown => None,
    }
}

fn get_active_window_x11() -> Option<String> {
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

fn get_active_window_wayland() -> Option<String> {
    // Wayland doesn't have a universal way to get active window
    // Some compositors support wlrctl or other tools
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

fn get_selected_text_x11() -> Option<String> {
    // Try reading X11 PRIMARY selection (automatically set on text selection)
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

fn get_selected_text_wayland() -> Option<String> {
    // Try wl-paste with primary selection
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
