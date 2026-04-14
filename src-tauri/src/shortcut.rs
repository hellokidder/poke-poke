use crate::settings::SettingsStore;
use crate::tray;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// Read the current shortcut from settings and register it.
/// Unregisters any previously registered shortcuts first.
pub fn apply_shortcut(app: &AppHandle) {
    // Unregister all first
    let _ = app.global_shortcut().unregister_all();

    let shortcut_str = {
        let store = app.state::<Arc<Mutex<SettingsStore>>>();
        let s = store.lock().unwrap();
        s.settings.panel_shortcut.clone()
    };

    if let Some(ref key) = shortcut_str {
        if key.is_empty() {
            eprintln!("[PokePoke] Shortcut is empty, skipping");
            return;
        }
        eprintln!("[PokePoke] Registering shortcut: {}", key);
        match key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            Ok(shortcut) => {
                let app_clone = app.clone();
                let result = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
                    eprintln!("[PokePoke] Shortcut event: {:?}", event.state);
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        tray::toggle_settings_window(&app_clone);
                    }
                });
                match &result {
                    Ok(_) => eprintln!("[PokePoke] Shortcut registered OK"),
                    Err(e) => eprintln!("[PokePoke] Failed to register shortcut '{}': {}", key, e),
                }
            }
            Err(e) => {
                eprintln!("[PokePoke] Invalid shortcut '{}': {}", key, e);
            }
        }
    } else {
        eprintln!("[PokePoke] No shortcut configured");
    }
}
