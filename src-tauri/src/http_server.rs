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

    let status = match req.status.as_deref() {
        Some("running") => SessionStatus::Running,
        Some("success") | Some("failed") => SessionStatus::Success,
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
        )
    };

    let _ = state.app.emit("sessions-updated", ());

    // Session-based popup dismiss: when a session resumes (user interacted),
    // close the existing popup for that session.
    // pending → running = user approved permission prompt
    // terminal → running = user started new prompt after completion
    let should_close_popup = !result.is_new
        && result.session.status == SessionStatus::Running
        && result
            .prev_status
            .as_ref()
            .is_some_and(|prev| prev.is_terminal() || *prev == SessionStatus::Pending);

    if should_close_popup {
        popup::close_popup(&state.app, &result.session.id, &state.popup_list);
    }

    // Popup when status transitions TO a terminal state, or running → pending
    let should_popup = {
        let is_terminal_transition = result.session.status.is_terminal()
            && match &result.prev_status {
                Some(prev) => prev != &result.session.status,
                None => true,
            };
        let is_pending_transition = result.session.status == SessionStatus::Pending
            && match &result.prev_status {
                Some(SessionStatus::Running) => true,
                None => true,
                _ => false,
            };
        is_terminal_transition || is_pending_transition
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
