use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let tray_icon = Image::from_path("icons/tray-default.png")
        .or_else(|_| Image::from_bytes(include_bytes!("../icons/tray-default.png")))
        .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/32x32.png")).unwrap());

    TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .icon_as_template(true)
        .tooltip("Poke Poke")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                // Use click position to place panel below the tray icon
                toggle_panel(tray.app_handle(), position.x, position.y);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_panel(app: &AppHandle, click_x: f64, click_y: f64) {
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

    // Convert physical click position to logical, place panel below menubar
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let logical_x = click_x / scale;
    let (screen_width, _) = get_screen_size(app);

    let x = (logical_x - panel_w / 2.0).clamp(8.0, screen_width - panel_w - 8.0);
    let y = 30.0; // just below macOS menubar

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
    let icon_bytes: &[u8] = if unread_count > 0 {
        include_bytes!("../icons/tray-unread.png")
    } else {
        include_bytes!("../icons/tray-default.png")
    };

    if let Ok(icon) = Image::from_bytes(icon_bytes) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_icon(Some(icon));
            let tooltip = if unread_count > 0 {
                format!("Poke Poke - {} unread", unread_count)
            } else {
                "Poke Poke".to_string()
            };
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
}
