use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::git::errors::GitError;
use crate::core::tasks::retry::{
    backoff_delay_ms, categorize, compute_retry_diff, is_retryable, load_retry_plan,
};
use crate::events::emitter::{emit_all, AppHandle};
use crate::events::structured::{
    publish_global, Event as StructuredEvent, PolicyEvent as StructuredPolicyEvent,
    StrategyEvent as StructuredStrategyEvent,
};

use super::super::registry::{TaskRegistry, EV_PROGRESS};
use crate::core::tasks::model::{TaskErrorEvent, TaskProgressEvent, TaskState};

impl TaskRegistry {
    pub fn spawn_git_push_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        dest: String,
        remote: Option<String>,
        refspecs: Option<Vec<String>>,
        username: Option<String>,
        password: Option<String>,
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
            match &app {
                Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running),
                None => this.set_state_noemit(&id, TaskState::Running),
            }
            this.publish_lifecycle_started(&id, "GitPush");

            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent {
                    task_id: id,
                    kind: "GitPush".into(),
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
                        "GitPush",
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

            let global_cfg = TaskRegistry::runtime_config();
            let mut effective_follow_redirects: bool = global_cfg.http.follow_redirects;
            let mut effective_max_redirects: u8 = global_cfg.http.max_redirects;
            let mut retry_plan: crate::core::tasks::retry::RetryPlan =
                global_cfg.retry.clone().into();
            let mut effective_insecure_skip_verify: bool = global_cfg.tls.insecure_skip_verify;
            let mut effective_skip_san_whitelist: bool = global_cfg.tls.skip_san_whitelist;
            let mut applied_codes: Vec<String> = vec![];
            if let Some(raw) = strategy_override.clone() {
                use crate::core::git::default_impl::opts::parse_strategy_override;
                match parse_strategy_override(Some(raw)) {
                    Err(e) => {
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(
                                id,
                                "GitPush",
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
                        return;
                    }
                    Ok(parsed_res) => {
                        if !parsed_res.ignored_top_level.is_empty()
                            || !parsed_res.ignored_nested.is_empty()
                        {
                            publish_global(StructuredEvent::Strategy(
                                StructuredStrategyEvent::IgnoredFields {
                                    id: id.to_string(),
                                    kind: "GitPush".into(),
                                    top_level: parsed_res.ignored_top_level.clone(),
                                    nested: parsed_res
                                        .ignored_nested
                                        .iter()
                                        .map(|(s, k)| format!("{s}.{k}"))
                                        .collect(),
                                },
                            ));
                        }
                        if let Some(parsed) = parsed_res.parsed {
                            if let Some(http_over) = parsed.http.as_ref() {
                                let (f, m, changed, conflict) = TaskRegistry::apply_http_override(
                                    "GitPush",
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
                                if let Some(msg) = conflict {
                                    if let Some(app_ref) = &app {
                                        let evt = TaskErrorEvent {
                                            task_id: id,
                                            kind: "GitPush".into(),
                                            category: "Protocol".into(),
                                            code: Some("strategy_override_conflict".into()),
                                            message: format!("http conflict: {msg}"),
                                            retried_times: None,
                                        };
                                        this.emit_error(app_ref, &evt);
                                    }
                                }
                            }
                            if let Some(tls_over) = parsed.tls.as_ref() {
                                let (ins, skip, changed, conflict) =
                                    TaskRegistry::apply_tls_override(
                                        "GitPush",
                                        &id,
                                        &global_cfg,
                                        Some(tls_over),
                                    );
                                effective_insecure_skip_verify = ins;
                                effective_skip_san_whitelist = skip;
                                if changed {
                                    publish_global(StructuredEvent::Strategy(
                                        StructuredStrategyEvent::TlsApplied {
                                            id: id.to_string(),
                                            insecure_skip_verify: ins,
                                            skip_san_whitelist: skip,
                                        },
                                    ));
                                    applied_codes.push("tls_strategy_override_applied".into());
                                }
                                if let Some(msg) = conflict {
                                    if let Some(app_ref) = &app {
                                        let evt = TaskErrorEvent {
                                            task_id: id,
                                            kind: "GitPush".into(),
                                            category: "Protocol".into(),
                                            code: Some("strategy_override_conflict".into()),
                                            message: format!("tls conflict: {msg}"),
                                            retried_times: None,
                                        };
                                        this.emit_error(app_ref, &evt);
                                    }
                                }
                            }
                            if let Some(retry_over) = parsed.retry.as_ref() {
                                let (plan_new, changed) = TaskRegistry::apply_retry_override(
                                    &global_cfg.retry,
                                    Some(retry_over),
                                );
                                if changed {
                                    let base_plan = load_retry_plan();
                                    let (diff, _) = compute_retry_diff(&base_plan, &plan_new);
                                    publish_global(StructuredEvent::Policy(
                                        StructuredPolicyEvent::RetryApplied {
                                            id: id.to_string(),
                                            code: "retry_strategy_override_applied".to_string(),
                                            changed: diff
                                                .changed
                                                .into_iter()
                                                .map(|s| s.to_string())
                                                .collect(),
                                        },
                                    ));
                                    applied_codes.push("retry_strategy_override_applied".into());
                                }
                                retry_plan = plan_new;
                            }
                            tracing::info!(
                                target = "strategy",
                                kind = "push",
                                has_override = true,
                                http_follow = ?effective_follow_redirects,
                                http_max_redirects = ?effective_max_redirects,
                                tls_insecure = ?effective_insecure_skip_verify,
                                tls_skip_san = ?effective_skip_san_whitelist,
                                strategy_override_valid = true,
                                "strategyOverride accepted for push (parse+http/tls apply)"
                            );
                        } else {
                            tracing::info!(
                                target = "strategy",
                                kind = "push",
                                has_override = true,
                                http_follow = ?effective_follow_redirects,
                                http_max_redirects = ?effective_max_redirects,
                                tls_insecure = ?effective_insecure_skip_verify,
                                tls_skip_san = ?effective_skip_san_whitelist,
                                strategy_override_valid = true,
                                "strategyOverride accepted for push (empty object)"
                            );
                        }
                    }
                }
            }

            tracing::debug!(
                target = "strategy",
                kind = "push",
                task_id = %id,
                applied_codes = ?applied_codes,
                "emit push strategy summary"
            );
            applied_codes.sort();
            applied_codes.dedup();
            TaskRegistry::emit_strategy_summary(
                &app,
                id,
                "GitPush",
                (effective_follow_redirects, effective_max_redirects),
                &retry_plan,
                (effective_insecure_skip_verify, effective_skip_san_whitelist),
                applied_codes.clone(),
                false,
            );
            let rollout =
                crate::core::git::transport::decide_https_to_custom(&global_cfg, dest.as_str());
            if rollout.eligible {
                let percent = global_cfg.http.fake_sni_rollout_percent;
                publish_global(StructuredEvent::Strategy(
                    StructuredStrategyEvent::AdaptiveTlsRollout {
                        id: id.to_string(),
                        kind: "GitPush".into(),
                        percent_applied: percent,
                        sampled: rollout.sampled,
                    },
                ));
            }

            let plan = retry_plan;
            let mut attempt: u32 = 0;
            let upload_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            loop {
                if token.is_cancelled() {
                    match &app {
                        Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled),
                        None => this.set_state_noemit(&id, TaskState::Canceled),
                    }
                    this.publish_lifecycle_canceled(&id);
                    break;
                }

                let interrupt_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                upload_started.store(false, std::sync::atomic::Ordering::Relaxed);
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

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id;
                    let hook_for_cb = progress_hook.clone();
                    let upload_started_cb = std::sync::Arc::clone(&upload_started);
                    let creds_opt = match (username.as_deref(), password.as_deref()) {
                        (Some(u), Some(p)) if !u.is_empty() => Some((u, p)),
                        (_, Some(p)) => Some(("x-access-token", p)),
                        _ => None,
                    };
                    let refspecs_vec: Option<Vec<String>> = refspecs.clone();
                    let refspecs_slices: Option<Vec<&str>> = refspecs_vec
                        .as_ref()
                        .map(|v| v.iter().map(|s| s.as_str()).collect());
                    service.push_blocking(
                        &dest_path,
                        remote.as_deref(),
                        refspecs_slices.as_deref(),
                        creds_opt,
                        &interrupt_flag,
                        move |p| {
                            if p.phase == "Upload" {
                                upload_started_cb.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
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
                            "GitPush",
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
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app {
                            let prog = TaskProgressEvent {
                                task_id: id,
                                kind: "GitPush".into(),
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
                        emit_adaptive_tls_observability(id, "GitPush");
                        match &app {
                            Some(app_ref) => {
                                this.set_state_emit(app_ref, &id, TaskState::Completed)
                            }
                            None => this.set_state_noemit(&id, TaskState::Completed),
                        }
                        this.publish_lifecycle_completed(&id);
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "push error: {}", e);
                        if let Some(app_ref) = &app {
                            let err_evt = TaskErrorEvent::from_parts(
                                id,
                                "GitPush",
                                cat,
                                format!("{e}"),
                                Some(attempt),
                            );
                            this.emit_error(app_ref, &err_evt);
                        }
                        if !upload_started.load(std::sync::atomic::Ordering::Relaxed)
                            && is_retryable(&e)
                            && attempt < plan.max
                        {
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!(
                                    "Retrying (attempt {} of {}) in {} ms",
                                    attempt, plan.max, delay
                                );
                                let prog = TaskProgressEvent {
                                    task_id: id,
                                    kind: "GitPush".into(),
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
                            emit_adaptive_tls_observability(id, "GitPush");
                            match &app {
                                Some(app_ref) => {
                                    this.set_state_emit(app_ref, &id, TaskState::Failed)
                                }
                                None => {
                                    this.set_state_noemit(&id, TaskState::Failed);
                                    this.publish_lifecycle_failed_if_needed(
                                        &id,
                                        "failed without error event",
                                    );
                                }
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
}
