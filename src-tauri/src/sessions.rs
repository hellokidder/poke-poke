use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Pending,
    Running,
    Success,
}

impl SessionStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionStatus::Success)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub terminal_tty: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

pub struct SessionStore {
    pub sessions: Vec<Session>,
    file_path: PathBuf,
}

/// Return value from upsert: the session, whether it's new, and the previous status if updated
pub struct UpsertResult {
    pub session: Session,
    pub is_new: bool,
    pub prev_status: Option<SessionStatus>,
}

impl SessionStore {
    pub fn load(file_path: PathBuf) -> Self {
        let sessions = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            sessions,
            file_path,
        }
    }

    fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            &self.file_path,
            serde_json::to_string_pretty(&self.sessions).unwrap_or_default(),
        );
    }

    pub fn upsert_session(
        &mut self,
        task_id: String,
        title: String,
        message: String,
        source: Option<String>,
        priority: Priority,
        status: SessionStatus,
        terminal_tty: Option<String>,
        workspace_path: Option<String>,
    ) -> UpsertResult {
        // Try to find existing session by task_id
        if let Some(existing) = self.sessions.iter_mut().find(|s| s.task_id == task_id) {
            let prev_status = existing.status.clone();
            existing.title = title;
            existing.message = message;
            if source.is_some() {
                existing.source = source;
            }
            existing.priority = priority;
            existing.status = status;
            existing.updated_at = Utc::now();
            if terminal_tty.is_some() {
                existing.terminal_tty = terminal_tty;
            }
            if workspace_path.is_some() {
                existing.workspace_path = workspace_path;
            }
            let session = existing.clone();
            self.save();
            UpsertResult {
                session,
                is_new: false,
                prev_status: Some(prev_status),
            }
        } else {
            let now = Utc::now();
            let session = Session {
                id: uuid::Uuid::new_v4().to_string(),
                task_id,
                title,
                message,
                source,
                priority,
                status,
                created_at: now,
                updated_at: now,
                terminal_tty,
                workspace_path,
            };
            self.sessions.insert(0, session.clone());
            self.save();
            UpsertResult {
                session,
                is_new: true,
                prev_status: None,
            }
        }
    }

    pub fn get_all(&self) -> &[Session] {
        &self.sessions
    }

    pub fn remove_session(&mut self, id: &str) -> bool {
        let len = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        if self.sessions.len() < len {
            self.save();
            true
        } else {
            false
        }
    }

    /// Remove terminal-state sessions older than `retention_hours`.
    /// Running/Pending sessions are never removed.
    /// Returns the number of sessions removed.
    pub fn cleanup_expired(&mut self, retention_hours: u32) -> usize {
        let cutoff = Utc::now() - chrono::Duration::hours(retention_hours as i64);
        let before = self.sessions.len();
        self.sessions.retain(|s| {
            if !s.status.is_terminal() {
                return true;
            }
            s.updated_at > cutoff
        });
        let removed = before - self.sessions.len();
        if removed > 0 {
            self.save();
        }
        removed
    }
}
