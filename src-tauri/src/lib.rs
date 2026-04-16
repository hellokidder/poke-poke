mod commands;
mod http_server;
mod popup;
mod sessions;
mod settings;
mod shortcut;
mod sound;
mod tray;

use sessions::SessionStore;
use settings::SettingsStore;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let data_dir = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));

            // Migrate legacy data file name
            let legacy_path = data_dir.join("notifications.json");
            let store_path = data_dir.join("sessions.json");
            if !store_path.exists() && legacy_path.exists() {
                let _ = std::fs::rename(&legacy_path, &store_path);
            }

            let store = Arc::new(Mutex::new(SessionStore::load(store_path)));
            let popup_list = popup::create_popup_list();

            let settings_path = data_dir.join("settings.json");
            let settings_store = Arc::new(Mutex::new(SettingsStore::load(settings_path)));

            app.handle().manage(store.clone());
            app.handle().manage(popup_list.clone());
            app.handle().manage(settings_store.clone());

            tray::create_tray(app.handle())?;

            // Register saved global shortcut
            shortcut::apply_shortcut(app.handle());

            let app_handle = app.handle().clone();
            let store_clone = store.clone();
            let popup_clone = popup_list.clone();
            let settings_clone = settings_store.clone();
            tauri::async_runtime::spawn(async move {
                http_server::start(app_handle, store_clone, popup_clone, settings_clone).await;
            });

            // Background cleanup timer: every 5 minutes
            {
                let cleanup_store = store.clone();
                let cleanup_settings = settings_store.clone();
                let cleanup_app = app.handle().clone();
                std::thread::spawn(move || {
                    use tauri::Emitter;
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(300));
                        // TTL-based cleanup
                        let retention = cleanup_settings
                            .lock()
                            .unwrap()
                            .settings
                            .session_retention_hours;
                        if retention > 0 {
                            let cleaned =
                                cleanup_store.lock().unwrap().cleanup_expired(retention);
                            if cleaned > 0 {
                                let _ = cleanup_app.emit("sessions-updated", ());
                            }
                        }
                        // Stale session reaping
                        let reaped = cleanup_store.lock().unwrap().reap_stale_sessions();
                        if reaped > 0 {
                            let _ = cleanup_app.emit("sessions-updated", ());
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_session_by_id,
            commands::remove_session,
            commands::close_popup_window,
            commands::open_session_source,
            commands::focus_session_terminal,
            commands::check_cc_integration,
            commands::check_codex_integration,
            commands::check_cursor_integration,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings_window,
            commands::close_settings_window,
            commands::list_system_sounds,
            commands::preview_sound,
        ])
        .build(tauri::generate_context!())
        .expect("error while building PokePoke")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}

fn dirs_next() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::PathBuf::from(home).join(".pokepoke");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
