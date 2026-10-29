mod api;
mod config;
mod tray;

use api::{ConnectionStatus, HermesApi};
use config::Config;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Tracks in-flight chat streams: used to (1) report `busy` to the tray/UI
/// while a chat is streaming and (2) cancel a stream from the frontend.
pub struct ChatAbort {
    pub active: Mutex<HashSet<String>>,
    pub cancels: Mutex<HashMap<String, tokio::sync::broadcast::Sender<()>>>,
}

/// App start time for the status stats (uptime).
static APP_START: OnceLock<std::time::Instant> = OnceLock::new();

/// Cached session count, refreshed by the `list_sessions` command. `get_stats`
/// reads this instead of issuing a full HTTP list request every few seconds.
static SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);

impl Default for ChatAbort {
    fn default() -> Self {
        Self {
            active: Mutex::new(HashSet::new()),
            cancels: Mutex::new(HashMap::new()),
        }
    }
}

/// Start background polling of Hermes API status
fn start_status_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let api = HermesApi::new();
        let mut last_status: Option<ConnectionStatus> = None;
        let mut backoff: u64 = 1;

        loop {
            let (base_url, api_key, interval) = {
                let cfg = app.state::<Config>();
                let file = cfg.file.lock().unwrap_or_else(|e| e.into_inner());
                (
                    file.api_url.clone(),
                    file.api_key.clone(),
                    file.poll_interval_secs.max(1),
                )
            };

            let mut status = api.poll_status(&base_url, &api_key).await;

            // While a chat stream is in flight, always report busy
            let active_chats = {
                let s = app.state::<ChatAbort>();
                let guard = s.active.lock().unwrap();
                guard.len()
            };
            if active_chats > 0 {
                status = ConnectionStatus::Busy;
            }

            // Always update tray (cheap if unchanged)
            tray::update_tray(&app, &status);

            // Emit to frontend on every poll so UI stays in sync
            let status_str = match &status {
                ConnectionStatus::Disconnected => "disconnected",
                ConnectionStatus::Idle => "idle",
                ConnectionStatus::Busy => "busy",
            };
            let _ = app.emit(
                "hermes-status",
                serde_json::json!({ "status": status_str }),
            );

            // Only log on change
            if Some(&status) != last_status.as_ref() {
                last_status = Some(status.clone());
            }

            // Exponential backoff while disconnected (up to 5× the interval)
            let wait = if status == ConnectionStatus::Disconnected {
                backoff = (backoff * 2).min(5);
                interval * backoff
            } else {
                backoff = 1;
                interval
            };

            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    let (toggle_key, quick_key) = {
                        let cfg = app.state::<Config>();
                        let file = cfg.file.lock().unwrap_or_else(|e| e.into_inner());
                        (file.toggle_hotkey.clone(), file.quick_input_hotkey.clone())
                    };

                    // Toggle window
                    if let Some((mods, code)) = parse_shortcut(&toggle_key) {
                        let configured = Shortcut::new(Some(mods), code);
                        if shortcut == &configured {
                            toggle_window(app);
                            return;
                        }
                    }

                    // Quick input (configurable, default Ctrl+Alt+Shift+C)
                    if let Some((mods, code)) = parse_shortcut(&quick_key) {
                        let configured = Shortcut::new(Some(mods), code);
                        if shortcut == &configured {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                                let _ = win.emit("quick-input", ());
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(Config::load())
        .manage(ChatAbort::default())
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            get_status,
            send_chat,
            send_chat_stream,
            cancel_chat,
            create_session,
            list_sessions,
            get_session_messages,
            delete_session,
            test_connection,
            hide_window,
            save_history,
            load_history,
            check_update,
            get_stats,
        ])
        .setup(|app| {
            let _ = APP_START.set(std::time::Instant::now());

            let show = MenuItem::with_id(app, "show", "Open Window", true, Some("Alt+Space"))?;
            let new_chat = MenuItem::with_id(app, "new_chat", "New Chat", true, None::<&str>)?;
            let settings_item =
        MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let check_update = MenuItem::with_id(app, "check_update", "Check for Updates...", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;

            let menu = Menu::with_items(app, &[&show, &new_chat, &settings_item, &sep, &check_update, &sep, &quit])?;

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
                    "new_chat" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("new-session", ());
                        }
                    }
                    "settings" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("navigate", "settings");
                        }
                    }
                    "check_update" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("check-update", ());
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    let should_toggle = match event {
                        tauri::tray::TrayIconEvent::DoubleClick { .. } => true,
                        tauri::tray::TrayIconEvent::Click { button, button_state, .. } => {
                            button == tauri::tray::MouseButton::Left
                                && button_state == tauri::tray::MouseButtonState::Up
                        }
                        _ => false,
                    };
                    if should_toggle {
                        toggle_window(app);
                    }
                })
                .build(app)?;

            // Restore window geometry (position/size) from saved config
            {
                let cfg = app.state::<Config>();
                let (pos, size) = {
                    let file = cfg.file.lock().unwrap();
                    (file.window_pos, file.window_size)
                };
                if let Some(win) = app.get_webview_window("main") {
                    if let Some((w, h)) = size {
                        let _ = win.set_size(tauri::PhysicalSize::new(w, h));
                    }
                    if let Some((x, y)) = pos {
                        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                    }
                }
            }

            // Apply auto-start from saved config
            {
                let cfg = app.state::<Config>();
                let file = cfg.file.lock().unwrap();
                if file.autostart {
                    let _ = app.autolaunch().enable();
                } else {
                    let _ = app.autolaunch().disable();
                }
            }

            // Start background status polling
            start_status_poller(app.handle().clone());

            // Register global shortcuts from config
            let gs = app.global_shortcut();
            let (toggle_key, quick_key) = {
                let cfg = app.state::<Config>();
                let file = cfg.file.lock().unwrap();
                (file.toggle_hotkey.clone(), file.quick_input_hotkey.clone())
            };
            if !toggle_key.is_empty() {
                if let Some(shortcut) = shortcut_from_str(&toggle_key) {
                    if let Err(e) = gs.register(shortcut) {
                        eprintln!("Failed to register toggle shortcut {}: {}", toggle_key, e);
                    }
                }
            }
            if !quick_key.is_empty() {
                if let Some(shortcut) = shortcut_from_str(&quick_key) {
                    if let Err(e) = gs.register(shortcut) {
                        eprintln!("Failed to register quick-input shortcut {}: {}", quick_key, e);
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close: hide instead of quitting.
            // Only tray menu "Quit" truly exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                save_window_geometry(window.app_handle());
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Parse a "Mod+Mod+Key" string (e.g. "Alt+Space", "Ctrl+Alt+Shift+C")
/// into `(Modifiers, Code)`. Returns None for unknown keys.
fn parse_shortcut(key: &str) -> Option<(Modifiers, Code)> {
    let mut mods = Modifiers::empty();
    let mut code = None;
    for part in key.split('+') {
        match part {
            "Alt" => mods |= Modifiers::ALT,
            "Ctrl" | "Control" => mods |= Modifiers::CONTROL,
            "Shift" => mods |= Modifiers::SHIFT,
            "Super" | "Win" | "Cmd" => mods |= Modifiers::SUPER,
            _ => {
                if let Some(c) = parse_key_code(part) {
                    code = Some(c);
                }
            }
        }
    }
    code.map(|c| (mods, c))
}

/// Build a `Shortcut` from a config string, trying Tauri's parser first and
/// falling back to our own key-name parser.
fn shortcut_from_str(key: &str) -> Option<Shortcut> {
    if let Ok(sc) = Shortcut::try_from(key) {
        return Some(sc);
    }
    parse_shortcut(key).map(|(mods, code)| Shortcut::new(Some(mods), code))
}

/// Show/hide the main window. Persists window geometry when hiding.
fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            save_window_geometry(app);
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// Persist the main window's position/size into the config file.
fn save_window_geometry(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
            let cfg = app.state::<Config>();
            if let Ok(mut file) = cfg.file.lock() {
                file.window_pos = Some((pos.x, pos.y));
                file.window_size = Some((size.width, size.height));
            }
            let _ = cfg.save();
        }
    }
}

/// Parse a single key name (e.g. "A", "1", "F5", "Space", "ArrowUp")
/// into a tauri `Code`. Returns None for unknown keys.
fn parse_key_code(key: &str) -> Option<Code> {
    use tauri_plugin_global_shortcut::Code::*;

    match key {
        "Space" => Some(Space),
        "Enter" => Some(Enter),
        "Tab" => Some(Tab),
        "Escape" => Some(Escape),
        "Backspace" => Some(Backspace),
        "Delete" => Some(Delete),
        "Home" => Some(Home),
        "End" => Some(End),
        "PageUp" => Some(PageUp),
        "PageDown" => Some(PageDown),
        "ArrowUp" => Some(ArrowUp),
        "ArrowDown" => Some(ArrowDown),
        "ArrowLeft" => Some(ArrowLeft),
        "ArrowRight" => Some(ArrowRight),
        _ => {
            // Single letters A-Z
            if key.len() == 1 {
                let c = key.chars().next()?;
                if c.is_ascii_alphabetic() {
                    let code = match c.to_ascii_uppercase() {
                        'A' => KeyA,
                        'B' => KeyB,
                        'C' => KeyC,
                        'D' => KeyD,
                        'E' => KeyE,
                        'F' => KeyF,
                        'G' => KeyG,
                        'H' => KeyH,
                        'I' => KeyI,
                        'J' => KeyJ,
                        'K' => KeyK,
                        'L' => KeyL,
                        'M' => KeyM,
                        'N' => KeyN,
                        'O' => KeyO,
                        'P' => KeyP,
                        'Q' => KeyQ,
                        'R' => KeyR,
                        'S' => KeyS,
                        'T' => KeyT,
                        'U' => KeyU,
                        'V' => KeyV,
                        'W' => KeyW,
                        'X' => KeyX,
                        'Y' => KeyY,
                        'Z' => KeyZ,
                        _ => unreachable!(),
                    };
                    return Some(code);
                }
                if c.is_ascii_digit() {
                    let code = match c {
                        '0' => Digit0,
                        '1' => Digit1,
                        '2' => Digit2,
                        '3' => Digit3,
                        '4' => Digit4,
                        '5' => Digit5,
                        '6' => Digit6,
                        '7' => Digit7,
                        '8' => Digit8,
                        '9' => Digit9,
                        _ => unreachable!(),
                    };
                    return Some(code);
                }
            }
            // Function keys F1-F12
            if let Some(num) = key.strip_prefix('F').and_then(|s| s.parse::<u8>().ok()) {
                let code = match num {
                    1 => F1,
                    2 => F2,
                    3 => F3,
                    4 => F4,
                    5 => F5,
                    6 => F6,
                    7 => F7,
                    8 => F8,
                    9 => F9,
                    10 => F10,
                    11 => F11,
                    12 => F12,
                    _ => return None,
                };
                return Some(code);
            }
            None
        }
    }
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

    let old_autostart = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        file.autostart
    };

    {
        let mut file = config.file.lock().map_err(|e| e.to_string())?;
        *file = new_config;
    }
    config.save()?;

    // Apply auto-start change
    let new_autostart = config.file.lock().unwrap().autostart;
    if new_autostart != old_autostart {
        if new_autostart {
            let _ = app.autolaunch().enable();
        } else {
            let _ = app.autolaunch().disable();
        }
    }

    // Re-register shortcuts with new config, reporting conflicts
    let (toggle_key, quick_key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.toggle_hotkey.clone(), file.quick_input_hotkey.clone())
    };

    let mut errors = Vec::new();
    if !toggle_key.is_empty() {
        if let Some(shortcut) = shortcut_from_str(&toggle_key) {
            if let Err(e) = gs.register(shortcut) {
                errors.push(format!("toggle hotkey \"{}\": {}", toggle_key, e));
            }
        } else {
            errors.push(format!("toggle hotkey \"{}\": unsupported key", toggle_key));
        }
    }
    if !quick_key.is_empty() {
        if let Some(shortcut) = shortcut_from_str(&quick_key) {
            if let Err(e) = gs.register(shortcut) {
                errors.push(format!("quick-input hotkey \"{}\": {}", quick_key, e));
            }
        } else {
            errors.push(format!("quick-input hotkey \"{}\": unsupported key", quick_key));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
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

/// Streaming chat: emits `chat-stream` events to the frontend, marks the app
/// busy for the duration, and can be aborted via `cancel_chat`.
#[tauri::command]
async fn send_chat_stream(
    app: AppHandle,
    chat: tauri::State<'_, ChatAbort>,
    config: tauri::State<'_, Config>,
    session_id: String,
    message: String,
) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };

    // Mark this session as active so the tray shows busy
    chat.active.lock().unwrap().insert(session_id.clone());
    let _ = app.emit("hermes-status", serde_json::json!({ "status": "busy" }));

    let (tx, rx) = tokio::sync::broadcast::channel(1);
    chat.cancels.lock().unwrap().insert(session_id.clone(), tx);

    let api = HermesApi::new();
    let result = api
        .send_message_stream(&app, &url, &key, &session_id, &message, rx)
        .await;

    // Cleanup
    chat.cancels.lock().unwrap().remove(&session_id);
    let remaining = {
        let mut s = chat.active.lock().unwrap();
        s.remove(&session_id);
        s.len()
    };

    // If no other chat is in flight, restore the real status immediately
    if remaining == 0 {
        let status = api.poll_status(&url, &key).await;
        tray::update_tray(&app, &status);
        let status_str = match &status {
            ConnectionStatus::Disconnected => "disconnected",
            ConnectionStatus::Idle => "idle",
            ConnectionStatus::Busy => "busy",
        };
        let _ = app.emit("hermes-status", serde_json::json!({ "status": status_str }));
    }

    result
}

