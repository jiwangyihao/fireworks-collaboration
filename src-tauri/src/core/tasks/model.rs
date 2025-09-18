use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::core::git::errors::ErrorCategory;

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
    GitInit { dest: String },
    GitAdd { dest: String, paths: Vec<String> },
    GitCommit { dest: String, message: String, allow_empty: bool, author_name: Option<String>, author_email: Option<String> },
    GitBranch { dest: String, name: String, checkout: bool, force: bool },
    GitCheckout { dest: String, reference: String, create: bool },
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
            Self::GitInit { .. } => "GitInit",
            Self::GitAdd { .. } => "GitAdd",
            Self::GitCommit { .. } => "GitCommit",
            Self::GitBranch { .. } => "GitBranch",
            Self::GitCheckout { .. } => "GitCheckout",
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

/// MP1.5: 标准化错误事件负载
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskErrorEvent {
    pub task_id: Uuid,
    pub kind: String,
    /// 统一字符串分类：Network|Tls|Verify|Protocol|Proxy|Auth|Cancel|Internal
    pub category: String,
    /// 可选的错误代码（预留，当前为空）
    pub code: Option<String>,
    pub message: String,
    /// 已重试次数（若有重试逻辑）
    pub retried_times: Option<u32>,
}

impl TaskErrorEvent {
    pub fn from_parts(task_id: Uuid, kind: &str, category: ErrorCategory, message: impl Into<String>, retried_times: Option<u32>) -> Self {
        Self {
            task_id,
            kind: kind.to_string(),
            category: match category {
                ErrorCategory::Network => "Network".into(),
                ErrorCategory::Tls => "Tls".into(),
                ErrorCategory::Verify => "Verify".into(),
                ErrorCategory::Protocol => "Protocol".into(),
                ErrorCategory::Proxy => "Proxy".into(),
                ErrorCategory::Auth => "Auth".into(),
                ErrorCategory::Cancel => "Cancel".into(),
                ErrorCategory::Internal => "Internal".into(),
            },
            code: None,
            message: message.into(),
            retried_times,
        }
    }
}
