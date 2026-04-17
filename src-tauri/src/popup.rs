use crate::sessions::Session;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
mod macos_panel {
    //! 把 Tauri 创建出来的普通 NSWindow 转成 non-activating NSPanel，
    //! 这样窗口显示时不会把 PokePoke 激活到前台、不抢用户的键盘焦点。
    //!
    //! 原因：Tauri v2 的 .focused(false) 在 macOS 上实际不生效
    //! (tao 的 TaoWindow 硬编码 canBecomeKeyWindow = YES)。
    //! 我们直接通过 objc2 设置 NSWindow 的 styleMask 和 collectionBehavior。

    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    // AppKit 常量；objc2-app-kit 没导出这些 bitflag 的裸值，直接按 Apple 头文件抄。
    const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: u64 = 1 << 7;
    // NSWindowCollectionBehavior
    const NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
    const NS_WINDOW_COLLECTION_BEHAVIOR_STATIONARY: u64 = 1 << 4;
    const NS_WINDOW_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

    /// 将指定 NSWindow 配置为 non-activating 面板并前置展示。
    /// 必须在主线程调用。
    ///
    /// # Safety
    /// `ns_window` 必须是有效的 NSWindow 指针（由 Tauri `ns_window()` 返回）。
    pub unsafe fn make_non_activating_panel(ns_window: *mut std::ffi::c_void) {
        if ns_window.is_null() {
            return;
        }
        let window: &AnyObject = &*(ns_window as *mut AnyObject);

        // 追加 NonactivatingPanel 位，让 NSWindow 表现得像 NSPanel：
        // 不会成为 key window / main window，因而不会激活所属 app。
        let current_mask: u64 = msg_send![window, styleMask];
        let new_mask = current_mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL;
        let _: () = msg_send![window, setStyleMask: new_mask];

        // 浮动：所有 Space 可见 + 全屏下也显示 + 不随 Space 切换（stationary）
        let behavior = NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES
            | NS_WINDOW_COLLECTION_BEHAVIOR_STATIONARY
            | NS_WINDOW_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY;
        let _: () = msg_send![window, setCollectionBehavior: behavior];

        // 不让窗口变成 main/key window（某些情况下点击也不会）
        let _: () = msg_send![window, setHidesOnDeactivate: false];

        // 用 orderFrontRegardless 取代 makeKeyAndOrderFront：
        // 显示窗口但不抢 key 状态，也不激活 app。
        let _: () = msg_send![window, orderFrontRegardless];
    }
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

    // 注意：这里 visible(false) + 构建后手动 order-front-regardless。
    // 如果让 Tauri 自己 show，会调用 makeKeyAndOrderFront 抢焦点。
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
        .visible(false)
        .accept_first_mouse(true)
        .build();

    match window {
        Ok(win) => {
            // macOS：把这个普通 NSWindow 转成 non-activating NSPanel 再显示，
            // 彻底避免弹窗激活 app、打断用户输入。
            // 注意：AppKit 调用（setStyleMask / orderFrontRegardless）必须在主线程，
            // 否则会被静默忽略，表现就是"弹窗不出现"。
            // show_popup 被 axum handler 调用时在 tokio worker 线程上，必须 dispatch。
            #[cfg(target_os = "macos")]
            {
                let win_for_main = win.clone();
                let win_fallback = win.clone();
                let dispatched = win
                    .run_on_main_thread(move || {
                        if let Ok(ns_window) = win_for_main.ns_window() {
                            unsafe {
                                macos_panel::make_non_activating_panel(ns_window);
                            }
                        } else {
                            let _ = win_for_main.show();
                        }
                    })
                    .is_ok();
                if !dispatched {
                    // dispatch 失败时至少让窗口显示出来，避免"弹窗丢失"
                    let _ = win_fallback.show();
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = win.show();
            }

            if let Ok(mut list) = popup_list.lock() {
                list.push(label);
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

