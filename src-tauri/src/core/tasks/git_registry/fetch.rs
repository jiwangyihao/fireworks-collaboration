use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::git::errors::GitError;
use crate::core::tasks::retry::{backoff_delay_ms, categorize, is_retryable};
use crate::events::emitter::{emit_all, AppHandle};
use crate::events::structured::{
    publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
    TransportEvent as StructuredTransportEvent,
};

use super::super::registry::{TaskRegistry, EV_PROGRESS};
use crate::core::tasks::model::{TaskErrorEvent, TaskProgressEvent, TaskState};

impl TaskRegistry {
    pub fn spawn_git_fetch_task_with_opts(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        repo: String,
        dest: String,
        preset: Option<String>,
        depth: Option<serde_json::Value>,
        filter: Option<String>,
        strategy_override: Option<serde_json::Value>,
        progress_hook: Option<Arc<dyn Fn(TaskProgressEvent) + Send + Sync>>,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        let progress_hook_outer = progress_hook.clone();
        tokio::task::spawn_blocking(move || {
            let progress_hook = progress_hook_outer;
            fn emit_adaptive_tls_observability(id: Uuid, kind: &str) {
                use crate::core::git::transport::{
                    metrics_enabled, tl_snapshot, tl_take_fallback_events, FallbackEventRecord,
                };
                use crate::events::structured::{
                    publish_global, Event as StructuredEvent,
                    StrategyEvent as StructuredStrategyEvent,
                };
                let fallback_events = tl_take_fallback_events();
                if metrics_enabled() {
                    let snap = tl_snapshot();
                    if let Some(t) = snap.timing {
                        publish_global(StructuredEvent::Strategy(
                            StructuredStrategyEvent::AdaptiveTlsTiming {
                                id: id.to_string(),
                                kind: kind.to_string(),
                                used_fake_sni: snap.used_fake.unwrap_or(false),
                                fallback_stage: snap
                                    .fallback_stage
                                    .unwrap_or("Unknown")
                                    .to_string(),
                                connect_ms: t.connect_ms,
                                tls_ms: t.tls_ms,
                                first_byte_ms: t.first_byte_ms,
                                total_ms: t.total_ms,
                                cert_fp_changed: snap.cert_fp_changed.unwrap_or(false),
                                ip_source: snap.ip_source.clone(),
                                ip_latency_ms: snap.ip_latency_ms,
                                ip_selection_stage: snap.ip_strategy.map(|s| s.to_string()),
                            },
                        ));
                    }
                }
                for evt in fallback_events {
                    match evt {
                        FallbackEventRecord::Transition { from, to, reason } => {
                            let snap = tl_snapshot();
                            publish_global(StructuredEvent::Strategy(
                                StructuredStrategyEvent::AdaptiveTlsFallback {
                                    id: id.to_string(),
                                    kind: kind.to_string(),
                                    from: from.to_string(),
                                    to: to.to_string(),
                                    reason,
                                    ip_source: snap.ip_source.clone(),
                                    ip_latency_ms: snap.ip_latency_ms,
                                },
                            ));
                        }
                        FallbackEventRecord::AutoDisable {
                            enabled,
                            threshold_pct,
                            cooldown_secs,
                        } => {
                            publish_global(StructuredEvent::Strategy(
                                StructuredStrategyEvent::AdaptiveTlsAutoDisable {
                                    id: id.to_string(),
                                    kind: kind.to_string(),
                                    enabled,
                                    threshold_pct,
                                    cooldown_secs,
                                },
                            ));
                        }
                    }
                }
            }
            let _ = &preset;
            match &app {
                Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running),
                None => this.set_state_noemit(&id, TaskState::Running),
            }
            this.publish_lifecycle_started(&id, "GitFetch");

            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent {
                    task_id: id,
                    kind: "GitFetch".into(),
                    phase: "Starting".into(),
                    percent: 0,
                    objects: None,
                    bytes: None,
                    total_hint: None,
                    retried_times: None,
                };
                emit_all(app_ref, EV_PROGRESS, &prog);
                if let Some(hook) = &progress_hook {
                    hook(prog.clone());
                }
            }

            if token.is_cancelled() {
                if let Some(app_ref) = &app {
                    let err = TaskErrorEvent::from_parts(
                        id,
                        "GitFetch",
                        crate::core::git::errors::ErrorCategory::Cancel,
                        "user canceled",
                        None,
                    );
                    this.emit_error(app_ref, &err);
                }
                match &app {
                    Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled),
                    None => this.set_state_noemit(&id, TaskState::Canceled),
                }
                this.publish_lifecycle_canceled(&id);
                return;
            }

