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

/// Focus the terminal that matches the task's tty.
/// Does NOT close the popup — the frontend handles that separately
/// to avoid destroying the calling webview mid-command.
#[tauri::command]
pub fn focus_task_terminal(
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
        if !tty.is_empty() && focus_terminal(tty) {
            return;
        }
    }

    // Fallback: open workspace path in Finder
    if let Some(ref path) = workspace_path {
        if !path.is_empty() {
            let _ = Command::new("open").arg(path).output();
        }
    }
}

/// Focus the terminal window that spawned the task (used by panel clicks).
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
        if !tty.is_empty() && focus_terminal(tty) {
            return;
        }
    }

    if let Some(path) = workspace_path {
        if !path.is_empty() {
            let _ = Command::new("open").arg(&path).output();
        }
    }
}

/// Check if a macOS application is currently running (without launching it).
fn is_app_running(app_name: &str) -> bool {
    Command::new("osascript")
        .args(["-e", &format!("application \"{}\" is running", app_name)])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Focus the terminal matching the given tty.
/// Only tries terminal apps that are already running to avoid
/// launching unwanted apps (e.g. Terminal.app when user uses iTerm2).
fn focus_terminal(tty: &str) -> bool {
    // iTerm2 first (most common dev terminal on macOS)
    if is_app_running("iTerm2") && focus_iterm2(tty) {
        return true;
    }
    // Then Terminal.app
    if is_app_running("Terminal") && focus_terminal_app(tty) {
        return true;
    }
    false
}

fn focus_iterm2(tty: &str) -> bool {
    // Step 1: Use AppleScript to find and select the right window/tab/session
    let script = format!(
        r#"tell application "iTerm2"
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if tty of s is "{tty}" then
                    tell w to select t
                    select s
                    return true
                end if
            end repeat
        end repeat
    end repeat
    return false
end tell"#,
        tty = tty
    );

    if !run_applescript_bool(&script) {
        return false;
    }

    // Step 2: Use `open -a` to reliably activate iTerm2.
    // AppleScript's `activate` doesn't work reliably when called from
    // a Tauri (non-terminal) app context; `open -a` is more robust.
    let _ = Command::new("open").args(["-a", "iTerm"]).output();
    true
}

fn focus_terminal_app(tty: &str) -> bool {
    let script = format!(
        r#"tell application "Terminal"
    set found to false
    repeat with w in windows
        repeat with t in tabs of w
            if tty of t is "{tty}" then
                set index of w to 1
                set selected tab of w to t
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

    if !run_applescript_bool(&script) {
        return false;
    }

    let _ = Command::new("open").args(["-a", "Terminal"]).output();
    true
}

fn run_applescript_bool(script: &str) -> bool {
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
