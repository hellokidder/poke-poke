use crate::notifications::{Task, TaskStore};
use crate::popup::{self, PopupList};
use crate::tray;
use std::process::Command;
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

/// Focus the terminal window that spawned the task.
/// Tries Terminal.app, then iTerm2 (matching by tty).
/// Falls back to opening workspace_path in Finder if neither finds the tty.
#[tauri::command]
pub fn open_task_source(
    id: String,
    store: State<'_, Arc<Mutex<TaskStore>>>,
) {
    let (terminal_tty, workspace_path) = {
        let s = store.lock().unwrap();
        let task = s.get_all().iter().find(|t| t.id == id).cloned();
        match task {
            Some(t) => (t.terminal_tty, t.workspace_path),
            None => return,
        }
    };

    if let Some(ref tty) = terminal_tty {
        if !tty.is_empty() && try_focus_terminal(tty) {
            return;
        }
    }

    // Fallback: open workspace path in Finder
    if let Some(path) = workspace_path {
        if !path.is_empty() {
            let _ = Command::new("open").arg(&path).output();
        }
    }
}

/// Try Terminal.app then iTerm2. Returns true if a window was found and focused.
fn try_focus_terminal(tty: &str) -> bool {
    // AppleScript returns "true" if found, "false" otherwise (exit 0 either way)
    let terminal_script = format!(
        r#"tell application "Terminal"
    set found to false
    repeat with w in windows
        repeat with t in tabs of w
            if tty of t is "{tty}" then
                activate
                set selected tab of w to t
                set index of w to 1
                set found to true
                exit repeat
            end if
        end repeat
        if found then exit repeat
    end repeat
    return found
end tell"#,
        tty = tty
    );

    if run_applescript_found(&terminal_script) {
        return true;
    }

    let iterm_script = format!(
        r#"tell application "iTerm2"
    set found to false
    repeat with w in windows
        if found then exit repeat
        repeat with t in tabs of w
            if found then exit repeat
            repeat with s in sessions of t
                if tty of s is "{tty}" then
                    activate
                    select s
                    set found to true
                    exit repeat
                end if
            end repeat
        end repeat
    end repeat
    return found
end tell"#,
        tty = tty
    );

    run_applescript_found(&iterm_script)
}

/// Run an AppleScript and return true only if stdout is "true".
fn run_applescript_found(script: &str) -> bool {
    Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
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