            let parsed_options_res = crate::core::git::default_impl::opts::parse_depth_filter_opts(
                depth.clone(),
                filter.clone(),
                strategy_override.clone(),
            );
            let global_cfg = TaskRegistry::runtime_config();
            let mut effective_follow_redirects: bool = global_cfg.http.follow_redirects;
            let mut effective_max_redirects: u8 = global_cfg.http.max_redirects;
            let mut retry_plan: crate::core::tasks::retry::RetryPlan =
                global_cfg.retry.clone().into();
            let mut depth_applied: Option<u32> = None;
            let mut filter_requested: Option<String> = None;
            let mut applied_codes: Vec<String> = vec![];
            if let Err(e) = parsed_options_res {
                let msg_string = e.to_string();
                if msg_string.contains("unsupported filter:") {
                    publish_global(StructuredEvent::Transport(
                        StructuredTransportEvent::PartialFilterUnsupported {
                            id: id.to_string(),
                            requested: msg_string.clone(),
                        },
                    ));
                }
                if let Some(app_ref) = &app {
                    let err_evt = TaskErrorEvent::from_parts(
                        id,
                        "GitFetch",
                        categorize(&e),
                        format!("{e}"),
                        None,
                    );
                    this.emit_error(app_ref, &err_evt);
                }
                match &app {
                    Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Failed),
                    None => this.set_state_noemit(&id, TaskState::Failed),
                }
                this.emit_error_structured(&TaskErrorEvent {
                    task_id: id,
                    kind: "GitFetch".into(),
                    category: "Runtime".into(),
                    code: Some("fetch_failed".into()),
                    message: format!("fatal: {e}"),
                    retried_times: None,
                });
                return;
            } else if let Ok(opts) = parsed_options_res.as_ref() {
                if opts.filter.is_some() {
                    publish_global(StructuredEvent::Transport(
                        StructuredTransportEvent::PartialFilterCapability {
                            id: id.to_string(),
                            supported: global_cfg.partial_filter_supported,
                        },
                    ));
                }
                if !opts.ignored_top_level.is_empty() || !opts.ignored_nested.is_empty() {
                    publish_global(StructuredEvent::Strategy(
                        StructuredStrategyEvent::IgnoredFields {
                            id: id.to_string(),
                            kind: "GitFetch".into(),
                            top_level: opts.ignored_top_level.clone(),
                            nested: opts
                                .ignored_nested
                                .iter()
                                .map(|(s, k)| format!("{s}.{k}"))
                                .collect(),
                        },
                    ));
                }
                depth_applied = opts.depth;
                if let Some(f) = opts.filter.as_ref() {
                    filter_requested = Some(f.as_str().to_string());
                }
                if let Some(http_over) = opts
                    .strategy_override
                    .as_ref()
                    .and_then(|s| s.http.as_ref())
                {
                    let (f, m, changed, conflict) = TaskRegistry::apply_http_override(
                        "GitFetch",
                        &id,
                        &global_cfg,
                        Some(http_over),
                    );
                    effective_follow_redirects = f;
                    effective_max_redirects = m;
                    if changed {
                        publish_global(StructuredEvent::Strategy(
                            StructuredStrategyEvent::HttpApplied {
                                id: id.to_string(),
                                follow: f,
                                max_redirects: m,
                            },
                        ));
                        applied_codes.push("http_strategy_override_applied".into());
                    }
                    if let Some(_conflict_msg) = conflict {}
                }
                if let Some(retry_over) = opts
                    .strategy_override
                    .as_ref()
                    .and_then(|s| s.retry.as_ref())
                {
                    let (plan, changed) =
                        TaskRegistry::apply_retry_override(&global_cfg.retry, Some(retry_over));
                    retry_plan = plan;
                    if changed {
                        applied_codes.push("retry_strategy_override_applied".into());
                    }
                }
                tracing::info!(
                    target = "git",
                    depth = ?opts.depth,
                    filter = ?opts.filter.as_ref().map(|f| f.as_str()),
                    has_strategy = ?opts.strategy_override.is_some(),
                    strategy_http_follow = ?effective_follow_redirects,
                    strategy_http_max_redirects = ?effective_max_redirects,
                    "git_fetch options accepted (depth/filter/strategy parsed)"
                );
                if let Some((_, shallow)) = TaskRegistry::decide_partial_fallback(
                    depth_applied,
                    filter_requested.as_deref(),
                    global_cfg.partial_filter_supported,
                ) {
                    publish_global(StructuredEvent::Transport(
                        StructuredTransportEvent::PartialFilterFallback {
                            id: id.to_string(),
                            shallow,
                            message: "partial_filter_fallback".into(),
                        },
                    ));
                }
                TaskRegistry::emit_strategy_summary(
                    &app,
                    id,
                    "GitFetch",
                    (effective_follow_redirects, effective_max_redirects),
                    &retry_plan,
                    applied_codes.clone(),
                    filter_requested.is_some(),
                );
                let rollout =
                    crate::core::git::transport::decide_https_to_custom(&global_cfg, repo.as_str());
                if rollout.eligible {
                    let percent = global_cfg.http.fake_sni_rollout_percent;
                    publish_global(StructuredEvent::Strategy(
                        StructuredStrategyEvent::AdaptiveTlsRollout {
                            id: id.to_string(),
                            kind: "GitFetch".into(),
                            percent_applied: percent,
                            sampled: rollout.sampled,
                        },
                    ));
                }
            }

            let plan = retry_plan.clone();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() {
                    match &app {
                        Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled),
                        None => this.set_state_noemit(&id, TaskState::Canceled),
                    }
                    break;
                }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let interrupt_for_thread = std::sync::Arc::clone(&interrupt_flag);
                let token_for_thread = token.clone();
                let watcher = std::thread::spawn(move || {
                    while !token_for_thread.is_cancelled()
                        && !interrupt_for_thread.load(std::sync::atomic::Ordering::Relaxed)
                    {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    if token_for_thread.is_cancelled() {
                        interrupt_for_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });

                if let Some(app_ref) = &app {
                    let prog = TaskProgressEvent {
                        task_id: id,
                        kind: "GitFetch".into(),
                        phase: "Fetching".into(),
                        percent: 10,
                        objects: None,
                        bytes: None,
                        total_hint: None,
                        retried_times: None,
                    };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                    if let Some(hook) = &progress_hook {
                        hook(prog.clone());
                    }
                }

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id;
                    let hook_for_cb = progress_hook.clone();
                    service.fetch_blocking(
                        repo.as_str(),
                        &dest_path,
                        depth_applied,
                        &interrupt_flag,
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
                                if let Some(hook) = &hook_for_cb {
                                    hook(prog.clone());
                                }
                            }
                        },
                    )
                };

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed)
                {
                    if let Some(app_ref) = &app {
                        let err = TaskErrorEvent::from_parts(
                            id,
                            "GitFetch",
                            crate::core::git::errors::ErrorCategory::Cancel,
                            "user canceled",
                            None,
                        );
                        this.emit_error(app_ref, &err);
                    }
                    match &app {
                        Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled),
                        None => this.set_state_noemit(&id, TaskState::Canceled),
                    }
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app {
                            let prog = TaskProgressEvent {
                                task_id: id,
                                kind: "GitFetch".into(),
                                phase: "Completed".into(),
                                percent: 100,
                                objects: None,
                                bytes: None,
                                total_hint: None,
                                retried_times: None,
                            };
                            emit_all(app_ref, EV_PROGRESS, &prog);
                            if let Some(hook) = &progress_hook {
                                hook(prog.clone());
                            }
                        }
                        emit_adaptive_tls_observability(id, "GitFetch");
                        match &app {
                            Some(app_ref) => {
                                this.set_state_emit(app_ref, &id, TaskState::Completed)
                            }
                            None => this.set_state_noemit(&id, TaskState::Completed),
                        }
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "fetch error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(
                                id,
                                "GitFetch",
                                cat,
                                format!("{e}"),
                                Some(attempt),
                            );
                            this.emit_error(app_ref, &err_evt);
                        }
                        if is_retryable(&e) && attempt < plan.max {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!(
                                    "Retrying (attempt {} of {}) in {} ms",
                                    attempt, plan.max, delay
                                );
                                let prog = TaskProgressEvent {
                                    task_id: id,
                                    kind: "GitFetch".into(),
                                    phase,
                                    percent: 0,
                                    objects: None,
                                    bytes: None,
                                    total_hint: None,
                                    retried_times: Some(attempt),
                                };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                                if let Some(hook) = &progress_hook {
                                    hook(prog.clone());
                                }
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            emit_adaptive_tls_observability(id, "GitFetch");
                            match &app {
                                Some(app_ref) => {
                                    this.set_state_emit(app_ref, &id, TaskState::Failed)
                                }
                                None => this.set_state_noemit(&id, TaskState::Failed),
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    pub fn spawn_git_fetch_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        repo: String,
        dest: String,
        preset: Option<String>,
    ) -> JoinHandle<()> {
        self.spawn_git_fetch_task_with_opts(
            app, id, token, repo, dest, preset, None, None, None, None,
        )
    }
}
