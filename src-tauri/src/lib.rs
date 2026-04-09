mod config;
mod tray;
mod usage;

use config::AppConfig;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

pub(crate) struct AppState {
    pub(crate) config: Mutex<AppConfig>,
    pub(crate) usage: Mutex<usage::UsageData>,
}

#[tauri::command]
fn get_usage(state: tauri::State<AppState>) -> usage::UsageData {
    state.usage.lock().unwrap().clone()
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_api_key(key: String, state: tauri::State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    config.auth.api_key = key;
    config.save()
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
fn save_widget_position(x: f64, y: f64, state: tauri::State<AppState>) {
    let mut config = state.config.lock().unwrap();
    config.widget.position_x = x;
    config.widget.position_y = y;
    let _ = config.save();
}

#[tauri::command]
async fn refresh_usage(state: tauri::State<'_, AppState>) -> Result<usage::UsageData, String> {
    let api_key = {
        let config = state.config.lock().unwrap();
        config.api_key()
    };
    let Some(api_key) = api_key else {
        return Err("No API key configured".to_string());
    };
    let data = usage::fetch_usage(&api_key).await?;
    {
        let mut current = state.usage.lock().unwrap();
        *current = data.clone();
    }
    Ok(data)
}

fn start_polling(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval = {
                let state = app_handle.state::<AppState>();
                let config = state.config.lock().unwrap();
                config.polling.interval_seconds
            };

            let api_key = {
                let state = app_handle.state::<AppState>();
                let config = state.config.lock().unwrap();
                config.api_key()
            };

            if let Some(key) = api_key {
                match usage::fetch_usage(&key).await {
                    Ok(data) => {
                        let state = app_handle.state::<AppState>();
                        {
                            let mut current = state.usage.lock().unwrap();
                            *current = data.clone();
                        }
                        let _ = app_handle.emit("usage-updated", &data);
                        tray::update_tray_icon(&app_handle, data.max_utilization());
                        tray::update_tray_stats(&app_handle, &data);
                    }
                    Err(e) => {
                        eprintln!("Usage fetch error: {}", e);
                        let _ = app_handle.emit("usage-error", &e);
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    });
}

pub fn run() {
    let config = AppConfig::load();
    let state = AppState {
        config: Mutex::new(config),
        usage: Mutex::new(usage::UsageData::empty()),
    };

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            start_polling(app.handle().clone());
            tray::setup_tray(app.handle()).expect("Failed to setup tray");
            // Restore widget position from config
            let config = app.state::<AppState>().config.lock().unwrap().clone();
            if let Some(window) = app.get_webview_window("widget") {
                let _ = window.set_position(tauri::PhysicalPosition::new(
                    config.widget.position_x as i32,
                    config.widget.position_y as i32,
                ));
                if !config.widget.show_on_launch {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_usage,
            get_config,
            set_api_key,
            refresh_usage,
            save_widget_position,
            exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
