use std::{collections::HashMap, sync::{Arc, Mutex}, time::SystemTime};
use tokio::{time::{sleep, Duration}, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::events::emitter::{emit_all, AppHandle};
use super::model::{TaskMeta, TaskKind, TaskState, TaskSnapshot, TaskStateEvent, TaskProgressEvent, TaskErrorEvent};
use super::retry::{load_retry_plan, backoff_delay_ms, is_retryable, categorize};
use crate::core::git::errors::GitError;

const EV_STATE: &str = "task://state";
const EV_PROGRESS: &str = "task://progress";
const EV_ERROR: &str = "task://error";

#[derive(Debug)]
pub struct TaskRegistry {
    inner: Mutex<HashMap<Uuid, TaskMeta>>,
}

impl TaskRegistry {
    pub fn new() -> Self { Self { inner: Mutex::new(HashMap::new()) } }

    pub fn create(&self, kind: TaskKind) -> (Uuid, CancellationToken) {
        let id = Uuid::new_v4();
        let token = CancellationToken::new();
        let meta = TaskMeta { id, kind, state: TaskState::Pending, created_at: SystemTime::now(), cancel_token: token.clone(), fail_reason: None };
        self.inner.lock().unwrap().insert(id, meta);
        (id, token)
    }

    pub fn list(&self) -> Vec<TaskSnapshot> { self.inner.lock().unwrap().values().map(TaskSnapshot::from).collect() }

    pub fn snapshot(&self, id: &Uuid) -> Option<TaskSnapshot> { self.inner.lock().unwrap().get(id).map(TaskSnapshot::from) }

    pub fn cancel(&self, id: &Uuid) -> bool { self.inner.lock().unwrap().get(id).map(|m| { m.cancel_token.cancel(); true }).unwrap_or(false) }

    fn with_meta<F: FnOnce(&mut TaskMeta)>(&self, id: &Uuid, f: F) -> Option<TaskMeta> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(m) = guard.get_mut(id) { f(m); Some(m.clone()) } else { None }
    }

    fn emit_state(&self, app:&AppHandle, id:&Uuid) { if let Some(m) = self.inner.lock().unwrap().get(id) { let evt = TaskStateEvent::new(m); emit_all(app, EV_STATE, &evt); } }

    fn set_state_emit(&self, app:&AppHandle, id:&Uuid, s:TaskState){ if self.with_meta(id, |m| m.state = s).is_some(){ self.emit_state(app, id);} }

    fn set_state_noemit(&self, id:&Uuid, s:TaskState){ let _ = self.with_meta(id, |m| m.state = s); }

    fn emit_error(&self, app:&AppHandle, evt:&TaskErrorEvent) { emit_all(app, EV_ERROR, evt); }

    pub fn spawn_sleep_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, total_ms: u64) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            let step = 50u64; // 更细颗粒度便于测试
            let mut elapsed = 0u64;
            while elapsed < total_ms {
                if token.is_cancelled(){ match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled)}; return; }
                sleep(Duration::from_millis(step)).await;
                elapsed += step;
                if let Some(app_ref) = &app {
                    let percent = ((elapsed.min(total_ms) as f64 / total_ms as f64) * 100.0) as u32;
                    let prog = TaskProgressEvent { task_id: id, kind: "Sleep".into(), phase: "Running".into(), percent, objects: None, bytes: None, total_hint: None, retried_times: None };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }
            }
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }
        })
    }

    /// 启动 Git Clone 任务（阻塞线程执行），支持取消与基本进度事件
    /// Decide whether to emit a partial filter fallback event.
    /// Placeholder capability model: always unsupported for now. Once real capability
    /// detection is available we plug it in here. Returns the optional (message, shallow_mode)
    /// where shallow_mode=true means depth retained (depth+filter) and false means full.
    fn decide_partial_fallback(depth_applied: Option<u32>, filter_requested: Option<&str>) -> Option<(String, bool)> {
        if filter_requested.is_none() { return None; }
        // Capability simulation: if env FWC_PARTIAL_FILTER_CAPABLE=1 treat as supported (no fallback event)
        let capable = std::env::var("FWC_PARTIAL_FILTER_CAPABLE").map(|v| v=="1").unwrap_or(false);
        if capable { return None; }
        let shallow = depth_applied.is_some();
        let msg = if shallow { "partial filter unsupported; fallback=shallow (depth retained)".to_string() } else { "partial filter unsupported; fallback=full".to_string() };
        Some((msg, shallow))
    }

    pub fn spawn_git_clone_task_with_opts(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, depth: Option<serde_json::Value>, filter: Option<String>, strategy_override: Option<serde_json::Value>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }

            // 预发一个开始事件
            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            // 为测试与前端观察提供一个极短缓冲，确保能够看到 Running 状态
            std::thread::sleep(std::time::Duration::from_millis(50));

            // 提前检查取消
            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitClone", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                return;
            }

            // 参数解析：depth 已在 P2.2b 生效；filter 在 P2.2d 引入占位（当前不真正启用 partial，进行回退提示）
            let parsed_options_res = crate::core::git::default_impl::opts::parse_depth_filter_opts(depth.clone(), filter.clone(), strategy_override.clone());
            let mut depth_applied: Option<u32> = None;
            let mut filter_requested: Option<String> = None; // 记录用户请求的 filter（用于回退信息）
            if let Err(e) = parsed_options_res {
                // 直接作为 Protocol/错误分类失败
                if let Some(app_ref) = &app { let err_evt = TaskErrorEvent::from_parts(id, "GitClone", super::retry::categorize(&e), format!("{}", e), None); this.emit_error(app_ref, &err_evt); }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                return;
            } else {
                if let Some(opts) = parsed_options_res.ok() {
                    depth_applied = opts.depth;
                    if let Some(f) = opts.filter.as_ref() { filter_requested = Some(f.as_str().to_string()); }
                    tracing::info!(target="git", depth=?opts.depth, filter=?opts.filter.as_ref().map(|f| f.as_str()), has_strategy=?opts.strategy_override.is_some(), "git_clone options accepted (depth active; filter parsed)");
                    // P2.2d: 当前阶段尚未真正启用 partial clone，若用户请求了 filter，需要发送一次非阻断回退提示。
                    if let Some((msg, _shallow)) = Self::decide_partial_fallback(depth_applied, filter_requested.as_deref()) {
                        if let Some(app_ref) = &app {
                            let warn_evt = TaskErrorEvent { task_id: id, kind: "GitClone".into(), category: "Protocol".into(), code: Some("partial_filter_fallback".into()), message: msg, retried_times: None };
                            this.emit_error(app_ref, &warn_evt);
                        }
                    }
                }
            }

            let plan = load_retry_plan();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() { match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }; break; }

                // per-attempt interrupt flag and watcher
                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() {
                        interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });

                // 执行克隆
                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    service
                        .clone_blocking(
                            repo.as_str(),
                            &dest_path,
                            depth_applied,
                            &*interrupt_flag,
                            move |p| {
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitClone", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app {
                            let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None };
                            emit_all(app_ref, EV_PROGRESS, &prog);
                        }
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "clone error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitClone", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        if is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitClone".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    /// 启动 Git Fetch 任务（阻塞线程执行），支持取消与基本进度事件
    pub fn spawn_git_fetch_task_with_opts(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, preset: Option<String>, depth: Option<serde_json::Value>, filter: Option<String>, strategy_override: Option<serde_json::Value>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let _ = &preset; // 目前 git2 路径未使用该预设参数
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }

            // 预发一个开始事件
            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitFetch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                return;
            }

            // 解析与校验（P2.2a+c）；P2.2e：若用户请求 filter（partial fetch 尚未真正启用）发送非阻断回退事件
            let parsed_options_res = crate::core::git::default_impl::opts::parse_depth_filter_opts(depth.clone(), filter.clone(), strategy_override.clone());
            let mut depth_applied: Option<u32> = None;
            let mut filter_requested: Option<String> = None;
            if let Err(e) = parsed_options_res {
                if let Some(app_ref) = &app { let err_evt = TaskErrorEvent::from_parts(id, "GitFetch", super::retry::categorize(&e), format!("{}", e), None); this.emit_error(app_ref, &err_evt); }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                return;
            } else if let Ok(opts) = parsed_options_res.as_ref() {
                depth_applied = opts.depth; // P2.2c: depth now effective
                if let Some(f) = opts.filter.as_ref() { filter_requested = Some(f.as_str().to_string()); }
                tracing::info!(target="git", depth=?opts.depth, filter=?opts.filter.as_ref().map(|f| f.as_str()), has_strategy=?opts.strategy_override.is_some(), "git_fetch options accepted (depth active; filter parsed)");
                if let Some((msg, _shallow)) = Self::decide_partial_fallback(depth_applied, filter_requested.as_deref()) {
                    if let Some(app_ref) = &app {
                        let warn_evt = TaskErrorEvent { task_id: id, kind: "GitFetch".into(), category: "Protocol".into(), code: Some("partial_filter_fallback".into()), message: msg, retried_times: None };
                        this.emit_error(app_ref, &warn_evt);
                    }
                }
            }

            let plan = load_retry_plan();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() {
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    break;
                }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() { interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed); }
                });

                // 进入阶段性进度
                if let Some(app_ref) = &app {
                    let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Fetching".into(), percent: 10, objects: None, bytes: None, total_hint: None, retried_times: None };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    service
                        .fetch_blocking(
                            repo.as_str(),
                            &dest_path,
                            depth_applied,
                            &*interrupt_flag,
                            move |p| {
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitFetch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app { let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "fetch error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitFetch", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        if is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitFetch".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    // Backward compatible wrappers (P2.2a): tests and existing callers without new params
    pub fn spawn_git_clone_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String) -> JoinHandle<()> {
        self.spawn_git_clone_task_with_opts(app, id, token, repo, dest, None, None, None)
    }

    pub fn spawn_git_fetch_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, repo: String, dest: String, preset: Option<String>) -> JoinHandle<()> {
        self.spawn_git_fetch_task_with_opts(app, id, token, repo, dest, preset, None, None, None)
    }

    /// 启动 Git Push 任务（阻塞线程执行），支持取消与阶段事件
    pub fn spawn_git_push_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, remote: Option<String>, refspecs: Option<Vec<String>>, username: Option<String>, password: Option<String>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }

            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase: "Starting".into(), percent: 0, objects: None, bytes: None, total_hint: None, retried_times: None };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(id, "GitPush", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                    this.emit_error(app_ref, &err);
                }
                match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                return;
            }

            let plan = load_retry_plan();
            let mut attempt: u32 = 0;
            // 用于检测是否进入上传阶段（进入后不再自动重试）
            let upload_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            loop {
                if token.is_cancelled() { match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }; break; }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                upload_started.store(false, std::sync::atomic::Ordering::Relaxed);
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled() && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() { interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed); }
                });

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    let upload_started_cb = std::sync::Arc::clone(&upload_started);
                    // 允许仅提供 token（password）；若 username 为空或缺失，默认使用 "x-access-token"
                    let creds_opt = match (username.as_deref(), password.as_deref()) {
                        (Some(u), Some(p)) if !u.is_empty() => Some((u, p)),
                        (None, Some(p)) => Some(("x-access-token", p)),
                        (Some(u), Some(p)) if u.is_empty() => Some(("x-access-token", p)),
                        _ => None
                    };
                    let refspecs_vec: Option<Vec<String>> = refspecs.clone();
                    let refspecs_opt: Option<Vec<String>> = refspecs_vec;
                    let refspecs_slices: Option<Vec<&str>> = refspecs_opt.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());
                    service
                        .push_blocking(
                            &dest_path,
                            remote.as_deref(),
                            refspecs_slices.as_deref(),
                            creds_opt.map(|(u,p)| (u, p)),
                            &*interrupt_flag,
                            move |p| {
                                if p.phase == "Upload" { upload_started_cb.store(true, std::sync::atomic::Ordering::Relaxed); }
                                if let Some(app_ref) = &app_for_cb {
                                    let prog = TaskProgressEvent {
                                        task_id: id_for_cb,
                                        kind: p.kind,
                                        phase: p.phase,
                                        percent: p.percent,
                                        objects: p.objects,
                                        bytes: p.bytes,
                                        total_hint: p.total_hint,
                                        retried_times: None,
                                    };
                                    emit_all(app_ref, EV_PROGRESS, &prog);
                                }
                            },
                        )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(id, "GitPush", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None);
                        this.emit_error(app_ref, &err);
                    }
                    match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled), None => this.set_state_noemit(&id, TaskState::Canceled) }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app { let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                        match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed), None => this.set_state_noemit(&id, TaskState::Completed) }
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "push error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(id, "GitPush", cat, format!("{}", e), Some(attempt));
                            this.emit_error(app_ref, &err_evt);
                        }
                        // 仅在尚未进入上传阶段时允许自动重试
                        if !upload_started.load(std::sync::atomic::Ordering::Relaxed) && is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!("Retrying (attempt {} of {}) in {} ms", attempt, plan.max, delay);
                                let prog = TaskProgressEvent { task_id: id, kind: "GitPush".into(), phase, percent: 0, objects: None, bytes: None, total_hint: None, retried_times: Some(attempt) };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed), None => this.set_state_noemit(&id, TaskState::Failed) }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    /// 启动 Git Init 任务：初始化一个新的仓库
    pub fn spawn_git_init_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { // 取消早退
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitInit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::init::git_init(&dest_path, &*interrupt_flag, move |_p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: "GitInit".into(), phase: "Running".into(), percent: 100, objects: None, bytes: None, total_hint: None, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitInit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitInit", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) }
                }
            }
        })
    }

    /// 启动 Git Add 任务：暂存文件
    pub fn spawn_git_add_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, paths: Vec<String>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let path_vec = paths.clone();
            let ref_slices: Vec<&str> = path_vec.iter().map(|s| s.as_str()).collect();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::add::git_add(&dest_path, &ref_slices, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitAdd", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) }
                }
            }
        })
    }

    /// 启动 Git Commit 任务：创建一次提交
    pub fn spawn_git_commit_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, message: String, allow_empty: bool, author_name: Option<String>, author_email: Option<String>) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCommit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                let author_opt = match (author_name.as_deref(), author_email.as_deref()) {
                    (Some(n), Some(e)) => Some(crate::core::git::default_impl::commit::Author { name: Some(n), email: Some(e) }),
                    _ => None,
                };
                crate::core::git::default_impl::commit::git_commit(&dest_path, &message, author_opt, allow_empty, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb {
                        let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None };
                        emit_all(app_ref, EV_PROGRESS, &prog);
                    }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCommit", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} 
                match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) }
                return;
            }
            match res {
                Ok(()) => { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } },
                Err(e) => {
                    let cat = super::retry::categorize(&e);
                    if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitCommit", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} 
                    match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) }
                }
            }
        })
    }

    /// 启动 Git Branch 任务：创建/强制更新/可选检出分支
    pub fn spawn_git_branch_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, checkout: bool, force: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitBranch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::branch::git_branch(&dest_path, &name, checkout, force, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitBranch", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitBranch", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }

    /// 启动 Git Checkout 任务：切换或创建+切换分支
    pub fn spawn_git_checkout_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, reference: String, create: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCheckout", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::checkout::git_checkout(&dest_path, &reference, create, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitCheckout", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitCheckout", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }

    /// 启动 Git Tag 任务：创建/更新标签
    pub fn spawn_git_tag_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, message: Option<String>, annotated: bool, force: bool) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitTag", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let msg_opt = message.clone();
            let res: Result<(), GitError> = {
                let app_for_cb = app.clone();
                let id_for_cb = id.clone();
                crate::core::git::default_impl::tag::git_tag(&dest_path, &name, msg_opt.as_deref(), annotated, force, &*interrupt_flag, move |p| {
                    if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); }
                })
            };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitTag", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitTag", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }

    pub fn spawn_git_remote_set_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, url: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteSet", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_set(&dest_path, &name, &url, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteSet", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteSet", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }

    pub fn spawn_git_remote_add_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String, url: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_add(&dest_path, &name, &url, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteAdd", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteAdd", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }

    pub fn spawn_git_remote_remove_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, dest: String, name: String) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Running), None=> this.set_state_noemit(&id,TaskState::Running) }
            if token.is_cancelled() { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteRemove", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dest_path = std::path::PathBuf::from(dest.clone());
            let res: Result<(), GitError> = { let app_for_cb = app.clone(); let id_for_cb = id.clone(); crate::core::git::default_impl::remote::git_remote_remove(&dest_path, &name, &*interrupt_flag, move |p| { if let Some(app_ref)=&app_for_cb { let prog = TaskProgressEvent { task_id: id_for_cb, kind: p.kind, phase: p.phase, percent: p.percent, objects: p.objects, bytes: p.bytes, total_hint: p.total_hint, retried_times: None }; emit_all(app_ref, EV_PROGRESS, &prog); } }) };
            if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed) { if let Some(app_ref)=&app { let err = TaskErrorEvent::from_parts(id, "GitRemoteRemove", crate::core::git::errors::ErrorCategory::Cancel, "user canceled", None); this.emit_error(app_ref,&err);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled) } return; }
            match res { Ok(())=> { match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) } }, Err(e)=> { let cat = super::retry::categorize(&e); if let Some(app_ref)=&app { let err_evt = TaskErrorEvent::from_parts(id, "GitRemoteRemove", cat, format!("{}", e), None); this.emit_error(app_ref,&err_evt);} match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Failed), None=> this.set_state_noemit(&id,TaskState::Failed) } } }
        })
    }
}

