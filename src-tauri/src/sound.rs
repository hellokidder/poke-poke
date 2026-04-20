use crate::settings::SettingsStore;
use std::sync::{Arc, Mutex};

/// Play the alert sound based on current settings.
pub fn play_alert_with_settings(settings_store: &Arc<Mutex<SettingsStore>>) {
    let sound = {
        let s = settings_store.lock().unwrap();
        s.settings.alert_sound.clone()
    };

    play_sound_str(&sound);
}

/// Play a named sound for preview purposes.
pub fn play_sound_by_name(name: &str) {
    let sound = format!("system:{}", name);
    play_sound_str(&sound);
}

fn resolve_sound_path(sound: &str) -> Option<String> {
    if sound == "mute" {
        return None;
    }

    let path = if let Some(name) = sound.strip_prefix("system:") {
        format!("/System/Library/Sounds/{}.aiff", name)
    } else {
        // Unknown format, fallback
        "/System/Library/Sounds/Glass.aiff".to_string()
    };

    Some(path)
}

fn play_sound_str(sound: &str) {
    let Some(path) = resolve_sound_path(sound) else {
        return;
    };

    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay").arg(&path).output();
    });
}

/// List available system sound names (without extension).
pub fn list_system_sounds() -> Vec<String> {
    let dir = std::path::Path::new("/System/Library/Sounds");
    let mut sounds: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".aiff").map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    sounds.sort();
    sounds
}

#[cfg(test)]
mod tests {
    use super::resolve_sound_path;

    #[test]
    fn resolve_sound_path_returns_system_sound_path() {
        assert_eq!(
            resolve_sound_path("system:Glass").as_deref(),
            Some("/System/Library/Sounds/Glass.aiff"),
        );
    }

    #[test]
    fn resolve_sound_path_returns_none_for_mute() {
        assert_eq!(resolve_sound_path("mute"), None);
    }

    #[test]
    fn resolve_sound_path_falls_back_for_unknown_format() {
        assert_eq!(
            resolve_sound_path("custom:path").as_deref(),
            Some("/System/Library/Sounds/Glass.aiff"),
        );
    }
}