#[tauri::command]
fn cancel_chat(
    chat: tauri::State<'_, ChatAbort>,
    session_id: String,
) -> Result<(), String> {
    if let Some(tx) = chat.cancels.lock().unwrap().get(&session_id) {
        let _ = tx.send(());
    }
    Ok(())
}

#[tauri::command]
async fn delete_session(
    config: tauri::State<'_, Config>,
    session_id: String,
) -> Result<(), String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    api.delete_session(&url, &key, &session_id).await
}

#[tauri::command]
async fn get_session_messages(
    config: tauri::State<'_, Config>,
    session_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    api.get_session_messages(&url, &key, &session_id).await
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
    let sessions = api.list_sessions(&url, &key).await?;
    SESSION_COUNT.store(sessions.len(), Ordering::Relaxed);
    Ok(sessions)
}

#[tauri::command]
async fn test_connection(config: tauri::State<'_, Config>) -> Result<String, String> {
    let (url, key) = {
        let file = config.file.lock().map_err(|e| e.to_string())?;
        (file.api_url.clone(), file.api_key.clone())
    };
    let api = HermesApi::new();
    match api.poll_status_isolated(&url, &key).await {
        ConnectionStatus::Disconnected => Err("Cannot reach Hermes API".into()),
        other => Ok(serde_json::to_string(&other).map_err(|e| e.to_string())?),
    }
}
// ── UI / Utility Commands ─────────────────────────────────

