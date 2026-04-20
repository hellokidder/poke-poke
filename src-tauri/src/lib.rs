mod commands;
#[path = "bin/hook.rs"]
pub mod hook_cli;
mod http_server;
mod popup;
mod sessions;
mod settings;
mod shortcut;
mod sound;
mod tray;

use sessions::{Session, SessionStore};
use settings::SettingsStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if hook_cli::should_run_from_current_process() {
        hook_cli::main();
        return;
    }

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

            // 启动时一次性全量探活：Task C 的新语义下 session = 活着的 agent。
            // 老版本（含 Task A/B）遗留的 Idle/LastFailed session 宿主大概率已死；
            // 等第一次探活循环还要 5s，用户看到面板瞬间"积攒历史"体验不好，
            // 这里在 setup 里同步跑一次，只清理"能 100% 确认宿主已死"的 session。
            // miss_count 不生效——启动清理只 reap 明确死亡的，不加 grace period。
            {
                let mut guard = store.lock().unwrap();
                let dead_ids: Vec<String> = guard
                    .get_all()
                    .iter()
                    .filter(|s| !is_session_alive(s))
                    .map(|s| s.id.clone())
                    .collect();
                for id in &dead_ids {
                    guard.remove_session(id);
                }
                if !dead_ids.is_empty() {
                    eprintln!(
                        "[PokePoke] startup reap: removed {} dead session(s)",
                        dead_ids.len()
                    );
                }
            }

            // 高频探活线程：每 5 秒对所有 session（无状态过滤）做一次探活。
            // Task C 新语义：Idle / LastFailed 不再是终态，同样要被探活。
            // 连续 2 次 miss 才 reap（grace period = 10 秒），防止 pgrep 偶发
            // 权限/瞬时失败导致误杀活 agent。
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
                            .to_vec();

                        let mut to_remove: Vec<String> = Vec::new();

                        for session in &sessions {
                            if is_session_alive(session) {
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

                        // 清理已不存在的 session 对应的 miss_counts
                        miss_counts.retain(|id, _| sessions.iter().any(|s| &s.id == id));

                        // 二次确认后再删：snapshot 到 remove 之间可能有 hook 事件把
                        // session 的 terminal_tty 或 source 换掉（比如 agent 在另一
                        // 个终端复活），这时应该让它继续活着。
                        let mut removed_any = false;
                        for session_id in &to_remove {
                            miss_counts.remove(session_id);
                            let should_remove = probe_store
                                .lock()
                                .unwrap()
                                .get_all()
                                .iter()
                                .find(|s| s.id == *session_id)
                                .is_some_and(|s| !is_session_alive(s));
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

                        // 批量 emit 一次，避免按条刷新前端
                        if removed_any {
                            let _ = probe_app.emit("sessions-updated", ());
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
            commands::repair_cc_integration,
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

/// 判断一个 session 的宿主（agent 进程 / IDE）是否还活着。
///
/// Task C 决策 4：严格按 source 分层，没有"TTY stat 兜底"。
/// 识别不出 source、或缺关键字段（有 source 但没 TTY 的 CLI agent），
/// 都直接判死——"宁可误清也不留僵尸"。
///
/// 平台：仅 macOS 实现 pgrep 调用；Linux/Windows 先 fallback 到 TTY stat
/// 兜底，避免未测平台上误杀所有 session。
fn is_session_alive(session: &Session) -> bool {
    let source = session.source.as_deref().unwrap_or("").to_ascii_lowercase();

    match source.as_str() {
        "claude-code" => probe_cli_agent_alive(session, "claude"),
        "codex" => probe_cli_agent_alive(session, "codex"),
        "cursor" => probe_cursor_alive(),
        // source 识别不出：直接判死（决策 4 的 C 方案）
        _ => false,
    }
}

/// 探测 CLI agent（claude-code / codex）的存活：要求
/// 1) session 有非空 terminal_tty
/// 2) TTY 设备文件存在
/// 3) 该 TTY 上能找到对应名字的进程
fn probe_cli_agent_alive(session: &Session, pname: &str) -> bool {
    let tty = match session.terminal_tty.as_deref() {
        Some(t) if !t.is_empty() => t,
        // 有 source 但缺 TTY 的 CLI agent 无法探活，直接判死
        _ => return false,
    };

    // TTY 设备不存在 → 终端已关闭，宿主必死
    if !std::path::Path::new(tty).exists() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS 的 pgrep -t 接受的是 tty 短名：ttys017（不带 /dev/ 前缀）
        let tty_short = tty.trim_start_matches("/dev/");
        match std::process::Command::new("pgrep")
            .arg("-t")
            .arg(tty_short)
            .arg(pname)
            .output()
        {
            Ok(output) => !output.stdout.is_empty(),
            // pgrep 调用本身失败（二进制不在、权限异常）先当作活着，
            // 不能因为探活工具出问题就误杀所有 session
            Err(_) => true,
        }
    }

    // 非 macOS 平台未测试 pgrep 语义，保守 fallback：
    // TTY 存在即视为活着（等同于 Task C 之前的行为）。
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pname;
        true
    }
}

/// 探测 Cursor app 是否在运行。粒度粗——整个 Cursor 进程存在就算活，
/// 无法区分具体 workspace 窗口。新语义下可接受。
fn probe_cursor_alive() -> bool {
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("pgrep")
            .arg("-x")
            .arg("Cursor")
            .output()
        {
            Ok(output) => !output.stdout.is_empty(),
            Err(_) => true,
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
