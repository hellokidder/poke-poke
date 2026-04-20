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
    #[serde(default)]
    external_session_id: Option<String>,
    source: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    event_type: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum NotifyEventType {
    Running,
    Pending,
    Stop,
    SessionEnd,
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

fn parse_event_type(event_type: Option<&str>) -> Option<NotifyEventType> {
    match event_type.map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "running" => Some(NotifyEventType::Running),
        Some(ref s) if s == "pending" => Some(NotifyEventType::Pending),
        Some(ref s) if s == "stop" => Some(NotifyEventType::Stop),
        Some(ref s) if s == "session_end" => Some(NotifyEventType::SessionEnd),
        _ => None,
    }
}

fn parse_status(status: Option<&str>, event_type: Option<&NotifyEventType>) -> SessionStatus {
    match status.map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "running" => SessionStatus::Running,
        Some(ref s) if s == "idle" || s == "success" => SessionStatus::Idle,
        Some(ref s) if s == "last_failed" || s == "failure" || s == "failed" => {
            SessionStatus::LastFailed
        }
        None => match event_type {
            Some(NotifyEventType::Running) => SessionStatus::Running,
            Some(NotifyEventType::Pending) => SessionStatus::Pending,
            Some(NotifyEventType::Stop) => SessionStatus::Idle,
            _ => SessionStatus::Pending,
        },
        _ => SessionStatus::Pending,
    }
}

fn should_close_popup(
    is_new: bool,
    status: &SessionStatus,
    prev: Option<&SessionStatus>,
) -> bool {
    !is_new
        && *status == SessionStatus::Running
        && prev.is_some_and(|prev| {
            matches!(
                prev,
                SessionStatus::Idle | SessionStatus::LastFailed | SessionStatus::Pending
            )
        })
}

fn should_show_popup(
    status: &SessionStatus,
    prev: Option<&SessionStatus>,
    is_new: bool,
) -> bool {
    let is_stage_end_transition = matches!(*status, SessionStatus::Idle | SessionStatus::LastFailed)
        && (is_new || prev.map(|prev| prev != status).unwrap_or(true));
    let is_pending_transition =
        *status == SessionStatus::Pending && (is_new || prev == Some(&SessionStatus::Running));

    is_stage_end_transition || is_pending_transition
}

