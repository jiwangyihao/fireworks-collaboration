use crate::events::emitter::{emit_all, AppHandle};
use crate::events::structured::{
    publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
};

use super::super::base::{TaskRegistry, EV_ERROR};
use crate::core::tasks::model::TaskErrorEvent;

pub fn test_emit_clone_strategy_and_rollout(repo: &str, task_id: uuid::Uuid) {
    let _app = AppHandle;
    let global_cfg = TaskRegistry::runtime_config();
    if let Some(_rewritten) =
        crate::core::git::transport::maybe_rewrite_https_to_custom(&global_cfg, repo)
    {
        publish_global(StructuredEvent::Strategy(
            StructuredStrategyEvent::AdaptiveTlsRollout {
                id: task_id.to_string(),
                kind: "GitClone".into(),
                percent_applied: global_cfg.http.fake_sni_rollout_percent as u8,
                sampled: true,
            },
        ));
    }
}

pub fn test_emit_clone_with_override(
    _repo: &str,
    task_id: uuid::Uuid,
    mut strategy_override: serde_json::Value,
) {
    use crate::events::structured::{set_global_event_bus, MemoryEventBus};

    let app = AppHandle;
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let global_cfg = TaskRegistry::runtime_config();
    if let Some(inner) = strategy_override.get("strategyOverride") {
        strategy_override = inner.clone();
    }
    let parsed_opts = crate::core::git::default_impl::opts::parse_depth_filter_opts(
        None,
        None,
        Some(strategy_override),
    )
    .expect("parse override");
    let mut applied_codes: Vec<String> = vec![];
    let mut effective_follow = global_cfg.http.follow_redirects;
    let mut effective_max = global_cfg.http.max_redirects;
    let mut retry_plan: crate::core::tasks::retry::RetryPlan = global_cfg.retry.clone().into();
    let mut effective_insecure = global_cfg.tls.insecure_skip_verify;
    let mut effective_skip = global_cfg.tls.skip_san_whitelist;
    if let Some(http_over) = parsed_opts
        .strategy_override
        .as_ref()
        .and_then(|s| s.http.as_ref())
    {
        let (f, m, changed, conflict) =
            TaskRegistry::apply_http_override("GitClone", &task_id, &global_cfg, Some(http_over));
        effective_follow = f;
        effective_max = m;
        if changed {
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::HttpApplied {
                    id: task_id.to_string(),
                    follow: f,
                    max_redirects: m,
                },
            ));
            applied_codes.push("http_strategy_override_applied".into());
        }
        if let Some(msg) = conflict {
            let evt = TaskErrorEvent {
                task_id,
                kind: "GitClone".into(),
                category: "Protocol".into(),
                code: Some("strategy_override_conflict".into()),
                message: format!("http conflict: {}", msg),
                retried_times: None,
            };
            emit_all(&app, EV_ERROR, &evt);
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::Conflict {
                    id: task_id.to_string(),
                    kind: "http".into(),
                    message: msg,
                },
            ));
        }
    }
    if let Some(tls_over) = parsed_opts
        .strategy_override
        .as_ref()
        .and_then(|s| s.tls.as_ref())
    {
        let (ins, skip, changed, conflict) =
            TaskRegistry::apply_tls_override("GitClone", &task_id, &global_cfg, Some(tls_over));
        effective_insecure = ins;
        effective_skip = skip;
        if changed {
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::TlsApplied {
                    id: task_id.to_string(),
                    insecure_skip_verify: ins,
                    skip_san_whitelist: skip,
                },
            ));
            applied_codes.push("tls_strategy_override_applied".into());
        }
        if let Some(msg) = conflict {
            let evt = TaskErrorEvent {
                task_id,
                kind: "GitClone".into(),
                category: "Protocol".into(),
                code: Some("strategy_override_conflict".into()),
                message: format!("tls conflict: {}", msg),
                retried_times: None,
            };
            emit_all(&app, EV_ERROR, &evt);
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::Conflict {
                    id: task_id.to_string(),
                    kind: "tls".into(),
                    message: msg,
                },
            ));
        }
    }
    if let Some(retry_over) = parsed_opts
        .strategy_override
        .as_ref()
        .and_then(|s| s.retry.as_ref())
    {
        let (plan, changed) =
            TaskRegistry::apply_retry_override(&global_cfg.retry, Some(retry_over));
        retry_plan = plan;
        if changed {
            let base_plan = crate::core::tasks::retry::load_retry_plan();
            let (diff, _) = crate::core::tasks::retry::compute_retry_diff(&base_plan, &retry_plan);
            publish_global(crate::events::structured::Event::Policy(
                crate::events::structured::PolicyEvent::RetryApplied {
                    id: task_id.to_string(),
                    code: "retry_strategy_override_applied".to_string(),
                    changed: diff.changed.into_iter().map(|s| s.to_string()).collect(),
                },
            ));
            applied_codes.push("retry_strategy_override_applied".into());
        }
    }
    let applied_codes_clone = applied_codes.clone();
    publish_global(StructuredEvent::Strategy(
        StructuredStrategyEvent::Summary {
            id: task_id.to_string(),
            kind: "GitClone".into(),
            http_follow: effective_follow,
            http_max: effective_max,
            retry_max: retry_plan.max,
            retry_base_ms: retry_plan.base_ms,
            retry_factor: retry_plan.factor,
            retry_jitter: retry_plan.jitter,
            tls_insecure: effective_insecure,
            tls_skip_san: effective_skip,
            applied_codes: applied_codes_clone.clone(),
            filter_requested: false,
        },
    ));
    TaskRegistry::emit_strategy_summary(
        &Some(app.clone()),
        task_id,
        "GitClone",
        (effective_follow, effective_max),
        &retry_plan,
        (effective_insecure, effective_skip),
        applied_codes,
        false,
    );
}

pub fn test_emit_adaptive_tls_observability(task_id: uuid::Uuid, kind: &str) {
    use crate::core::git::transport::metrics::{
        metrics_enabled, tl_set_fallback_stage, tl_set_timing, tl_set_used_fake,
        tl_take_fallback_events, FallbackEventRecord, TimingCapture,
    };
    use crate::events::structured::{
        publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
    };
    let fallback_events = tl_take_fallback_events();
    if metrics_enabled() {
        tl_set_used_fake(true);
        tl_set_fallback_stage("Fake");
        let cap = TimingCapture {
            connect_ms: Some(10),
            tls_ms: Some(30),
            first_byte_ms: Some(40),
            total_ms: Some(50),
        };
        tl_set_timing(&cap);
        publish_global(StructuredEvent::Strategy(
            StructuredStrategyEvent::AdaptiveTlsTiming {
                id: task_id.to_string(),
                kind: kind.to_string(),
                used_fake_sni: true,
                fallback_stage: "Fake".into(),
                connect_ms: cap.connect_ms,
                tls_ms: cap.tls_ms,
                first_byte_ms: cap.first_byte_ms,
                total_ms: cap.total_ms,
                cert_fp_changed: false,
            },
        ));
    }
    for evt in fallback_events {
        match evt {
            FallbackEventRecord::Transition { from, to, reason } => {
                publish_global(StructuredEvent::Strategy(
                    StructuredStrategyEvent::AdaptiveTlsFallback {
                        id: task_id.to_string(),
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
            } => publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::AdaptiveTlsAutoDisable {
                    id: task_id.to_string(),
                    kind: kind.to_string(),
                    enabled,
                    threshold_pct,
                    cooldown_secs,
                },
            )),
        }
    }
}
