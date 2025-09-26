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
    StrategyEvent as StructuredStrategyEvent, TransportEvent as StructuredTransportEvent,
};

use super::super::base::{TaskRegistry, EV_PROGRESS};
use super::helpers::{handle_cancel, report_failure};
use crate::core::tasks::model::{TaskErrorEvent, TaskProgressEvent};

impl TaskRegistry {
    pub fn spawn_git_clone_task_with_opts(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        repo: String,
        dest: String,
        depth: Option<serde_json::Value>,
        filter: Option<String>,
        strategy_override: Option<serde_json::Value>,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
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
                            },
                        ));
                    }
                }
                for evt in fallback_events {
                    match evt {
                        FallbackEventRecord::Transition { from, to, reason } => {
                            publish_global(StructuredEvent::Strategy(
                                StructuredStrategyEvent::AdaptiveTlsFallback {
                                    id: id.to_string(),
                                    kind: kind.to_string(),
                                    from: from.to_string(),
                                    to: to.to_string(),
                                    reason,
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
            this.mark_running(&app, &id, "GitClone");

            if let Some(app_ref) = &app {
                let prog = TaskProgressEvent {
                    task_id: id,
                    kind: "GitClone".into(),
                    phase: "Starting".into(),
                    percent: 0,
                    objects: None,
                    bytes: None,
                    total_hint: None,
                    retried_times: None,
                };
                emit_all(app_ref, EV_PROGRESS, &prog);
            }

            std::thread::sleep(std::time::Duration::from_millis(50));

            if token.is_cancelled() {
                handle_cancel(&this, &app, &id, "GitClone");
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
            let mut effective_insecure_skip_verify: bool = global_cfg.tls.insecure_skip_verify;
            let mut effective_skip_san_whitelist: bool = global_cfg.tls.skip_san_whitelist;
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
                report_failure(
                    &this,
                    &app,
                    &id,
                    "GitClone",
                    &e,
                    None,
                    "failed without error event",
                );
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
                            kind: "GitClone".into(),
                            top_level: opts.ignored_top_level.clone(),
                            nested: opts
                                .ignored_nested
                                .iter()
                                .map(|(s, k)| format!("{}.{k}", s))
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
                        "GitClone",
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
                    if let Some(conflict_msg) = conflict {
                        publish_global(StructuredEvent::Strategy(
                            StructuredStrategyEvent::Conflict {
                                id: id.to_string(),
                                kind: "http".into(),
                                message: conflict_msg,
                            },
                        ));
                    }
                }
                if let Some(tls_over) = opts.strategy_override.as_ref().and_then(|s| s.tls.as_ref())
                {
                    let (ins, skip, changed, conflict) = TaskRegistry::apply_tls_override(
                        "GitClone",
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
                    if let Some(conflict_msg) = conflict {
                        publish_global(StructuredEvent::Strategy(
                            StructuredStrategyEvent::Conflict {
                                id: id.to_string(),
                                kind: "tls".into(),
                                message: conflict_msg,
                            },
                        ));
                    }
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
                        let base_plan = load_retry_plan();
                        let (diff, _) = compute_retry_diff(&base_plan, &retry_plan);
                        publish_global(StructuredEvent::Policy(
                            StructuredPolicyEvent::RetryApplied {
                                id: id.to_string(),
                                code: "retry_strategy_override_applied".to_string(),
                                changed: diff.changed.into_iter().map(|s| s.to_string()).collect(),
                            },
                        ));
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
                    strategy_tls_insecure = ?effective_insecure_skip_verify,
                    strategy_tls_skip_san = ?effective_skip_san_whitelist,
                    "git_clone options accepted (depth/filter/strategy parsed)"
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
                    "GitClone",
                    (effective_follow_redirects, effective_max_redirects),
                    &retry_plan,
                    (effective_insecure_skip_verify, effective_skip_san_whitelist),
                    applied_codes.clone(),
                    filter_requested.is_some(),
                );
                if let Some(rewritten) = crate::core::git::transport::maybe_rewrite_https_to_custom(
                    &global_cfg,
                    repo.as_str(),
                ) {
                    let _ = rewritten;
                    let percent = global_cfg.http.fake_sni_rollout_percent;
                    publish_global(StructuredEvent::Strategy(
                        StructuredStrategyEvent::AdaptiveTlsRollout {
                            id: id.to_string(),
                            kind: "GitClone".into(),
                            percent_applied: percent as u8,
                            sampled: true,
                        },
                    ));
                }
            }

            let plan = retry_plan.clone();
            let mut attempt: u32 = 0;
            loop {
                if token.is_cancelled() {
                    handle_cancel(&this, &app, &id, "GitClone");
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

                let dest_path = std::path::PathBuf::from(dest.clone());
                let res: Result<(), GitError> = {
                    use crate::core::git::service::GitService;
                    let service = crate::core::git::DefaultGitService::new();
                    let app_for_cb = app.clone();
                    let id_for_cb = id.clone();
                    service.clone_blocking(
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

                if token.is_cancelled() || interrupt_flag.load(std::sync::atomic::Ordering::Relaxed)
                {
                    handle_cancel(&this, &app, &id, "GitClone");
                    interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = watcher.join();
                    break;
                }

                match res {
                    Ok(()) => {
                        if let Some(app_ref) = &app {
                            let prog = TaskProgressEvent {
                                task_id: id,
                                kind: "GitClone".into(),
                                phase: "Completed".into(),
                                percent: 100,
                                objects: None,
                                bytes: None,
                                total_hint: None,
                                retried_times: None,
                            };
                            emit_all(app_ref, EV_PROGRESS, &prog);
                        }
                        emit_adaptive_tls_observability(id, "GitClone");
                        this.mark_completed(&app, &id);
                        interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = watcher.join();
                        break;
                    }
                    Err(e) => {
                        let cat = categorize(&e);
                        tracing::error!(target = "git", category = ?cat, "clone error: {}", e);
                        if is_retryable(&e) && attempt < plan.max {
                            this.emit_error_if_app(&app, || {
                                TaskErrorEvent::from_parts(
                                    id,
                                    "GitClone",
                                    cat,
                                    format!("{}", e),
                                    Some(attempt),
                                )
                            });
                            let delay = backoff_delay_ms(&plan, attempt);
                            attempt += 1;
                            if let Some(app_ref) = &app {
                                let phase = format!(
                                    "Retrying (attempt {} of {}) in {} ms",
                                    attempt, plan.max, delay
                                );
                                let prog = TaskProgressEvent {
                                    task_id: id,
                                    kind: "GitClone".into(),
                                    phase,
                                    percent: 0,
                                    objects: None,
                                    bytes: None,
                                    total_hint: None,
                                    retried_times: Some(attempt),
                                };
                                emit_all(app_ref, EV_PROGRESS, &prog);
                            }
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        } else {
                            emit_adaptive_tls_observability(id, "GitClone");
                            report_failure(
                                &this,
                                &app,
                                &id,
                                "GitClone",
                                &e,
                                Some(attempt),
                                "failed without error event",
                            );
                            interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let _ = watcher.join();
                            break;
                        }
                    }
                }
            }
        })
    }

    pub fn spawn_git_clone_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        repo: String,
        dest: String,
    ) -> JoinHandle<()> {
        self.spawn_git_clone_task_with_opts(app, id, token, repo, dest, None, None, None)
    }
}
