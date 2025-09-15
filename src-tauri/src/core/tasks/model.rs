use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TaskKind {
    GitClone { repo: String, dest: String },
    GitFetch { repo: String, dest: String },
    GitPush { dest: String, remote: Option<String>, refspecs: Option<Vec<String>>, username: Option<String>, password: Option<String> },
    HttpFake { url: String, method: String },
    Sleep { ms: u64 },
    Unknown,
}

impl TaskKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GitClone { .. } => "GitClone",
            Self::GitFetch { .. } => "GitFetch",
            Self::GitPush { .. } => "GitPush",
            Self::HttpFake { .. } => "HttpFake",
            Self::Sleep { .. } => "Sleep",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskMeta {
    pub id: Uuid,
    pub kind: TaskKind,
    pub state: TaskState,
    pub created_at: SystemTime,
    pub cancel_token: CancellationToken,
    pub fail_reason: Option<String>,
}

impl TaskMeta {
    pub fn created_at_ms(&self) -> u64 {
        self.created_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: Uuid,
    pub kind: String,
    pub state: TaskState,
    pub created_at: u64,
}

impl TaskSnapshot {
    pub fn from(m: &TaskMeta) -> Self {
        Self {
            id: m.id,
            kind: m.kind.as_str().to_string(),
            state: m.state.clone(),
            created_at: m.created_at_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStateEvent {
    pub task_id: Uuid,
    pub kind: String,
    pub state: TaskState,
    pub created_at: u64,
}

impl TaskStateEvent {
    pub fn new(m: &TaskMeta) -> Self {
        Self {
            task_id: m.id,
            kind: m.kind.as_str().to_string(),
            state: m.state.clone(),
            created_at: m.created_at_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressEvent {
    pub task_id: Uuid,
    pub kind: String,
    pub phase: String,
    pub percent: u32,
    // P0.6: Git 相关的可选指标
    pub objects: Option<u64>,
    pub bytes: Option<u64>,
    pub total_hint: Option<u64>,
    /// MP1.4: 可选的重试计数（仅在重试事件中出现）
    pub retried_times: Option<u32>,
}
