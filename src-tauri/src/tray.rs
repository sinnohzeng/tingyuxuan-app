use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::platform;

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let labels = platform::shortcut_labels();

    let dictate_label = format!("开始听写   {}", labels.dictate);
    let translate_label = format!("开始翻译   {}", labels.translate);
    let ai_label = format!("AI 助手    {}", labels.ai_assistant);

    let dictate_item = MenuItem::with_id(app, "dictate", &dictate_label, true, None::<&str>)?;
    let translate_item = MenuItem::with_id(app, "translate", &translate_label, true, None::<&str>)?;
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
        .icon(Image::from_bytes(include_bytes!("../icons/icon.png"))?)
        .menu(&menu)
        .tooltip("TingYuXuan - 听语轩")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "dictate" | "translate" | "ai_assistant" => {
                let handle = app.clone();
                let mode = event.id.as_ref().to_string();
                tauri::async_runtime::spawn(async move {
                    crate::handle_shortcut_action(&handle, &mode).await;
                });
            }
            "settings" => {
                show_main_window(app);
                let _ = tauri::Emitter::emit(app, "open-settings", ());
            }
            "history" => {
                show_main_window(app);
                let _ = tauri::Emitter::emit(app, "open-history", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        // 左键单击托盘图标显示主窗口（释放时触发，匹配 Windows 交互惯例）。
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
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
