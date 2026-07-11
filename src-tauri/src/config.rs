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
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en".into()
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8642".into(),
            api_key: "".into(),
            poll_interval_secs: 3,
            toggle_hotkey: "Alt+Space".into(),
            quick_input_hotkey: "Ctrl+Alt+Shift+C".into(),
            language: "en".into(),
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

    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(data) = fs::read_to_string(&p) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            let cfg = Self::default();
            let _ = cfg.save();
            cfg
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