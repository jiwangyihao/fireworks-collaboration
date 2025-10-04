use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::git::errors::ErrorCategory;
use crate::core::tasks::model::{
    TaskErrorEvent, TaskKind, TaskProgressEvent, TaskState, WorkspaceBatchOperation,
};
use crate::events::emitter::{emit_all, AppHandle};

use super::registry::{TaskRegistry, EV_PROGRESS};

#[derive(Clone)]
pub struct CloneOptions {
    pub repo_url: String,
    pub dest: String,
    pub depth_u32: Option<u32>,
    pub depth_value: Option<serde_json::Value>,
    pub filter: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
    pub recurse_submodules: bool,
}

#[derive(Clone)]
pub struct FetchOptions {
    pub repo_url: String,
    pub dest: String,
    pub preset: Option<String>,
    pub depth_u32: Option<u32>,
    pub depth_value: Option<serde_json::Value>,
    pub filter: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct PushOptions {
    pub dest: String,
    pub remote: Option<String>,
    pub refspecs: Option<Vec<String>>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
}

#[derive(Clone)]
pub enum WorkspaceBatchChildOperation {
    Clone(CloneOptions),
    Fetch(FetchOptions),
    Push(PushOptions),
    #[cfg(test)]
    Sleep(u64),
}

#[derive(Clone)]
pub struct WorkspaceBatchChildSpec {
    pub repo_id: String,
    pub repo_name: String,
    pub operation: WorkspaceBatchChildOperation,
}

#[derive(Clone, Debug)]
pub struct BatchFailure {
    pub repo_id: String,
    pub repo_name: String,
    pub message: String,
}

struct BatchProgressState {
    total: usize,
    percents: HashMap<Uuid, u32>,
    completed: usize,
    success: usize,
    failure: usize,
    finished: HashSet<Uuid>,
}

#[derive(Clone)]
struct BatchSnapshot {
    percent: u32,
    completed: usize,
    success: usize,
    failure: usize,
    total: usize,
}

impl BatchProgressState {
    fn new(total: usize) -> Self {
        Self {
            total,
            percents: HashMap::new(),
            completed: 0,
            success: 0,
            failure: 0,
            finished: HashSet::new(),
        }
    }

    fn register_child(&mut self, id: Uuid) {
        self.percents.entry(id).or_insert(0);
    }

    fn update_percent(&mut self, id: Uuid, percent: u32) -> BatchSnapshot {
        self.percents
            .entry(id)
            .and_modify(|v| *v = (*v).max(percent.min(100)))
            .or_insert(percent.min(100));
        self.snapshot()
    }

    fn mark_finished(&mut self, id: Uuid, success: bool) -> BatchSnapshot {
        self.percents.insert(id, 100);
        if self.finished.insert(id) {
            self.completed += 1;
            if success {
                self.success += 1;
            } else {
                self.failure += 1;
            }
        }
        self.snapshot()
    }

    fn snapshot(&self) -> BatchSnapshot {
        let total = self.total.max(1);
        let sum: u32 = self.percents.values().copied().sum();
        let percent = (sum / total as u32).min(100);
        BatchSnapshot {
            percent,
            completed: self.completed,
            success: self.success,
            failure: self.failure,
            total: self.total,
        }
    }
}

impl WorkspaceBatchOperation {
    fn progress_action(&self) -> &'static str {
        match self {
            WorkspaceBatchOperation::Clone => "Cloning",
            WorkspaceBatchOperation::Fetch => "Fetching",
            WorkspaceBatchOperation::Push => "Pushing",
        }
    }

    fn summary_label(&self) -> &'static str {
        match self {
            WorkspaceBatchOperation::Clone => "batch clone",
            WorkspaceBatchOperation::Fetch => "batch fetch",
            WorkspaceBatchOperation::Push => "batch push",
        }
    }
}

