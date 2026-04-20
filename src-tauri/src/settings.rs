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
}

fn default_sound() -> String {
    "system:Glass".into()
}

fn default_locale() -> String {
    "zh".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            panel_shortcut: None,
            alert_sound: default_sound(),
            locale: default_locale(),
            auto_start: false,
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

#[cfg(test)]
mod tests {
    use super::{Settings, SettingsStore};
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    fn temp_store() -> (TempDir, SettingsStore, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        (dir, store, path)
    }

    #[test]
    fn load_returns_defaults_when_file_is_missing() {
        let (_dir, store, _path) = temp_store();

        assert_eq!(store.settings.panel_shortcut, None);
        assert_eq!(store.settings.alert_sound, "system:Glass");
        assert_eq!(store.settings.locale, "zh");
        assert!(!store.settings.auto_start);
    }

    #[test]
    fn update_and_reload_round_trip_preserves_settings() {
        let (_dir, mut store, path) = temp_store();
        let next = Settings {
            panel_shortcut: Some("CmdOrCtrl+Shift+P".into()),
            alert_sound: "mute".into(),
            locale: "en".into(),
            auto_start: true,
        };

        store.update(next);

        let reloaded = SettingsStore::load(path);
        assert_eq!(
            reloaded.settings.panel_shortcut.as_deref(),
            Some("CmdOrCtrl+Shift+P"),
        );
        assert_eq!(reloaded.settings.alert_sound, "mute");
        assert_eq!(reloaded.settings.locale, "en");
        assert!(reloaded.settings.auto_start);
    }

    #[test]
    fn load_fills_missing_fields_from_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"locale":"en"}"#).unwrap();

        let store = SettingsStore::load(path);
        assert_eq!(store.settings.locale, "en");
        assert_eq!(store.settings.alert_sound, "system:Glass");
        assert_eq!(store.settings.panel_shortcut, None);
        assert!(!store.settings.auto_start);
    }

    #[test]
    fn load_returns_defaults_when_file_is_damaged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not valid json").unwrap();

        let store = SettingsStore::load(path);
        assert_eq!(store.settings.panel_shortcut, None);
        assert_eq!(store.settings.alert_sound, "system:Glass");
        assert_eq!(store.settings.locale, "zh");
        assert!(!store.settings.auto_start);
    }
}
