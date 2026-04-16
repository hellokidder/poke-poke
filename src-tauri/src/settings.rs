use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Global shortcut to toggle the panel, e.g. "CmdOrCtrl+Shift+P"
    #[serde(default)]
    pub panel_shortcut: Option<String>,

    /// Alert sound: "system:Glass", "system:Ping", "mute", etc.
    #[serde(default = "default_sound")]
    pub alert_sound: String,

    /// UI locale: "zh" or "en"
    #[serde(default = "default_locale")]
    pub locale: String,

    /// Launch on macOS login
    #[serde(default)]
    pub auto_start: bool,

    /// Session retention in hours. 0 = keep forever.
    #[serde(default = "default_retention")]
    pub session_retention_hours: u32,
}

fn default_sound() -> String {
    "system:Glass".into()
}

fn default_locale() -> String {
    "zh".into()
}

fn default_retention() -> u32 {
    24
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            panel_shortcut: None,
            alert_sound: default_sound(),
            locale: default_locale(),
            auto_start: false,
            session_retention_hours: default_retention(),
        }
    }
}

pub struct SettingsStore {
    pub settings: Settings,
    file_path: PathBuf,
}

impl SettingsStore {
    pub fn load(file_path: PathBuf) -> Self {
        let settings = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Settings::default()
        };
        Self {
            settings,
            file_path,
        }
    }

    pub fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            &self.file_path,
            serde_json::to_string_pretty(&self.settings).unwrap_or_default(),
        );
    }

    pub fn update(&mut self, new_settings: Settings) {
        self.settings = new_settings;
        self.save();
    }
}