impl TaskRegistry {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_workspace_batch_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        parent_id: Uuid,
        parent_token: CancellationToken,
        operation: WorkspaceBatchOperation,
        specs: Vec<WorkspaceBatchChildSpec>,
        max_concurrency: usize,
    ) -> tokio::task::JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let total = specs.len();
            tracing::info!(
                target = "workspace_batch",
                operation = ?operation,
                total,
                parent = %parent_id,
                "Starting workspace batch task"
            );

            this.mark_running(&app, &parent_id, "WorkspaceBatch");

            if total == 0 {
                emit_parent_progress(
                    &app,
                    parent_id,
                    &operation,
                    &BatchSnapshot {
                        percent: 100,
                        completed: 0,
                        success: 0,
                        failure: 0,
                        total: 0,
                    },
                    Some("No repositories to process".to_string()),
                );
                this.mark_completed(&app, &parent_id);
                return;
            }

            let concurrency = max_concurrency.max(1).min(total.max(1));
            let semaphore = Arc::new(Semaphore::new(concurrency));
            let progress_state = Arc::new(Mutex::new(BatchProgressState::new(total)));
            let failures = Arc::new(Mutex::new(Vec::<BatchFailure>::new()));

            emit_parent_progress(
                &app,
                parent_id,
                &operation,
                &BatchSnapshot {
                    percent: 0,
                    completed: 0,
                    success: 0,
                    failure: 0,
                    total,
                },
                None,
            );

            let mut join_set = JoinSet::new();

            for spec in specs {
                if parent_token.is_cancelled() {
                    tracing::warn!(
                        target = "workspace_batch",
                        parent = %parent_id,
                        "Parent task cancelled before scheduling all children"
                    );
                    break;
                }

                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(
                            target = "workspace_batch",
                            error = %e,
                            "Semaphore acquisition failed"
                        );
                        break;
                    }
                };

                let registry_clone = Arc::clone(&this);
                let app_clone = app.clone();
                let progress_clone = Arc::clone(&progress_state);
                let failures_clone = Arc::clone(&failures);
                let parent_token_clone = parent_token.clone();
                let operation_clone = operation.clone();
                let spec_clone = spec.clone();
                let parent_id_clone = parent_id;

                join_set.spawn(async move {
                    let _permit = permit;
                    let (child_id, child_token, handle) = {
                        let registry_inner = Arc::clone(&registry_clone);
                        match spec_clone.operation.clone() {
                            WorkspaceBatchChildOperation::Clone(opts) => {
                                let (child_id, token) = registry_inner.create(TaskKind::GitClone {
                                    repo: opts.repo_url.clone(),
                                    dest: opts.dest.clone(),
                                    depth: opts.depth_u32,
                                    filter: opts.filter.clone(),
                                    strategy_override: opts.strategy_override.clone(),
                                    recurse_submodules: opts.recurse_submodules,
                                });

                                {
                                    let mut guard = progress_clone.lock().unwrap();
                                    guard.register_child(child_id);
                                }

                                let hook_progress = create_progress_hook(
                                    Arc::clone(&progress_clone),
                                    app_clone.clone(),
                                    parent_id_clone,
                                    operation_clone.clone(),
                                    child_id,
                                );

                                registry_inner.link_parent_child(parent_id_clone, child_id);
                                let handle = registry_inner.spawn_git_clone_task_with_opts(
                                    None,
                                    child_id,
                                    token.clone(),
                                    opts.repo_url,
                                    opts.dest,
                                    opts.depth_value,
                                    opts.filter,
                                    opts.strategy_override,
                                    opts.recurse_submodules,
                                    Some(hook_progress),
                                );
                                (child_id, token, handle)
                            }
                            WorkspaceBatchChildOperation::Fetch(opts) => {
                                let (child_id, token) = registry_inner.create(TaskKind::GitFetch {
                                    repo: opts.repo_url.clone(),
                                    dest: opts.dest.clone(),
                                    depth: opts.depth_u32,
                                    filter: opts.filter.clone(),
                                    strategy_override: opts.strategy_override.clone(),
                                });
                                {
                                    let mut guard = progress_clone.lock().unwrap();
                                    guard.register_child(child_id);
                                }
                                let hook_progress = create_progress_hook(
                                    Arc::clone(&progress_clone),
                                    app_clone.clone(),
                                    parent_id_clone,
                                    operation_clone.clone(),
                                    child_id,
                                );
                                registry_inner.link_parent_child(parent_id_clone, child_id);
                                let handle = registry_inner.spawn_git_fetch_task_with_opts(
                                    None,
                                    child_id,
                                    token.clone(),
                                    opts.repo_url,
                                    opts.dest,
                                    opts.preset,
                                    opts.depth_value,
                                    opts.filter,
                                    opts.strategy_override,
                                    Some(hook_progress),
                                );
                                (child_id, token, handle)
                            }
                            WorkspaceBatchChildOperation::Push(opts) => {
                                let (child_id, token) = registry_inner.create(TaskKind::GitPush {
                                    dest: opts.dest.clone(),
                                    remote: opts.remote.clone(),
                                    refspecs: opts.refspecs.clone(),
                                    username: opts.username.clone(),
                                    password: opts.password.clone(),
                                    strategy_override: opts.strategy_override.clone(),
                                });
                                {
                                    let mut guard = progress_clone.lock().unwrap();
                                    guard.register_child(child_id);
                                }
                                let hook_progress = create_progress_hook(
                                    Arc::clone(&progress_clone),
                                    app_clone.clone(),
                                    parent_id_clone,
                                    operation_clone.clone(),
                                    child_id,
                                );
                                registry_inner.link_parent_child(parent_id_clone, child_id);
                                let handle = registry_inner.spawn_git_push_task(
                                    None,
                                    child_id,
                                    token.clone(),
                                    opts.dest,
                                    opts.remote,
                                    opts.refspecs,
                                    opts.username,
                                    opts.password,
                                    opts.strategy_override,
                                    Some(hook_progress),
                                );
                                (child_id, token, handle)
                            }
                            #[cfg(test)]
                            WorkspaceBatchChildOperation::Sleep(ms) => {
                                let (child_id, token) = registry_inner.create(TaskKind::Sleep { ms });
                                {
                                    let mut guard = progress_clone.lock().unwrap();
                                    guard.register_child(child_id);
                                }
                                registry_inner.link_parent_child(parent_id_clone, child_id);
                                let handle = registry_inner.spawn_sleep_task(None, child_id, token.clone(), ms);
                                (child_id, token, handle)
                            }
                        }
                    };

                    // propagate cancel
                    let parent_cancel = parent_token_clone.clone();
                    let child_cancel = child_token.clone();
                    tokio::spawn(async move {
                        parent_cancel.cancelled().await;
                        child_cancel.cancel();
                    });

                    if let Err(join_err) = handle.await {
                        tracing::error!(
                            target = "workspace_batch",
                            parent = %parent_id_clone,
                            repo = %spec_clone.repo_id,
                            error = %join_err,
                            "Child task join failed"
                        );
                        registry_clone.mark_failed(&None, &child_id, "Child task join failure");
                        registry_clone
                            .with_meta(&child_id, |m| m.fail_reason = Some(join_err.to_string()));
                        let snapshot = {
                            let mut guard = progress_clone.lock().unwrap();
                            guard.mark_finished(child_id, false)
                        };
                        failures_clone.lock().unwrap().push(BatchFailure {
                            repo_id: spec_clone.repo_id.clone(),
                            repo_name: spec_clone.repo_name.clone(),
                            message: join_err.to_string(),
                        });
                        emit_parent_progress(&app_clone, parent_id_clone, &operation_clone, &snapshot, None);
                        return;
                    }

                    let final_state = wait_for_terminal_state(Arc::clone(&registry_clone), child_id).await;
                    let success = matches!(final_state, TaskState::Completed);
                    let snapshot = {
                        let mut guard = progress_clone.lock().unwrap();
                        guard.mark_finished(child_id, success)
                    };

                    if success {
                        emit_parent_progress(
                            &app_clone,
                            parent_id_clone,
                            &operation_clone,
                            &snapshot,
                            None,
                        );
                    } else {
                        let failure_reason = registry_clone
                            .fail_reason(&child_id)
                            .unwrap_or_else(|| "Unknown failure".to_string());
                        failures_clone.lock().unwrap().push(BatchFailure {
                            repo_id: spec_clone.repo_id.clone(),
                            repo_name: spec_clone.repo_name.clone(),
                            message: failure_reason.clone(),
                        });
                        emit_parent_progress(
                            &app_clone,
                            parent_id_clone,
                            &operation_clone,
                            &snapshot,
                            None,
                        );
                    }
                });
            }

            while let Some(res) = join_set.join_next().await {
                if let Err(e) = res {
                    tracing::error!(
                        target = "workspace_batch",
                        parent = %parent_id,
                        error = %e,
                        "Batch child task join failure"
                    );
                }
            }

            if parent_token.is_cancelled() {
                tracing::info!(
                    target = "workspace_batch",
                    parent = %parent_id,
                    "Parent task cancelled"
                );
                this.mark_canceled(&app, &parent_id);
                return;
            }

            let failure_list = failures.lock().unwrap();
            if failure_list.is_empty() {
                tracing::info!(
                    target = "workspace_batch",
                    parent = %parent_id,
                    "Workspace batch task completed successfully"
                );
                this.mark_completed(&app, &parent_id);
            } else {
                let summary = summarize_failures(&failure_list, operation.summary_label());
                this.with_meta(&parent_id, |m| m.fail_reason = Some(summary.clone()));
                if let Some(app_ref) = &app {
                    let err_evt = TaskErrorEvent::from_parts(
                        parent_id,
                        "WorkspaceBatch",
                        ErrorCategory::Internal,
                        summary.clone(),
                        None,
                    );
                    this.emit_error(app_ref, &err_evt);
                }
                this.mark_failed(&app, &parent_id, &summary);
            }
        })
    }
}

