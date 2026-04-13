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
    let label = if is_cc_connected() {
        "Claude Code Connected ✓"
    } else {
        "Connect Claude Code"
    };

    let connect_item = MenuItemBuilder::with_id("cc_toggle", label).build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    MenuBuilder::new(app)
        .item(&connect_item)
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

    let panel_w = 380.0;
    let panel_h = 520.0;

    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let logical_x = click_x / scale;
    let (screen_width, _) = get_screen_size(app);

    let x = (logical_x - panel_w / 2.0).clamp(8.0, screen_width - panel_w - 8.0);
    let y = 30.0;

    let builder = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("Poke Poke")
        .inner_size(panel_w, panel_h)
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

fn get_screen_size(app: &AppHandle) -> (f64, f64) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        (size.width as f64 / scale, size.height as f64 / scale)
    } else {
        (1920.0, 1080.0)
    }
}

pub fn update_tray_icon(app: &AppHandle, unread_count: usize) {
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = if unread_count > 0 {
            format!("Poke Poke - {} unread", unread_count)
        } else {
            "Poke Poke".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}
