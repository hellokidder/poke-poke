mod commands;
mod http_server;
mod notifications;
mod popup;
mod sound;
mod tray;

use notifications::TaskStore;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
            let store_path = data_dir.join("notifications.json");
            let store = Arc::new(Mutex::new(TaskStore::load(store_path)));
            let popup_list = popup::create_popup_list();

            app.handle().manage(store.clone());
            app.handle().manage(popup_list.clone());

            tray::create_tray(app.handle())?;

            let app_handle = app.handle().clone();
            let store_clone = store.clone();
            let popup_clone = popup_list.clone();
            tauri::async_runtime::spawn(async move {
                http_server::start(app_handle, store_clone, popup_clone).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_notifications,
            commands::get_unread_count,
            commands::get_notification_by_id,
            commands::mark_notification_read,
            commands::mark_all_read,
            commands::close_popup_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PokePoke");
}

fn dirs_next() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::PathBuf::from(home).join(".pokepoke");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
