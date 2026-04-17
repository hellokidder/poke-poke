use std::process::Command;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let tray_icon = Image::from_bytes(include_bytes!("../icons/tray-default.png"))
        .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/32x32.png")).unwrap());

    let menu = build_tray_menu(app)?;

    TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .icon_as_template(false)
        .tooltip("Poke Poke")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "cc_toggle" => handle_cc_toggle(app),
            "codex_toggle" => handle_codex_toggle(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_panel(tray.app_handle(), position.x, position.y);
            }
        })
        .build(app)?;

    Ok(())
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let cc_label = if is_cc_connected() {
        "Claude Code Connected \u{2713}"
    } else {
        "Connect Claude Code"
    };
    let codex_label = if is_codex_connected() {
        "Codex CLI Connected \u{2713}"
    } else {
        "Connect Codex CLI"
    };

    let cc_item = MenuItemBuilder::with_id("cc_toggle", cc_label).build(app)?;
    let codex_item = MenuItemBuilder::with_id("codex_toggle", codex_label).build(app)?;
    let cursor_item = MenuItemBuilder::with_id("cursor_info", "Cursor: poke-hook --install-cursor <path>")
        .enabled(false)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    MenuBuilder::new(app)
        .item(&cc_item)
        .item(&codex_item)
        .item(&cursor_item)
        .separator()
        .item(&quit_item)
        .build()
}

fn is_cc_connected() -> bool {
    let hook_path = hook_bin_path();
    if !hook_path.exists() {
        return false;
    }
    Command::new(&hook_path)
        .arg("--check")
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str::<serde_json::Value>(out.trim()).ok()
        })
        .and_then(|v| v["connected"].as_bool())
        .unwrap_or(false)
}

fn is_codex_connected() -> bool {
    let hook_path = hook_bin_path();
    if !hook_path.exists() {
        return false;
    }
    Command::new(&hook_path)
        .arg("--check-codex")
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str::<serde_json::Value>(out.trim()).ok()
        })
        .and_then(|v| v["connected"].as_bool())
        .unwrap_or(false)
}

fn handle_cc_toggle(app: &AppHandle) {
    let hook_path = hook_bin_path();
    let connected = is_cc_connected();
    let arg = if connected { "--uninstall" } else { "--install" };

    let bin = if hook_path.exists() {
        hook_path
    } else {
        env_fallback_path()
    };

    if bin.exists() {
        let _ = Command::new(&bin).arg(arg).output();
    } else {
        eprintln!("[PokePoke] poke-hook binary not found");
        return;
    }

    // Rebuild menu to reflect new state
    if let Ok(menu) = build_tray_menu(app) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn handle_codex_toggle(app: &AppHandle) {
    let hook_path = hook_bin_path();
    let connected = is_codex_connected();
    let arg = if connected {
        "--uninstall-codex"
    } else {
        "--install-codex"
    };

    let bin = if hook_path.exists() {
        hook_path
    } else {
        env_fallback_path()
    };

    if bin.exists() {
        let _ = Command::new(&bin).arg(arg).output();
    } else {
        eprintln!("[PokePoke] poke-hook binary not found");
        return;
    }

    if let Ok(menu) = build_tray_menu(app) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn hook_bin_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".local/bin/poke-hook")
}

fn env_fallback_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("poke-hook")
}

const PANEL_W: f64 = 380.0;
const PANEL_H: f64 = 520.0;

fn toggle_panel(app: &AppHandle, click_x: f64, _click_y: f64) {
    if let Some(window) = app.get_webview_window("panel") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
        return;
    }

    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let logical_x = click_x / scale;
    let (screen_width, _) = get_screen_size(app);

    let x = (logical_x - PANEL_W / 2.0).clamp(8.0, screen_width - PANEL_W - 8.0);
    let y = 30.0;

    create_panel_window(app, x, y);
}


fn create_panel_window(app: &AppHandle, x: f64, y: f64) {
    let builder = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("Poke Poke")
        .inner_size(PANEL_W, PANEL_H)
        .position(x, y)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .shadow(false)
        .transparent(true);

    if let Ok(window) = builder.build() {
        let app_handle = app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if let Some(win) = app_handle.get_webview_window("panel") {
                    let _ = win.hide();
                }
            }
        });
    }
}

const SETTINGS_W: f64 = 560.0;
const SETTINGS_H: f64 = 660.0;

/// Open a standalone settings window (singleton). If already open, focus it.
pub fn open_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        #[cfg(target_os = "macos")]
        {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        }
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    create_settings_window(app);
}

/// Toggle the settings window: show if hidden/absent, hide if visible.
/// Used by the global shortcut.
pub fn toggle_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.destroy();
        } else {
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            }
            let _ = win.show();
            let _ = win.set_focus();
        }
        return;
    }
    create_settings_window(app);
}

fn create_settings_window(app: &AppHandle) {
    let (screen_w, screen_h) = get_screen_size(app);
    let x = (screen_w - SETTINGS_W) / 2.0;
    let y = (screen_h - SETTINGS_H) / 2.0;

    // Settings 是需要前台交互的窗口：临时切到 Regular 让它能正常获得焦点、
    // 出现在 Dock / Cmd+Tab 里；窗口关闭时再切回 Accessory，避免 popup 抢焦点。
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }

    let built = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("")
        .inner_size(SETTINGS_W, SETTINGS_H)
        .position(x, y)
        .decorations(false)
        .always_on_top(false)
        .resizable(false)
        .skip_taskbar(false)
        .shadow(true)
        .transparent(true)
        .build();

    if let Ok(window) = built {
        let _ = window.set_focus();
        let app_handle = app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event {
                #[cfg(target_os = "macos")]
                {
                    let _ = app_handle
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = &app_handle;
                }
            }
        });
    } else {
        // 构建失败也要把激活策略还原
        #[cfg(target_os = "macos")]
        {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
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