#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

/// Persist the local chat history (~/HermesTray/history.json).
/// Shape: { "active_session": string|null, "titles": {...}, "messages": {...} }
#[tauri::command]
fn save_history(history: serde_json::Value) -> Result<(), String> {
    let data = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;

    // Skip the disk write when the payload is unchanged since the last save.
    // The frontend debounces saves, but rename/auto-title/switch paths can still
    // produce identical consecutive payloads; avoid pointless I/O in those cases.
    {
        static LAST_HISTORY: Mutex<Option<String>> = Mutex::new(None);
        let mut last = LAST_HISTORY.lock().map_err(|e| e.to_string())?;
        if last.as_deref() == Some(data.as_str()) {
            return Ok(());
        }
        *last = Some(data.clone());
    }

    let p = config::ConfigFile::history_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&p, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_history() -> Result<serde_json::Value, String> {
    let p = config::ConfigFile::history_path();
    match std::fs::read_to_string(&p) {
        Ok(data) => serde_json::from_str(&data).map_err(|e| e.to_string()),
        Err(_) => Ok(serde_json::json!({
            "active_session": null,
            "titles": {},
            "messages": {},
        })),
    }
}

/// Check GitHub Releases for a newer version. Returns `Some({version, url})`
/// when an update is available, `None` when up-to-date.
#[tauri::command]
async fn check_update() -> Result<Option<serde_json::Value>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.github.com/repos/jackclub-cn/HermesTray/releases/latest")
        .header("User-Agent", "HermesTray")
        .send()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Update check failed: HTTP {}", resp.status().as_u16()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = body
        .get("tag_name")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");
    if !tag.is_empty() && is_newer(tag, current) {
        let url = body
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        Ok(Some(serde_json::json!({ "version": tag, "url": url })))
    } else {
        Ok(None)
    }
}