fn create_progress_hook(
    progress_state: Arc<Mutex<BatchProgressState>>,
    app: Option<AppHandle>,
    parent_id: Uuid,
    operation: WorkspaceBatchOperation,
    child_id: Uuid,
) -> Arc<dyn Fn(TaskProgressEvent) + Send + Sync> {
    Arc::new(move |evt: TaskProgressEvent| {
        let snapshot = {
            let mut guard = progress_state.lock().unwrap();
            guard.update_percent(child_id, evt.percent)
        };
        emit_parent_progress(&app, parent_id, &operation, &snapshot, None);
    })
}

fn emit_parent_progress(
    app: &Option<AppHandle>,
    parent_id: Uuid,
    operation: &WorkspaceBatchOperation,
    snapshot: &BatchSnapshot,
    message_override: Option<String>,
) {
    if let Some(app_ref) = app {
        let phase = if let Some(msg) = message_override {
            msg
        } else {
            format!(
                "{} {}/{} completed ({} failed)",
                operation.progress_action(),
                snapshot.completed,
                snapshot.total,
                snapshot.failure
            )
        };
        let event = TaskProgressEvent {
            task_id: parent_id,
            kind: "WorkspaceBatch".into(),
            phase,
            percent: snapshot.percent,
            objects: Some(snapshot.completed as u64),
            bytes: None,
            total_hint: Some(snapshot.total as u64),
            retried_times: None,
        };
        emit_all(app_ref, EV_PROGRESS, &event);
    }
}

async fn wait_for_terminal_state(registry: Arc<TaskRegistry>, id: Uuid) -> TaskState {
    loop {
        if let Some(snapshot) = registry.snapshot(&id) {
            match snapshot.state {
                TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                    return snapshot.state
                }
                _ => {}
            }
        }
        sleep(Duration::from_millis(50)).await;
    }
}

fn summarize_failures(failures: &[BatchFailure], label: &str) -> String {
    let count = failures.len();
    let mut parts: Vec<String> = failures
        .iter()
        .take(3)
        .map(|f| format!("{} ({})", f.repo_name, f.repo_id))
        .collect();
    if count > 3 {
        parts.push(format!("... +{} more", count - 3));
    }
    format!(
        "{}: {} repository failures: {}",
        label,
        count,
        parts.join(", ")
    )
}
