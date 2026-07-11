mod api;
mod config;
mod tray;

use api::{ConnectionStatus, HermesApi};
use config::Config;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Start background polling of Hermes API status
fn start_status_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let api = HermesApi::new();
        let mut last_status: Option<ConnectionStatus> = None;

        loop {
            let (base_url, api_key, interval) = {
                let cfg = app.state::<Config>();
                let file = cfg.file.lock().unwrap();
                (
                    file.api_url.clone(),
                    file.api_key.clone(),
                    file.poll_interval_secs,
                )
            };

            let status = api.poll_status(&base_url, &api_key).await;

            if Some(&status) != last_status.as_ref() {
                last_status = Some(status.clone());
                tray::update_tray(&app, &status);

                let _ = app.emit(
                    "hermes-status",
                    serde_json::json!({
                        "status": match &status {
                            ConnectionStatus::Disconnected => "disconnected",
                            ConnectionStatus::Idle => "idle",
                            ConnectionStatus::Busy => "busy",
                        }
                    }),
                );
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    let cfg = app.state::<Config>();
                    let toggle_key = cfg.file.lock().unwrap().toggle_hotkey.clone();

                    // Parse the toggle shortcut from config
                    let toggle_parts: Vec<&str> = toggle_key.split('+').collect();
                    let mut toggle_mods = Modifiers::empty();
                    let mut toggle_code = None;
                    for part in &toggle_parts {
                        match *part {
                            "Alt" => toggle_mods |= Modifiers::ALT,
                            "Ctrl" | "Control" => toggle_mods |= Modifiers::CONTROL,
                            "Shift" => toggle_mods |= Modifiers::SHIFT,
                            "Super" | "Win" | "Cmd" => toggle_mods |= Modifiers::SUPER,
                            "Space" => toggle_code = Some(Code::Space),
                            "H" => toggle_code = Some(Code::KeyH),
                            "S" => toggle_code = Some(Code::KeyS),
                            "C" => toggle_code = Some(Code::KeyC),
                            _ => {}
                        }
                    }

                    if let Some(code) = toggle_code {
                        let configured = Shortcut::new(Some(toggle_mods), code);
                        if shortcut == &configured {
                            if let Some(win) = app.get_webview_window("main") {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                            return;
                        }
                    }

                    // Quick-input shortcut: Ctrl+Alt+Shift+C
                    if shortcut.matches(
                        Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT,
                        Code::KeyC,
                    ) {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("quick-input", ());
                        }
                    }
                })
                .build(),
        )
        .manage(Config::load())
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            get_status,
            send_chat,
            create_session,
            list_sessions,
            test_connection,
        ])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Open Window", true, Some("Alt+Space"))?;
            let settings_item =
        MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;

            let menu = Menu::with_items(app, &[&show, &settings_item, &sep, &quit])?;

            let icon = tray::make_tray_icon(160, 160, 160);
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .menu(&menu)
                .tooltip("HermesTray — connecting...")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "settings" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("navigate", "settings");
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Start background status polling
            start_status_poller(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close: hide instead of quitting.
            // Only tray menu "Quit" truly exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Tauri Commands ───────────────────────────────────────────

#[tauri::command]
fn get_config(config: tauri::State<Config>) -> Result<config::ConfigFile, String> {
    config.file.lock().map(|f| f.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_config(
    app: AppHandle,
    config: tauri::State<Config>,
    new_config: config::ConfigFile,
) -> Result<(), String> {
    // Unregister old shortcuts
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    {
        let mut file = config.file.lock().map_err(|e| e.to_string())?;
        *file = new_config;
    }
    config.save()?;

    // Re-register shortcuts with new config
    let gs = app.global_shortcut();
    let toggle_key = config.file.lock().unwrap().toggle_hotkey.clone();
    if !toggle_key.is_empty() {
        if let Ok(shortcut) = Shortcut::try_from(toggle_key.as_str()) {
            let _ = gs.register(shortcut);
        }
    }

    Ok(())
}

#[tauri::command]
async fn get_status(
    config: tauri::State<'_, Config>,
) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    let status = api.poll_status(&url, &key).await;
    Ok(serde_json::to_string(&status).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn send_chat(
    config: tauri::State<'_, Config>,
    session_id: String,
    message: String,
) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    api.send_message(&url, &key, &session_id, &message).await
}

#[tauri::command]
async fn create_session(config: tauri::State<'_, Config>) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    api.create_session(&url, &key).await
}

#[tauri::command]
async fn list_sessions(config: tauri::State<'_, Config>) -> Result<Vec<serde_json::Value>, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    api.list_sessions(&url, &key).await
}

#[tauri::command]
async fn test_connection(config: tauri::State<'_, Config>) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    match api.poll_status(&url, &key).await {
        ConnectionStatus::Disconnected => Err("Cannot reach Hermes API".into()),
        other => Ok(serde_json::to_string(&other).map_err(|e| e.to_string())?),
    }
}