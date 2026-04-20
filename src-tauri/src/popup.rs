use crate::sessions::Session;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl};

#[cfg(not(target_os = "macos"))]
use tauri::WebviewWindowBuilder;

// macOS：用 tauri-nspanel 把 popup 直接构造成 NSPanel
// （can_become_key_window=false、no_activate=true），
// 从窗口模型层面彻底避免 popup 抢焦点。
#[cfg(target_os = "macos")]
use tauri::{LogicalPosition, LogicalSize, Position, Size};
#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

#[cfg(target_os = "macos")]
tauri_panel! {
    // PokePoke popup 的 NSPanel 子类：
    //   - can_become_key_window=false：键盘事件永远不走 popup，用户输入不被打断
    //   - can_become_main_window=false：不做 main window，不激活 app
    //   - is_floating_panel=true：浮动在普通窗口之上，等价于 NSFloatingWindowLevel
    panel!(PokePopupPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

const POPUP_WIDTH: f64 = 360.0;
const POPUP_HEIGHT: f64 = 150.0;
const POPUP_MARGIN: f64 = 12.0;
const POPUP_GAP: f64 = 8.0;

pub type PopupList = Arc<Mutex<Vec<String>>>;

pub fn create_popup_list() -> PopupList {
    Arc::new(Mutex::new(Vec::new()))
}

pub fn show_popup(app: &AppHandle, session: &Session, popup_list: &PopupList) {
    let label = format!("popup-{}", session.id);

    let (x, y) = calculate_position(app, popup_list);

    // macOS：NSPanel 的创建与 AppKit 初始化必须在主线程，否则会直接
    // 让进程崩溃（不返回 Err、不 panic，而是 AppKit 内部 assertion 触发 abort）。
    // http_server 的 handler 跑在 tokio worker 上，所以这里必须 dispatch。
    //
    // 同时：popup 的创建结果需要在当前调用里用来决定"是否写入 popup_list"，
    // 我们走"乐观更新 + 失败回滚"策略：先记录 label，然后在主线程跑构建；
    // 若构建失败则 async 地从 list 中移除。
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut list) = popup_list.lock() {
            list.push(label.clone());
        }

        let app_for_build = app.clone();
        let popup_list_for_rollback = popup_list.clone();
        let label_for_build = label.clone();
        let _ = app.run_on_main_thread(move || {
            let ok = build_popup_window(&app_for_build, &label_for_build, x, y);
            if !ok {
                if let Ok(mut list) = popup_list_for_rollback.lock() {
                    list.retain(|l| l != &label_for_build);
                }
            }
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        let build_ok = build_popup_window(app, &label, x, y);
        if build_ok {
            if let Ok(mut list) = popup_list.lock() {
                list.push(label.clone());
            }
        }
    }

    // Auto-dismiss when user focuses the associated terminal session
    if let Some(ref tty) = session.terminal_tty {
        let app_clone = app.clone();
        let popup_list_clone = popup_list.clone();
        let id = session.id.clone();
        let tty = tty.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(1500));
                let still_exists = popup_list_clone
                    .lock()
                    .map(|list| list.contains(&format!("popup-{}", id)))
                    .unwrap_or(false);
                if !still_exists {
                    break;
                }
                if is_terminal_session_focused(&tty) {
                    close_popup(&app_clone, &id, &popup_list_clone);
                    break;
                }
            }
        });
    }
}

/// macOS：走 PanelBuilder 构造 NSPanel（不抢焦点）
#[cfg(target_os = "macos")]
fn build_popup_window(app: &AppHandle, label: &str, x: f64, y: f64) -> bool {
    let result = PanelBuilder::<_, PokePopupPanel>::new(app, label)
        .url(WebviewUrl::App("index.html".into()))
        .position(Position::Logical(LogicalPosition::new(x, y)))
        .size(Size::Logical(LogicalSize::new(POPUP_WIDTH, POPUP_HEIGHT)))
        .level(PanelLevel::Floating)
        .has_shadow(false)
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .stationary()
                .full_screen_auxiliary(),
        )
        .hides_on_deactivate(false)
        // 必须显式 nonactivating_panel：告诉 AppKit 这个 panel 被点击时
        // 不让所属 app 激活到前台。
        .style_mask(StyleMask::empty().nonactivating_panel())
        // 创建期就声明不激活本进程；这样即便有第一次 show 的瞬间也不抢 key window。
        .no_activate(true)
        .with_window(|w| {
            w.title("")
                .decorations(false)
                .resizable(false)
                .transparent(true)
                .accept_first_mouse(true)
                .skip_taskbar(true)
        })
        .build();

    match result {
        Ok(panel) => {
            // show() 而非 show_and_make_key()：显示 panel 但不让它成为 key window。
            panel.show();
            true
        }
        Err(e) => {
            eprintln!("[PokePoke] Failed to create popup panel: {}", e);
            false
        }
    }
}

/// 非 macOS：沿用原来的 WebviewWindowBuilder
#[cfg(not(target_os = "macos"))]
fn build_popup_window(app: &AppHandle, label: &str, x: f64, y: f64) -> bool {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
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
        .accept_first_mouse(true)
        .build();

    match window {
        Ok(win) => {
            let _ = win.show();
            true
        }
        Err(e) => {
            eprintln!("[PokePoke] Failed to create popup: {}", e);
            false
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

    // macOS：panel 有独立生命周期，优先走 panel API；退回 webview window。
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel(&label) {
            if let Some(win) = panel.to_window() {
                let _ = win.close();
            }
        } else if let Some(win) = app.get_webview_window(&label) {
            let _ = win.destroy();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.destroy();
        }
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

/// Narrow check: is the user actively viewing this exact terminal session?
/// Only checks the current session of the current tab of the front window.
/// Used to skip popup creation when the user is already looking at that session.
pub fn is_terminal_session_focused(tty: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::target_y;

    #[test]
    fn target_y_for_first_popup_starts_below_menu_bar() {
        assert_eq!(target_y(0), 12.0 + 30.0);
    }

    #[test]
    fn target_y_for_second_popup_adds_height_and_gap() {
        assert_eq!(target_y(1), 12.0 + 30.0 + 150.0 + 8.0);
    }
}