/// Minimal semver comparison (handles "0.4.0" and "v1.2.3" style).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split(['.', '-'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    let (a, b) = (parse(latest), parse(current));
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Runtime stats for the Settings "Live Status" section.
#[tauri::command]
fn get_stats() -> Result<serde_json::Value, String> {
    let uptime_secs = APP_START.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let session_count = SESSION_COUNT.load(Ordering::Relaxed);
    Ok(serde_json::json!({
        "uptime_secs": uptime_secs,
        "session_count": session_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    #[test]
    fn is_newer_basic_comparisons() {
        assert!(is_newer("0.5.0", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.0", "0.4.0"));
    }

    #[test]
    fn is_newer_strips_v_prefix() {
        assert!(is_newer("v1.2.3", "1.2.2"));
        assert!(!is_newer("v0.4.0", "0.4.0"));
    }

    #[test]
    fn is_newer_prerelease_ignored() {
        // "0.4.0-rc1" parses to [0,4,0] (the non-numeric suffix is dropped),
        // so it is not considered newer than a released 0.4.0.
        assert!(!is_newer("0.4.0-rc1", "0.4.0"));
    }

    #[test]
    fn is_newer_uneven_component_counts() {
        // [1,0,1] vs [1,0] → the extra patch component wins.
        assert!(is_newer("1.0.1", "1.0"));
        assert!(!is_newer("1.0", "1.0.1"));
    }

    #[test]
    fn parse_shortcut_single_modifier() {
        let (mods, code) = parse_shortcut("Alt+Space").unwrap();
        assert_eq!(mods, Modifiers::ALT);
        assert_eq!(code, Code::Space);
    }

    #[test]
    fn parse_shortcut_multiple_modifiers() {
        let (mods, code) = parse_shortcut("Ctrl+Alt+Shift+C").unwrap();
        assert_eq!(mods, Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT);
        assert_eq!(code, Code::KeyC);
    }

    #[test]
    fn parse_shortcut_function_key_without_modifier() {
        let (mods, code) = parse_shortcut("F5").unwrap();
        assert_eq!(mods, Modifiers::empty());
        assert_eq!(code, Code::F5);
    }

    #[test]
    fn parse_shortcut_unknown_key_returns_none() {
        assert!(parse_shortcut("Alt+UnknownKey").is_none());
        assert!(parse_shortcut("").is_none());
    }
}
