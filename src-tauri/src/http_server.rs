use crate::popup::{self, PopupList};
use crate::sessions::{Priority, Session, SessionStatus, SessionStore};
use crate::settings::SettingsStore;
use crate::sound;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
struct AppState {
    app: AppHandle,
    store: Arc<Mutex<SessionStore>>,
    popup_list: PopupList,
    settings_store: Arc<Mutex<SettingsStore>>,
}

#[derive(Deserialize)]
struct NotifyRequest {
    title: String,
    message: String,
    task_id: String,
    source: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    terminal_tty: Option<String>,
    #[serde(default)]
    workspace_path: Option<String>,
    // 配合 status=last_failed 使用；其他状态会被 upsert_session 忽略。
    #[serde(default)]
    failure_reason: Option<String>,
}

pub async fn start(
    app: AppHandle,
    store: Arc<Mutex<SessionStore>>,
    popup_list: PopupList,
    settings_store: Arc<Mutex<SettingsStore>>,
) {
    let state = AppState {
        app,
        store,
        popup_list,
        settings_store,
    };

    let router = Router::new()
        .route("/notify", post(handle_notify))
        .route("/sessions", get(handle_list))
        .with_state(state);

    let addr: SocketAddr = "127.0.0.1:9876".parse().unwrap();
    eprintln!("Poke Poke HTTP server listening on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind port 9876: {}. Trying 9877...", e);
            match tokio::net::TcpListener::bind("127.0.0.1:9877").await {
                Ok(l) => l,
                Err(e2) => {
                    eprintln!("Failed to bind fallback port: {}", e2);
                    return;
                }
            }
        }
    };

    let _ = axum::serve(listener, router).await;
}

async fn handle_notify(
    State(state): State<AppState>,
    Json(req): Json<NotifyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let priority = match req.priority.as_deref() {
        Some("high") => Priority::High,
        _ => Priority::Normal,
    };

    // 允许的 status 字符串（大小写不敏感）：
    //   running / pending / idle / last_failed
    // 兼容老字符串：success → idle，failure/failed → last_failed
    // 没有显式传 status 时默认 Pending。
    let status = match req.status.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "running" => SessionStatus::Running,
        Some(ref s) if s == "idle" || s == "success" => SessionStatus::Idle,
        Some(ref s) if s == "last_failed" || s == "failure" || s == "failed" => {
            SessionStatus::LastFailed
        }
        _ => SessionStatus::Pending,
    };

    let result = {
        let mut store = state.store.lock().unwrap();
        store.upsert_session(
            req.task_id,
            req.title,
            req.message,
            req.source,
            priority,
            status,
            req.terminal_tty,
            req.workspace_path,
            req.failure_reason,
        )
    };

    let _ = state.app.emit("sessions-updated", ());

    // popup 关闭：用户重新发起了一轮（状态切回 Running），把这条 session 上
    // 残留的 popup 关掉。触发条件是"上一轮 stage-ending 状态 → Running"：
    //   Idle / LastFailed / Pending → Running
    // 语义：上一轮结束后弹的 popup 已经达成提醒使命，新一轮开始时顺手收掉。
    let should_close_popup = !result.is_new
        && result.session.status == SessionStatus::Running
        && result.prev_status.as_ref().is_some_and(|prev| {
            matches!(
                prev,
                SessionStatus::Idle | SessionStatus::LastFailed | SessionStatus::Pending
            )
        });

    if should_close_popup {
        popup::close_popup(&state.app, &result.session.id, &state.popup_list);
    }

    // popup 触发：进入"阶段结束"状态（Idle / LastFailed）或 Pending 时弹。
    // 注意 Idle / LastFailed 在 Task C 后不再是"终态"，但它们仍是
    // "一轮 agent 工作的结束点"，该提醒用户的场景没变。
    let should_popup = {
        let is_stage_end_transition = matches!(
            result.session.status,
            SessionStatus::Idle | SessionStatus::LastFailed
        ) && match &result.prev_status {
            Some(prev) => prev != &result.session.status,
            None => true,
        };
        let is_pending_transition = result.session.status == SessionStatus::Pending
            && match &result.prev_status {
                Some(SessionStatus::Running) => true,
                None => true,
                _ => false,
            };
        is_stage_end_transition || is_pending_transition
    };

    if should_popup {
        let already_focused = result
            .session
            .terminal_tty
            .as_deref()
            .is_some_and(|tty| !tty.is_empty() && popup::is_terminal_session_focused(tty));

        if !already_focused {
            popup::show_popup(&state.app, &result.session, &state.popup_list);
            sound::play_alert_with_settings(&state.settings_store);
        }
    }

    let code = if result.is_new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    (code, Json(serde_json::to_value(&result.session).unwrap()))
}

async fn handle_list(State(state): State<AppState>) -> Json<Vec<Session>> {
    let store = state.store.lock().unwrap();
    Json(store.get_all().to_vec())
}
