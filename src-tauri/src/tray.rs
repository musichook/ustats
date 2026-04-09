use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

/// Holds references to menu items we need to update dynamically.
pub struct TrayMenuItems {
    pub toggle_widget: MenuItem<tauri::Wry>,
    pub stat_session: MenuItem<tauri::Wry>,
    pub stat_weekly: MenuItem<tauri::Wry>,
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;

    let stat_session = MenuItem::with_id(app, "stat_session", "Session: --", false, None::<&str>)?;
    let stat_weekly = MenuItem::with_id(app, "stat_weekly", "Week (all): --", false, None::<&str>)?;
    let toggle_widget = MenuItem::with_id(app, "toggle_widget", "Hide Widget", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Store references for dynamic updates
    app.manage(Mutex::new(TrayMenuItems {
        toggle_widget: toggle_widget.clone(),
        stat_session: stat_session.clone(),
        stat_weekly: stat_weekly.clone(),
    }));

    let menu = Menu::with_items(app, &[
        &stat_session,
        &stat_weekly,
        &separator1,
        &toggle_widget,
        &refresh,
        &separator2,
        &quit,
    ])?;

    let icon = Image::from_bytes(include_bytes!("../icons/icon-green.png"))?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle_widget" => {
                if let Some(window) = app.get_webview_window("widget") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                        update_toggle_label(app, false);
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                        update_toggle_label(app, true);
                    }
                }
            }
            "refresh" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<crate::AppState>();
                    let api_key = {
                        let config = state.config.lock().unwrap();
                        config.api_key()
                    };
                    if let Some(key) = api_key {
                        if let Ok(data) = crate::usage::fetch_usage(&key).await {
                            {
                                let mut current = state.usage.lock().unwrap();
                                *current = data.clone();
                            }
                            let _ = app.emit("usage-updated", &data);
                            update_tray_icon(&app, data.max_utilization());
                            update_tray_stats(&app, &data);
                        }
                    }
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("widget") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                        update_toggle_label(app, false);
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                        update_toggle_label(app, true);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn update_toggle_label(app: &AppHandle, is_visible: bool) {
    let items = app.state::<Mutex<TrayMenuItems>>();
    let items = items.lock().unwrap();
    let label = if is_visible { "Hide Widget" } else { "Show Widget" };
    let _ = items.toggle_widget.set_text(label);
}

pub fn update_tray_icon(app: &AppHandle, utilization: f64) {
    let icon_bytes: &[u8] = if utilization > 90.0 {
        include_bytes!("../icons/icon-red.png")
    } else if utilization > 70.0 {
        include_bytes!("../icons/icon-yellow.png")
    } else {
        include_bytes!("../icons/icon-green.png")
    };

    if let Ok(icon) = Image::from_bytes(icon_bytes) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

pub fn update_tray_stats(app: &AppHandle, data: &crate::usage::UsageData) {
    let items = app.state::<Mutex<TrayMenuItems>>();
    let items = items.lock().unwrap();

    let session_text = match &data.session {
        Some(b) => format!("Session: {}%", b.utilization as u32),
        None => "Session: --".to_string(),
    };
    let _ = items.stat_session.set_text(&session_text);

    let weekly_text = match &data.weekly_all {
        Some(b) => format!("Week (all): {}%", b.utilization as u32),
        None => "Week (all): --".to_string(),
    };
    let _ = items.stat_weekly.set_text(&weekly_text);
}
