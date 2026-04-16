use crate::popup::{self, PopupList};
use crate::sessions::{Session, SessionStore};
use crate::settings::{Settings, SettingsStore};
use crate::shortcut;
use crate::tray;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_sessions(
    store: State<'_, Arc<Mutex<SessionStore>>>,
) -> Vec<Session> {
    store.lock().unwrap().get_all().to_vec()
}

#[tauri::command]
pub fn get_session_by_id(
    id: String,
    store: State<'_, Arc<Mutex<SessionStore>>>,
) -> Option<Session> {
    let s = store.lock().unwrap();
    s.get_all().iter().find(|s| s.id == id).cloned()
}

/// Focus the terminal that matches the session's tty.
/// Does NOT close the popup — the frontend handles that separately
/// to avoid destroying the calling webview mid-command.
#[tauri::command]
pub fn focus_session_terminal(
    id: String,
    store: State<'_, Arc<Mutex<SessionStore>>>,
) {
    let (terminal_tty, workspace_path, source) = {
        let s = store.lock().unwrap();
        let session = s.get_all().iter().find(|s| s.id == id).cloned();
        match session {
            Some(s) => (s.terminal_tty, s.workspace_path, s.source),
            None => return,
        }
    };

    if let Some(ref tty) = terminal_tty {
        if !tty.is_empty() && focus_terminal(tty) {
            return;
        }
    }

    // For Cursor sessions, use `cursor` CLI to focus the editor window
    if let Some(ref src) = source {
        if src == "cursor" {
            if let Some(ref path) = workspace_path {
                if !path.is_empty() {
                    focus_cursor_workspace(path);
                    return;
                }
            }
        }
    }

    // Fallback: open workspace path in Finder
    if let Some(ref path) = workspace_path {
        if !path.is_empty() {
            let _ = Command::new("open").arg(path).output();
        }
    }
}

/// Focus the terminal window that spawned the session (used by panel clicks).
#[tauri::command]
pub fn open_session_source(
    id: String,
    store: State<'_, Arc<Mutex<SessionStore>>>,
) {
    let (terminal_tty, workspace_path, source) = {
        let s = store.lock().unwrap();
        let session = s.get_all().iter().find(|s| s.id == id).cloned();
        match session {
            Some(s) => (s.terminal_tty, s.workspace_path, s.source),
            None => return,
        }
    };

    if let Some(ref tty) = terminal_tty {
        if !tty.is_empty() && focus_terminal(tty) {
            return;
        }
    }

    // For Cursor sessions, use `cursor` CLI to focus the editor window
    if let Some(ref src) = source {
        if src == "cursor" {
            if let Some(ref path) = workspace_path {
                if !path.is_empty() {
                    focus_cursor_workspace(path);
                    return;
                }
            }
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
    if is_app_running("iTerm2") && focus_iterm2(tty) {
        return true;
    }
    if is_app_running("Terminal") && focus_terminal_app(tty) {
        return true;
    }
    false
}

fn focus_iterm2(tty: &str) -> bool {
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

/// Focus a Cursor editor window for the given workspace path.
/// Uses `cursor` CLI which reuses the existing window instead of
/// opening the GUI composer dialog (which `open -a Cursor` tends to do).
fn focus_cursor_workspace(path: &str) {
    let _ = Command::new("cursor").arg(path).output();
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
pub fn check_cc_integration() -> serde_json::Value {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let hook_path = std::path::PathBuf::from(&home).join(".local/bin/poke-hook");
    if !hook_path.exists() {
        return serde_json::json!({"installed": false, "hooks_configured": false, "connected": false});
    }
    Command::new(&hook_path)
        .arg("--check")
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(out.trim()).ok()
        })
        .unwrap_or(serde_json::json!({"installed": false, "hooks_configured": false, "connected": false}))
}

#[tauri::command]
pub fn check_codex_integration() -> serde_json::Value {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let hook_path = std::path::PathBuf::from(&home).join(".local/bin/poke-hook");
    if !hook_path.exists() {
        return serde_json::json!({"installed": false, "hooks_configured": false, "connected": false});
    }
    Command::new(&hook_path)
        .arg("--check-codex")
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(out.trim()).ok()
        })
        .unwrap_or(serde_json::json!({"installed": false, "hooks_configured": false, "connected": false}))
}

#[tauri::command]
pub fn check_cursor_integration(project_path: String) -> serde_json::Value {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let hook_path = std::path::PathBuf::from(&home).join(".local/bin/poke-hook");
    if !hook_path.exists() {
        return serde_json::json!({"installed": false, "hooks_configured": false, "connected": false});
    }
    Command::new(&hook_path)
        .args(["--check-cursor", &project_path])
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(out.trim()).ok()
        })
        .unwrap_or(serde_json::json!({"installed": false, "hooks_configured": false, "connected": false}))
}

#[tauri::command]
pub fn remove_session(
    id: String,
    store: State<'_, Arc<Mutex<SessionStore>>>,
    popup_list: State<'_, PopupList>,
    app: AppHandle,
) {
    {
        let mut s = store.lock().unwrap();
        s.remove_session(&id);
    }
    popup::close_popup(&app, &id, &popup_list);
    let _ = app.emit("sessions-updated", ());
}

#[tauri::command]
pub fn close_popup_window(
    id: String,
    popup_list: State<'_, PopupList>,
    app: AppHandle,
) {
    popup::close_popup(&app, &id, &popup_list);
}

#[tauri::command]
pub fn get_settings(store: State<'_, Arc<Mutex<SettingsStore>>>) -> Settings {
    store.lock().unwrap().settings.clone()
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    store: State<'_, Arc<Mutex<SettingsStore>>>,
    app: AppHandle,
) {
    {
        let mut s = store.lock().unwrap();
        s.update(settings);
    }
    shortcut::apply_shortcut(&app);
    let _ = app.emit("settings-updated", ());
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    tray::open_settings_window(&app);
}

#[tauri::command]
pub fn close_settings_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.destroy();
    }
}

#[tauri::command]
pub fn list_system_sounds() -> Vec<String> {
    crate::sound::list_system_sounds()
}

#[tauri::command]
pub fn preview_sound(name: String) {
    crate::sound::play_sound_by_name(&name);
}
