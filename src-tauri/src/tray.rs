use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dictate_item = MenuItem::with_id(
        app,
        "dictate",
        "开始听写   Ctrl+Shift+D",
        true,
        None::<&str>,
    )?;
    let translate_item = MenuItem::with_id(
        app,
        "translate",
        "开始翻译   Ctrl+Shift+T",
        true,
        None::<&str>,
    )?;
    let ai_item = MenuItem::with_id(
        app,
        "ai_assistant",
        "AI 助手    Ctrl+Shift+A",
        true,
        None::<&str>,
    )?;
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
                // Reuse the same shortcut-action event mechanism.
                let _ = app.emit("shortcut-action", event.id.as_ref());
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "history" => {
                // Open settings window — the frontend will switch to history tab
                // via the "open-history" event.
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("open-history", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
