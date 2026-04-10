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
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Success | TaskStatus::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub status: TaskStatus,
    pub read: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct TaskStore {
    pub tasks: Vec<Task>,
    file_path: PathBuf,
}

/// Return value from upsert: the task, whether it's new, and the previous status if updated
pub struct UpsertResult {
    pub task: Task,
    pub is_new: bool,
    pub prev_status: Option<TaskStatus>,
}

impl TaskStore {
    pub fn load(file_path: PathBuf) -> Self {
        let tasks = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self { tasks, file_path }
    }

    fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            &self.file_path,
            serde_json::to_string_pretty(&self.tasks).unwrap_or_default(),
        );
    }

    pub fn upsert_task(
        &mut self,
        task_id: String,
        title: String,
        message: String,
        source: Option<String>,
        priority: Priority,
        status: TaskStatus,
    ) -> UpsertResult {
        // Try to find existing task by task_id
        if let Some(existing) = self.tasks.iter_mut().find(|t| t.task_id == task_id) {
            let prev_status = existing.status.clone();
            existing.title = title;
            existing.message = message;
            if source.is_some() {
                existing.source = source;
            }
            existing.priority = priority;
            existing.status = status;
            existing.updated_at = Utc::now();
            // Mark unread when transitioning to terminal state
            if existing.status.is_terminal() && !prev_status.is_terminal() {
                existing.read = false;
            }
            let task = existing.clone();
            self.save();
            UpsertResult {
                task,
                is_new: false,
                prev_status: Some(prev_status),
            }
        } else {
            let now = Utc::now();
            let task = Task {
                id: uuid::Uuid::new_v4().to_string(),
                task_id,
                title,
                message,
                source,
                priority,
                status,
                read: false,
                created_at: now,
                updated_at: now,
            };
            self.tasks.insert(0, task.clone());
            self.save();
            UpsertResult {
                task,
                is_new: true,
                prev_status: None,
            }
        }
    }

    pub fn mark_read(&mut self, id: &str) -> bool {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.read = true;
            self.save();
            true
        } else {
            false
        }
    }

    pub fn mark_all_read(&mut self) {
        for t in &mut self.tasks {
            if t.status.is_terminal() {
                t.read = true;
            }
        }
        self.save();
    }

    pub fn unread_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| !t.read && t.status.is_terminal())
            .count()
    }

    pub fn get_all(&self) -> &[Task] {
        &self.tasks
    }

    pub fn remove_task(&mut self, id: &str) -> bool {
        let len = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        if self.tasks.len() < len {
            self.save();
            true
        } else {
            false
        }
    }
}