// (test helper for fallback recording was removed in cleanup — fallback currently validated by completion test only)

pub type SharedTaskRegistry = Arc<TaskRegistry>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    async fn wait_for_state(reg:&TaskRegistry, id:Uuid, target:TaskState, max_ms:u64) -> bool {
        let mut waited = 0u64;
        while waited < max_ms {
            if let Some(s) = reg.snapshot(&id) { if s.state == target { return true; } }
            sleep(Duration::from_millis(20)).await; waited += 20;
        }
        false
    }

    #[tokio::test]
    async fn test_create_initial_pending() {
        let reg = TaskRegistry::new();
        let (id, _t) = reg.create(TaskKind::Sleep { ms: 100 });
        let snap = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(snap.state, TaskState::Pending));
    }

    #[tokio::test]
    async fn test_sleep_task_completes() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
        let handle = reg.spawn_sleep_task(None, id, token, 120);
        // 等待完成
        let ok = wait_for_state(&reg, id, TaskState::Completed, 1500).await;
        assert!(ok, "task should complete");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cancel_sleep_task() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 500 });
        let handle = reg.spawn_sleep_task(None, id, token.clone(), 500);
        // 取消前先确认进入 running
        let entered = wait_for_state(&reg, id, TaskState::Running, 500).await; assert!(entered, "should enter running");
        token.cancel();
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await; assert!(canceled, "should cancel");
        handle.await.unwrap();
    }
}
