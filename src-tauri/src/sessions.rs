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

// Session 状态机（Task C 重构后）：
// 这四种都是"agent 活着"时的一种状态，没有"终态"概念。
// session 的生命周期唯一由宿主进程存活决定，见 lib.rs 的探活线程。
//
// - Running    : agent 正在处理某一轮（UserPromptSubmit / SessionStart）
// - Pending    : agent 停下等用户操作（CC Notification 权限询问等）
// - Idle       : 上一轮正常结束，agent 空闲等下一轮（Stop hook）
// - LastFailed : 上一轮因 API 错误结束，agent 仍活着（StopFailure hook）
//
// serde alias 保持对老 sessions.json 的向下兼容：
// - "success" -> Idle
// - "failure" -> LastFailed
// 新版本写入时使用新名字，老版本读新文件会反序列化失败（单向升级）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
// 用 snake_case 是为了让 `LastFailed` 正确序列化成 "last_failed"。
// 原来用 lowercase 会合并成 "lastfailed"，和前端 types.ts / i18n key 的
// "last_failed" 对不上，导致 StatusDot 的 Record 索引为 undefined，
// 整个 panel 白屏。Running/Pending/Idle 都是单个单词，snake_case 与
// lowercase 行为一致，没有兼容成本。
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[default]
    Pending,
    Running,
    #[serde(alias = "success")]
    Idle,
    #[serde(alias = "failure")]
    LastFailed,
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
    // LastFailed 状态下的 reason code（如 CC StopFailure 的 matcher 值）。
    // 只在 status=LastFailed 时有意义；前端负责根据 reason 做本地化展示。
    // 老 sessions.json 里没有这个字段，serde_default 保证兼容。
    #[serde(default)]
    pub failure_reason: Option<String>,
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
        failure_reason: Option<String>,
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
            existing.status = status.clone();
            existing.updated_at = Utc::now();
            if terminal_tty.is_some() {
                existing.terminal_tty = terminal_tty;
            }
            if workspace_path.is_some() {
                existing.workspace_path = workspace_path;
            }
            // failure_reason：只在进入 LastFailed 时才覆盖；其他状态下清空，避免
            // 一条 session 从 LastFailed 回到 Running/Idle 后仍显示旧的失败原因。
            existing.failure_reason = if status == SessionStatus::LastFailed {
                failure_reason
            } else {
                None
            };
            let session = existing.clone();
            self.save();
            UpsertResult {
                session,
                is_new: false,
                prev_status: Some(prev_status),
            }
        } else {
            let now = Utc::now();
            // 同理：非 LastFailed 新 session 不该携带 failure_reason
            let failure_reason = if status == SessionStatus::LastFailed {
                failure_reason
            } else {
                None
            };
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
                failure_reason,
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
}