async fn handle_notify(
    State(state): State<AppState>,
    Json(req): Json<NotifyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let priority = match req.priority.as_deref() {
        Some("high") => Priority::High,
        _ => Priority::Normal,
    };
    let event_type = parse_event_type(req.event_type.as_deref());

    if event_type == Some(NotifyEventType::SessionEnd) {
        let removed_id = {
            let mut store = state.store.lock().unwrap();
            store.remove_session_by_task_id(&req.task_id).map(|s| s.id)
        };

        if let Some(session_id) = removed_id.as_ref() {
            popup::close_popup(&state.app, session_id, &state.popup_list);
            let _ = state.app.emit("sessions-updated", ());
        }

        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "removed": removed_id.is_some(),
                "task_id": req.task_id,
            })),
        );
    }

    // 允许的 status 字符串（大小写不敏感）：
    //   running / pending / idle / last_failed
    // 兼容老字符串：success → idle，failure/failed → last_failed
    // 若缺失 status，则按 event_type 推导；两者都缺失时默认 Pending。
    let status = parse_status(req.status.as_deref(), event_type.as_ref());

    let result = {
        let mut store = state.store.lock().unwrap();
        store.upsert_session(
            req.task_id,
            req.external_session_id,
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
    let close_popup_now = should_close_popup(
        result.is_new,
        &result.session.status,
        result.prev_status.as_ref(),
    );

    if close_popup_now {
        popup::close_popup(&state.app, &result.session.id, &state.popup_list);
    }

    // popup 触发：进入"阶段结束"状态（Idle / LastFailed）或 Pending 时弹。
    // 注意 Idle / LastFailed 在 Task C 后不再是"终态"，但它们仍是
    // "一轮 agent 工作的结束点"，该提醒用户的场景没变。
    let show_popup_now = should_show_popup(
        &result.session.status,
        result.prev_status.as_ref(),
        result.is_new,
    );

    if show_popup_now {
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

#[cfg(test)]
mod tests {
    use super::{parse_event_type, parse_status, should_close_popup, should_show_popup, NotifyEventType};
    use crate::sessions::SessionStatus;

    #[test]
    fn parse_status_supports_case_insensitive_values_and_aliases() {
        assert_eq!(parse_status(Some("RUNNING"), None), SessionStatus::Running);
        assert_eq!(parse_status(Some("success"), None), SessionStatus::Idle);
        assert_eq!(parse_status(Some("FAILED"), None), SessionStatus::LastFailed);
        assert_eq!(parse_status(Some("last_failed"), None), SessionStatus::LastFailed);
    }

    #[test]
    fn parse_status_defaults_to_pending_for_missing_or_unknown_values() {
        assert_eq!(parse_status(None, None), SessionStatus::Pending);
        assert_eq!(parse_status(Some("unknown"), None), SessionStatus::Pending);
    }

    #[test]
    fn parse_event_type_supports_known_values_and_unknown_fallback() {
        assert_eq!(parse_event_type(Some("running")), Some(NotifyEventType::Running));
        assert_eq!(parse_event_type(Some("PENDING")), Some(NotifyEventType::Pending));
        assert_eq!(parse_event_type(Some("stop")), Some(NotifyEventType::Stop));
        assert_eq!(
            parse_event_type(Some("session_end")),
            Some(NotifyEventType::SessionEnd),
        );
        assert_eq!(parse_event_type(Some("other")), None);
    }

    #[test]
    fn parse_status_can_fall_back_to_event_type_when_status_missing() {
        assert_eq!(
            parse_status(None, Some(&NotifyEventType::Running)),
            SessionStatus::Running,
        );
        assert_eq!(
            parse_status(None, Some(&NotifyEventType::Pending)),
            SessionStatus::Pending,
        );
        assert_eq!(
            parse_status(None, Some(&NotifyEventType::Stop)),
            SessionStatus::Idle,
        );
    }

    #[test]
    fn explicit_status_has_priority_over_event_type_fallback() {
        assert_eq!(
            parse_status(Some("last_failed"), Some(&NotifyEventType::Stop)),
            SessionStatus::LastFailed,
        );
    }

    #[test]
    fn should_close_popup_returns_false_for_new_sessions() {
        assert!(!should_close_popup(true, &SessionStatus::Running, None));
    }

    #[test]
    fn should_close_popup_closes_idle_to_running_transition() {
        assert!(should_close_popup(
            false,
            &SessionStatus::Running,
            Some(&SessionStatus::Idle),
        ));
    }

    #[test]
    fn should_close_popup_closes_last_failed_to_running_transition() {
        assert!(should_close_popup(
            false,
            &SessionStatus::Running,
            Some(&SessionStatus::LastFailed),
        ));
    }

    #[test]
    fn should_close_popup_closes_pending_to_running_transition() {
        assert!(should_close_popup(
            false,
            &SessionStatus::Running,
            Some(&SessionStatus::Pending),
        ));
    }

    #[test]
    fn should_close_popup_keeps_popup_for_running_to_running_transition() {
        assert!(!should_close_popup(
            false,
            &SessionStatus::Running,
            Some(&SessionStatus::Running),
        ));
    }

    #[test]
    fn should_show_popup_for_running_to_idle_transition() {
        assert!(should_show_popup(
            &SessionStatus::Idle,
            Some(&SessionStatus::Running),
            false,
        ));
    }

    #[test]
    fn should_show_popup_for_running_to_last_failed_transition() {
        assert!(should_show_popup(
            &SessionStatus::LastFailed,
            Some(&SessionStatus::Running),
            false,
        ));
    }

    #[test]
    fn should_not_show_popup_for_idle_to_idle_transition() {
        assert!(!should_show_popup(
            &SessionStatus::Idle,
            Some(&SessionStatus::Idle),
            false,
        ));
    }

    #[test]
    fn should_not_show_popup_for_last_failed_to_last_failed_transition() {
        assert!(!should_show_popup(
            &SessionStatus::LastFailed,
            Some(&SessionStatus::LastFailed),
            false,
        ));
    }

    #[test]
    fn should_show_popup_for_running_to_pending_transition() {
        assert!(should_show_popup(
            &SessionStatus::Pending,
            Some(&SessionStatus::Running),
            false,
        ));
    }

    #[test]
    fn should_not_show_popup_for_pending_to_running_transition() {
        assert!(!should_show_popup(
            &SessionStatus::Running,
            Some(&SessionStatus::Pending),
            false,
        ));
    }

    #[test]
    fn should_show_popup_for_new_pending_session() {
        assert!(should_show_popup(&SessionStatus::Pending, None, true));
    }

    #[test]
    fn should_show_popup_for_new_idle_or_last_failed_session() {
        assert!(should_show_popup(&SessionStatus::Idle, None, true));
        assert!(should_show_popup(&SessionStatus::LastFailed, None, true));
    }
}
