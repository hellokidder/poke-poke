mod commands;
mod http_server;
mod popup;
mod sessions;
mod settings;
mod shortcut;
mod sound;
mod tray;

use sessions::{SessionStatus, SessionStore};
use settings::SettingsStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .setup(|app| {
            // macOS 的 "菜单栏工具" 模式通过 Info.plist 的 LSUIElement=true 声明
            // （见 tauri.conf.json / 打包产物的 Info.plist）。这样 NSApp 从启动
            // 之初就不是 Regular app，创建窗口时不会激活本进程、不抢用户输入焦点。
            // 这里不再运行时调用 set_activation_policy——实测会导致 dev 下 NSApp
            // 在 finishLaunching 时立刻退出。

            let data_dir = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));

            // Migrate legacy data file name
            let legacy_path = data_dir.join("notifications.json");
            let store_path = data_dir.join("sessions.json");
            if !store_path.exists() && legacy_path.exists() {
                let _ = std::fs::rename(&legacy_path, &store_path);
            }

            let store = Arc::new(Mutex::new(SessionStore::load(store_path)));
            let popup_list = popup::create_popup_list();

            let settings_path = data_dir.join("settings.json");
            let settings_store = Arc::new(Mutex::new(SettingsStore::load(settings_path)));

            app.handle().manage(store.clone());
            app.handle().manage(popup_list.clone());
            app.handle().manage(settings_store.clone());

            tray::create_tray(app.handle())?;

            // Register saved global shortcut
            shortcut::apply_shortcut(app.handle());

            let app_handle = app.handle().clone();
            let store_clone = store.clone();
            let popup_clone = popup_list.clone();
            let settings_clone = settings_store.clone();
            tauri::async_runtime::spawn(async move {
                http_server::start(app_handle, store_clone, popup_clone, settings_clone).await;
            });

            // High-frequency liveness probe thread: every 5s
            // Checks Running + Pending sessions; removes those whose host has disappeared
            // after 2 consecutive missed probes (grace period).
            {
                let probe_store = store.clone();
                let probe_popup = popup_list.clone();
                let probe_app = app.handle().clone();
                std::thread::spawn(move || {
                    let mut miss_counts: HashMap<String, u32> = HashMap::new();
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(5));

                        let sessions: Vec<_> = probe_store
                            .lock()
                            .unwrap()
                            .get_all()
                            .iter()
                            .filter(|s| {
                                matches!(
                                    s.status,
                                    SessionStatus::Running | SessionStatus::Pending
                                )
                            })
                            .cloned()
                            .collect();

                        let mut to_remove: Vec<String> = Vec::new();

                        for session in &sessions {
                            // P0: TTY stat for terminal sessions; no-TTY sessions (Cursor etc.)
                            // are treated as alive until P1-B adds process-level probing.
                            let alive = match session.terminal_tty.as_deref() {
                                Some(tty) if !tty.is_empty() => {
                                    std::path::Path::new(tty).exists()
                                }
                                _ => true,
                            };

                            if alive {
                                miss_counts.remove(&session.id);
                            } else {
                                let count =
                                    miss_counts.entry(session.id.clone()).or_insert(0);
                                *count += 1;
                                if *count >= 2 {
                                    to_remove.push(session.id.clone());
                                }
                            }
                        }

                        // Clean up miss_counts entries for sessions no longer active
                        miss_counts.retain(|id, _| sessions.iter().any(|s| &s.id == id));

                        // Perform removes outside the sessions borrow.
                        // Re-check both status AND current liveness before each remove to
                        // guard against two race classes:
                        //   1. Hook event changed status to Success between snapshot and now.
                        //   2. Hook event brought a new terminal_tty between snapshot and now
                        //      (session migrated to a new terminal); old TTY was gone but new
                        //      one is valid — removing would silently kill a live session.
                        let mut removed_any = false;
                        for session_id in &to_remove {
                            miss_counts.remove(session_id);
                            let should_remove = probe_store
                                .lock()
                                .unwrap()
                                .get_all()
                                .iter()
                                .find(|s| s.id == *session_id)
                                .is_some_and(|s| {
                                    matches!(
                                        s.status,
                                        SessionStatus::Running | SessionStatus::Pending
                                    ) && match s.terminal_tty.as_deref() {
                                        Some(tty) if !tty.is_empty() => {
                                            !std::path::Path::new(tty).exists()
                                        }
                                        // No TTY: treat as alive (P0 — Cursor etc.)
                                        _ => false,
                                    }
                                });
                            if should_remove {
                                remove_session_with_cleanup(
                                    &probe_app,
                                    &probe_store,
                                    &probe_popup,
                                    session_id,
                                );
                                removed_any = true;
                            }
                        }

                        // Emit once after all removes to avoid per-remove redraws
                        if removed_any {
                            let _ = probe_app.emit("sessions-updated", ());
                        }
                    }
                });
            }

            // Low-frequency TTL cleanup thread: every 1 hour
            // Only removes Success sessions older than 24h; does not touch active sessions.
            {
                let cleanup_store = store.clone();
                let cleanup_app = app.handle().clone();
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                        let cleaned = cleanup_store.lock().unwrap().cleanup_expired(24);
                        if cleaned > 0 {
                            let _ = cleanup_app.emit("sessions-updated", ());
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_session_by_id,
            commands::remove_session,
            commands::close_popup_window,
            commands::open_session_source,
            commands::focus_session_terminal,
            commands::check_cc_integration,
            commands::check_codex_integration,
            commands::check_cursor_integration,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings_window,
            commands::close_settings_window,
            commands::list_system_sounds,
            commands::preview_sound,
        ])
        .build(tauri::generate_context!())
        .expect("error while building PokePoke")
        .run(|_app_handle, event| {
            // NSApp finishLaunching 之后才切换激活策略。
            // 放在这个时机的原因：
            //   - dev 模式下裸二进制不读 Info.plist，LSUIElement 只在打包产物生效
            //   - 直接在 setup 里同步切会早于 finishLaunching，导致 NSApp 启动即退出
            //   - Ready 事件保证 run loop 已稳定进入
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Ready = event {
                let _ = _app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}

/// Close the popup associated with a session and remove it from the store.
/// Does NOT emit "sessions-updated" — caller is responsible for batching emits.
fn remove_session_with_cleanup(
    app: &tauri::AppHandle,
    store: &Arc<Mutex<SessionStore>>,
    popup_list: &popup::PopupList,
    session_id: &str,
) {
    popup::close_popup(app, session_id, popup_list);
    store.lock().unwrap().remove_session(session_id);
    // Caller emits "sessions-updated" to allow batching
}

fn dirs_next() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::PathBuf::from(home).join(".pokepoke");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
