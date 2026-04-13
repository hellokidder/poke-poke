use crate::notifications::{Task, TaskStore};
use crate::tray;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const POPUP_WIDTH: f64 = 360.0;
const POPUP_HEIGHT: f64 = 150.0;
const POPUP_MARGIN: f64 = 12.0;
const POPUP_GAP: f64 = 8.0;

pub type PopupList = Arc<Mutex<Vec<String>>>;

pub fn create_popup_list() -> PopupList {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn show_popup(app: &AppHandle, task: &Task, popup_list: &PopupList) {
    let label = format!("popup-{}", task.id);

    let (x, y) = calculate_position(app, popup_list);

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("")
        .inner_size(POPUP_WIDTH, POPUP_HEIGHT)
        .position(x, y)
        .decorations(false)
        .always_on_top(true)
        .focused(false)
        .skip_taskbar(true)
        .resizable(false)
        .transparent(true)
        .shadow(false)
        .visible(true)
        .accept_first_mouse(true)
        .build();

    match window {
        Ok(_) => {
            if let Ok(mut list) = popup_list.lock() {
                list.push(label);
            }

            // Auto-dismiss: monitor if the user switches to the matching terminal
            if let Some(ref tty) = task.terminal_tty {
                if !tty.is_empty() {
                    start_focus_monitor(
                        app.clone(),
                        task.id.clone(),
                        tty.clone(),
                        popup_list.clone(),
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("[PokePoke] Failed to create popup: {}", e);
        }
    }
}

fn calculate_position(app: &AppHandle, popup_list: &PopupList) -> (f64, f64) {
    let existing = popup_list.lock().map(|l| l.len()).unwrap_or(0);
    let (screen_width, _) = get_screen_size(app);
    let x = screen_width - POPUP_WIDTH - POPUP_MARGIN;
    let y = POPUP_MARGIN + 30.0 + (existing as f64 * (POPUP_HEIGHT + POPUP_GAP));
    (x, y)
}

fn get_screen_size(app: &AppHandle) -> (f64, f64) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        (size.width as f64 / scale, size.height as f64 / scale)
    } else {
        (1920.0, 1080.0)
    }
}

/// Target Y for popup at given index
fn target_y(index: usize) -> f64 {
    POPUP_MARGIN + 30.0 + (index as f64 * (POPUP_HEIGHT + POPUP_GAP))
}

pub fn close_popup(app: &AppHandle, id: &str, popup_list: &PopupList) {
    let label = format!("popup-{}", id);

    // Find index of removed popup so we know which ones need to animate
    let removed_index = if let Ok(mut list) = popup_list.lock() {
        let idx = list.iter().position(|l| l == &label);
        list.retain(|l| l != &label);
        idx
    } else {
        None
    };

    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.destroy();
    }

    if let Some(removed_idx) = removed_index {
        let app_clone = app.clone();
        let popup_list_clone = popup_list.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            animate_reposition(&app_clone, &popup_list_clone, removed_idx);
        });
    }
}

/// Smoothly animate remaining popups to their new positions after one is removed
fn animate_reposition(app: &AppHandle, popup_list: &PopupList, removed_idx: usize) {
    let labels = if let Ok(list) = popup_list.lock() {
        list.clone()
    } else {
        return;
    };

    let (screen_width, _) = get_screen_size(app);
    let x = screen_width - POPUP_WIDTH - POPUP_MARGIN;

    // Only popups at or after the removed index need to move
    // Their old position was target_y(actual_index + 1), new position is target_y(actual_index)
    let slide_delta = POPUP_HEIGHT + POPUP_GAP;
    let steps = 14;
    let frame_ms = 16; // ~60fps

    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        // ease-out cubic for natural deceleration
        let eased = 1.0 - (1.0 - t).powi(3);

        for (i, label) in labels.iter().enumerate().skip(removed_idx) {
            let final_y = target_y(i);
            let start_y = final_y + slide_delta;
            let y = start_y + (final_y - start_y) * eased;

            if let Some(win) = app.get_webview_window(label) {
                let _ = win.set_position(tauri::Position::Logical(
                    tauri::LogicalPosition::new(x, y),
                ));
            }
        }

        std::thread::sleep(Duration::from_millis(frame_ms));
    }
}

/// Poll every second to check if the user switched to the terminal
/// matching this popup's tty. If so, auto-dismiss the popup.
fn start_focus_monitor(app: AppHandle, task_id: String, tty: String, popup_list: PopupList) {
    std::thread::spawn(move || {
        let label = format!("popup-{}", task_id);

        loop {
            std::thread::sleep(Duration::from_secs(1));

            // Stop if popup was already closed (by click or other means)
            if app.get_webview_window(&label).is_none() {
                break;
            }

            if is_terminal_session_focused(&tty) {
                // The user is looking at the matching terminal — dismiss popup
                close_popup(&app, &task_id, &popup_list);

                let store = app.state::<Arc<Mutex<TaskStore>>>();
                let unread = {
                    let mut s = store.lock().unwrap();
                    s.mark_read(&task_id);
                    s.unread_count()
                };
                tray::update_tray_icon(&app, unread);
                let _ = app.emit("notifications-updated", ());
                break;
            }
        }
    });
}

/// Check if the frontmost terminal app's current session matches the given tty.
fn is_terminal_session_focused(tty: &str) -> bool {
    let script = format!(
        r#"tell application "System Events"
    set frontApp to name of first application process whose frontmost is true
end tell

if frontApp is "iTerm2" then
    tell application "iTerm2"
        try
            return (tty of current session of current tab of current window) is "{tty}"
        end try
    end tell
else if frontApp is "Terminal" then
    tell application "Terminal"
        try
            return (tty of selected tab of front window) is "{tty}"
        end try
    end tell
end if
return false"#,
        tty = tty
    );

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}
