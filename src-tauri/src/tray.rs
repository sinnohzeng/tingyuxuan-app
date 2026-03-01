use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};

use crate::platform;

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let labels = platform::shortcut_labels();

    let dictate_label = format!("开始听写   {}", labels.dictate);
    let translate_label = format!("开始翻译   {}", labels.translate);
    let ai_label = format!("AI 助手    {}", labels.ai_assistant);

    let dictate_item = MenuItem::with_id(app, "dictate", &dictate_label, true, None::<&str>)?;
    let translate_item =
        MenuItem::with_id(app, "translate", &translate_label, true, None::<&str>)?;
    let ai_item = MenuItem::with_id(app, "ai_assistant", &ai_label, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置...", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "历史记录...", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &dictate_item,
            &translate_item,
            &ai_item,
            &sep1,
            &settings_item,
            &history_item,
            &sep2,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("TingYuXuan - 听语轩")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "dictate" | "translate" | "ai_assistant" => {
                let _ = app.emit("shortcut-action", event.id.as_ref());
            }
            "settings" => {
                show_main_window(app);
                let _ = app.emit("open-settings", ());
            }
            "history" => {
                show_main_window(app);
                let _ = app.emit("open-history", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        // 双击托盘图标显示主窗口（Windows/macOS，Linux 不触发）。
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// 显示并聚焦主窗口。
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
