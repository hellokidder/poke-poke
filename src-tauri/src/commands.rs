use crate::notifications::{Task, TaskStore};
use crate::popup::{self, PopupList};
use crate::tray;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_notifications(
    store: State<'_, Arc<Mutex<TaskStore>>>,
) -> Vec<Task> {
    store.lock().unwrap().get_all().to_vec()
}

#[tauri::command]
pub fn get_unread_count(store: State<'_, Arc<Mutex<TaskStore>>>) -> usize {
    store.lock().unwrap().unread_count()
}

#[tauri::command]
pub fn get_notification_by_id(
    id: String,
    store: State<'_, Arc<Mutex<TaskStore>>>,
) -> Option<Task> {
    let s = store.lock().unwrap();
    s.get_all().iter().find(|t| t.id == id).cloned()
}

#[tauri::command]
pub fn mark_notification_read(
    id: String,
    store: State<'_, Arc<Mutex<TaskStore>>>,
    popup_list: State<'_, PopupList>,
    app: AppHandle,
) {
    let unread_count = {
        let mut s = store.lock().unwrap();
        s.mark_read(&id);
        s.unread_count()
    };
    popup::close_popup(&app, &id, &popup_list);
    tray::update_tray_icon(&app, unread_count);
}

#[tauri::command]
pub fn mark_all_read(
    store: State<'_, Arc<Mutex<TaskStore>>>,
    app: AppHandle,
) {
    {
        let mut s = store.lock().unwrap();
        s.mark_all_read();
    }
    tray::update_tray_icon(&app, 0);
}

#[tauri::command]
pub fn close_popup_window(
    id: String,
    store: State<'_, Arc<Mutex<TaskStore>>>,
    popup_list: State<'_, PopupList>,
    app: AppHandle,
) {
    let unread_count = {
        let mut s = store.lock().unwrap();
        s.mark_read(&id);
        s.unread_count()
    };
    popup::close_popup(&app, &id, &popup_list);
    tray::update_tray_icon(&app, unread_count);
}
