use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Persisted configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub api_url: String,
    pub api_key: String,
    pub poll_interval_secs: u64,
    pub toggle_hotkey: String,
    pub quick_input_hotkey: String,
    pub autostart: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    // Window geometry (backend-only, restored on startup)
    #[serde(default)]
    pub window_pos: Option<(i32, i32)>,
    #[serde(default)]
    pub window_size: Option<(u32, u32)>,
}

fn default_language() -> String {
    "en".into()
}

fn default_theme() -> String {
    "dark".into()
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8642".into(),
            api_key: "".into(),
            poll_interval_secs: 3,
            toggle_hotkey: "Alt+Space".into(),
            quick_input_hotkey: "Ctrl+Alt+Shift+C".into(),
            autostart: false,
            language: "en".into(),
            theme: "dark".into(),
            window_pos: None,
            window_size: None,
        }
    }
}

impl ConfigFile {
    fn path() -> PathBuf {
        // Save to ~/HermesTray/config.json (user home, not buried in AppData)
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("HermesTray");
        dir.join("config.json")
    }

    /// Local chat history file (~/HermesTray/history.json)
    pub fn history_path() -> PathBuf {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("HermesTray");
        dir.join("history.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(data) = fs::read_to_string(&p) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&p, &data).map_err(|e| e.to_string())
    }
}

/// Runtime settings (wrapped in Mutex for Tauri managed state)
pub struct Config {
    pub file: Mutex<ConfigFile>,
}

impl Config {
    pub fn load() -> Self {
        Self {
            file: Mutex::new(ConfigFile::load()),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let cfg = self.file.lock().map_err(|e| e.to_string())?;
        cfg.save()
    }
}