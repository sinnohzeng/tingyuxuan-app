//! 系统托盘菜单 — 对标 Typeless 托盘设计。
//!
//! 菜单结构：反馈 → 主页 → 分隔 → 设置/麦克风 → 分隔 → 词典 → 分隔 → 版本/更新 → 分隔 → 退出
//! 麦克风子菜单每次右键打开时惰性重建（cpal 无设备热插拔通知）。

use tauri::{
    AppHandle, Emitter, Manager,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::state::{ConfigState, RecorderState, TrayState};

/// 编译时从 Cargo.toml 读取的仓库 URL 和版本号。
const PKG_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 构建初始菜单 + TrayIcon，存入 TrayState。
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(app)?;

    let tray = TrayIconBuilder::new()
        .id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/icon.png"))?)
        .menu(&menu)
        .tooltip("TingYuXuan - 听语轩")
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => show_main_window(app),
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    ..
                } => {
                    // 惰性重建菜单：每次右键时重新枚举设备。
                    let _ = rebuild_tray_menu(app);
                }
                _ => {}
            }
        })
        .build(app)?;

    // 存入 managed state 以便后续 rebuild。
    let tray_state = app.state::<TrayState>();
    *tray_state.0.blocking_lock() = Some(tray);

    Ok(())
}

/// 从 TrayState 取出 handle，重建菜单并 set_menu。
fn rebuild_tray_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(app)?;
    let tray_state = app.state::<TrayState>();
    if let Some(ref tray) = *tray_state.0.blocking_lock() {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

/// 构建完整 Menu 结构。
fn build_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let feedback = MenuItem::with_id(app, "feedback", "反馈意见", true, None::<&str>)?;
    let open_main = MenuItem::with_id(app, "open_main", "打开听语轩主页", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置...", true, None::<&str>)?;
    let mic_submenu = build_mic_submenu(app)?;
    let dictionary = MenuItem::with_id(app, "dictionary", "将词汇添加到词典", true, None::<&str>)?;

    let version_label = format!("版本 {PKG_VERSION}");
    let version = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let check_update = MenuItem::with_id(app, "check_update", "检查更新", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出听语轩", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &feedback,
        &open_main,
        &PredefinedMenuItem::separator(app)?,
        &settings,
        &mic_submenu,
        &PredefinedMenuItem::separator(app)?,
        &dictionary,
        &PredefinedMenuItem::separator(app)?,
        &version,
        &check_update,
        &PredefinedMenuItem::separator(app)?,
        &quit,
    ])?;
    Ok(menu)
}

/// 动态构建麦克风选择子菜单（CheckMenuItem + 勾选当前设备）。
fn build_mic_submenu(app: &AppHandle) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let submenu = Submenu::with_id(app, "mic_submenu", "选择麦克风", true)?;

    // 读取当前配置中的设备 ID。
    let selected_id = app
        .try_state::<ConfigState>()
        .and_then(|cs| cs.0.blocking_read().audio.input_device_id.clone());

    // "系统默认"选项 — device_id=None 时勾选。
    let default_checked = selected_id.is_none();
    let default_item = CheckMenuItem::with_id(
        app, "mic:default", "系统默认", true, default_checked, None::<&str>,
    )?;
    submenu.append(&default_item)?;

    // 枚举真实设备。
    if let Ok(devices) = tingyuxuan_core::audio::devices::enumerate_input_devices() {
        for dev in devices {
            let checked = selected_id.as_deref() == Some(&dev.id);
            let item_id = format!("mic:{}", dev.id);
            let item = CheckMenuItem::with_id(
                app, &item_id, &dev.name, true, checked, None::<&str>,
            )?;
            submenu.append(&item)?;
        }
    }

    Ok(submenu)
}

/// 菜单事件路由。
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "feedback" => open_url(app, &format!("{PKG_REPOSITORY}/issues")),
        "open_main" => show_main_window(app),
        "settings" => {
            show_main_window(app);
            let _ = app.emit("open-settings", ());
        }
        "dictionary" => {
            show_main_window(app);
            let _ = app.emit("open-dictionary", ());
        }
        "check_update" => open_url(app, &format!("{PKG_REPOSITORY}/releases/latest")),
        "quit" => app.exit(0),
        _ if id.starts_with("mic:") => {
            let handle = app.clone();
            let menu_id = id.to_string();
            tauri::async_runtime::spawn(async move {
                handle_mic_selection(&handle, &menu_id).await;
            });
        }
        _ => {}
    }
}

/// 处理麦克风选择：更新 config + recorder + 重建菜单。
async fn handle_mic_selection(app: &AppHandle, menu_id: &str) {
    let device_id = match menu_id.strip_prefix("mic:") {
        Some("default") => None,
        Some(id) => Some(id.to_string()),
        None => return,
    };

    // 更新配置并通知 recorder actor。
    if let Some(config_state) = app.try_state::<ConfigState>() {
        let mut config = config_state.0.write().await;
        config.audio.input_device_id = device_id.clone();
        if let Err(e) = config.save() {
            tracing::error!("保存麦克风设置失败: {e}");
            return;
        }
    }

    if let Some(recorder_state) = app.try_state::<RecorderState>() {
        recorder_state.0.set_device(device_id).await;
    }

    // 重建菜单以更新勾选状态。
    let _ = rebuild_tray_menu(app);
    tracing::info!(menu_id, "麦克风设备已切换");
}

/// 使用 tauri-plugin-opener 打开 URL。
fn open_url(app: &AppHandle, url: &str) {
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        tracing::error!(url, "打开 URL 失败: {e}");
    }
}

/// 显示并聚焦主窗口。
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
