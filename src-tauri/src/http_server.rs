use crate::notifications::{Priority, Task, TaskStatus, TaskStore};
use crate::popup::{self, PopupList};
use crate::sound;
use crate::tray;
use axum::{
    extract::{Path, State},
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
    store: Arc<Mutex<TaskStore>>,
    popup_list: PopupList,
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

pub async fn start(app: AppHandle, store: Arc<Mutex<TaskStore>>, popup_list: PopupList) {
    let state = AppState {
        app,
        store,
        popup_list,
    };

    let router = Router::new()
        .route("/notify", post(handle_notify))
        .route("/notifications", get(handle_list))
        .route("/notifications/{id}/read", post(handle_mark_read))
        .route("/notifications/read-all", post(handle_mark_all_read))
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
        Some("running") => TaskStatus::Running,
        Some("success") => TaskStatus::Success,
        Some("failed") => TaskStatus::Failed,
        _ => TaskStatus::Pending,
    };

    let result = {
        let mut store = state.store.lock().unwrap();
        store.upsert_task(
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

    let _ = state.app.emit("notifications-updated", ());

    // Popup when status transitions TO a terminal state, or running → pending
    let should_popup = {
        let is_terminal_transition = result.task.status.is_terminal()
            && match &result.prev_status {
                Some(prev) => prev != &result.task.status,
                None => true, // new task created directly as success/failed
            };
        let is_pending_transition = result.task.status == TaskStatus::Pending
            && match &result.prev_status {
                Some(TaskStatus::Running) => true, // running → pending
                None => true,                      // new task created directly as pending
                _ => false,
            };
        is_terminal_transition || is_pending_transition
    };

    if should_popup {
        popup::show_popup(&state.app, &result.task, &state.popup_list);
        sound::play_alert();
    }

    let unread = state.store.lock().unwrap().unread_count();
    tray::update_tray_icon(&state.app, unread);

    let code = if result.is_new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    (code, Json(serde_json::to_value(&result.task).unwrap()))
}

async fn handle_list(State(state): State<AppState>) -> Json<Vec<Task>> {
    let store = state.store.lock().unwrap();
    Json(store.get_all().to_vec())
}

async fn handle_mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    let unread = {
        let mut store = state.store.lock().unwrap();
        if store.mark_read(&id) {
            let _ = state.app.emit("notifications-updated", ());
            store.unread_count()
        } else {
            return StatusCode::NOT_FOUND;
        }
    };
    tray::update_tray_icon(&state.app, unread);
    StatusCode::OK
}

async fn handle_mark_all_read(State(state): State<AppState>) -> StatusCode {
    {
        let mut store = state.store.lock().unwrap();
        store.mark_all_read();
        let _ = state.app.emit("notifications-updated", ());
    }
    tray::update_tray_icon(&state.app, 0);
    StatusCode::OK
}
