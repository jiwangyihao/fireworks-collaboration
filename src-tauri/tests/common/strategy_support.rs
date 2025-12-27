//! `strategy_support`: helper utilities for emitting strategy-related structured events during tests.
//! Migrated from `core.registry.git.test_support` to keep production crate free of test-only helpers.
#![allow(dead_code)]

use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::default_impl::opts::parse_depth_filter_opts;
use fireworks_collaboration_lib::core::git::transport;
use fireworks_collaboration_lib::core::git::transport::metrics::{
    metrics_enabled, tl_set_fallback_stage, tl_set_timing, tl_set_used_fake,
    tl_take_fallback_events, FallbackEventRecord, TimingCapture,
};

use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::retry::{
    compute_retry_diff, load_retry_plan, RetryPlan,
};

use fireworks_collaboration_lib::events::structured::{
    publish_global, set_global_event_bus, Event as StructuredEvent, MemoryEventBus,
    StrategyEvent as StructuredStrategyEvent,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn config_override_cell() -> &'static Mutex<Option<PathBuf>> {
    static CELL: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

fn current_config_override() -> Option<PathBuf> {
    config_override_cell().lock().unwrap().clone()
}

#[cfg(test)]
fn set_test_config_override(base: Option<PathBuf>) {
    *config_override_cell().lock().unwrap() = base;
}

fn runtime_config() -> AppConfig {
    let mut cfg = if let Some(base) = current_config_override() {
        fireworks_collaboration_lib::core::config::loader::load_or_init_at(base.as_path())
            .unwrap_or_else(|_| AppConfig::default())
    } else {
        fireworks_collaboration_lib::core::config::loader::load_or_init()
            .unwrap_or_else(|_| AppConfig::default())
    };
    if matches!(
        std::env::var("FWC_PARTIAL_FILTER_SUPPORTED").as_deref(),
        Ok("1")
    ) {
        cfg.partial_filter_supported = true;
    }
    if matches!(
        std::env::var("FWC_PARTIAL_FILTER_CAPABLE").as_deref(),
        Ok("1")
    ) {
        cfg.partial_filter_supported = true;
    }
    cfg
}

fn publish_strategy_summary(
    task_id: uuid::Uuid,
    kind: &str,
    http_follow: bool,
    http_max: u8,
    retry_plan: &RetryPlan,
    applied_codes: Vec<String>,
    filter_requested: bool,
) {
    publish_global(StructuredEvent::Strategy(
        StructuredStrategyEvent::Summary {
            id: task_id.to_string(),
            kind: kind.to_string(),
            http_follow,
            http_max,
            retry_max: retry_plan.max,
            retry_base_ms: retry_plan.base_ms,
            retry_factor: retry_plan.factor,
            retry_jitter: retry_plan.jitter,
            applied_codes,
            filter_requested,
        },
    ));
}

pub fn test_emit_clone_strategy_and_rollout(repo: &str, task_id: uuid::Uuid) {
    // let _app = AppHandle; // Removed invalid construction
    let global_cfg = runtime_config();
    let decision = transport::decide_https_to_custom(&global_cfg, repo);
    if decision.eligible {
        publish_global(StructuredEvent::Strategy(
            StructuredStrategyEvent::AdaptiveTlsRollout {
                id: task_id.to_string(),
                kind: "GitClone".into(),
                percent_applied: global_cfg.http.fake_sni_rollout_percent as u8,
                sampled: decision.sampled,
            },
        ));
    }
}

pub fn test_emit_clone_with_override(
    _repo: &str,
    task_id: uuid::Uuid,
    mut strategy_override: Value,
) {
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let global_cfg = runtime_config();
    if let Some(inner) = strategy_override.get("strategyOverride") {
        strategy_override = inner.clone();
    }
    let parsed_opts =
        parse_depth_filter_opts(None, None, Some(strategy_override)).expect("parse override");
    let mut applied_codes: Vec<String> = vec![];
    let mut effective_follow = global_cfg.http.follow_redirects;
    let mut effective_max = global_cfg.http.max_redirects;
    let mut retry_plan: RetryPlan = global_cfg.retry.clone().into();
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
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::Conflict {
                    id: task_id.to_string(),
                    kind: "http".into(),
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
            let base_plan = load_retry_plan();
            let (diff, _) = compute_retry_diff(&base_plan, &retry_plan);
            publish_global(StructuredEvent::Policy(
                fireworks_collaboration_lib::events::structured::PolicyEvent::RetryApplied {
                    id: task_id.to_string(),
                    code: "retry_strategy_override_applied".to_string(),
                    changed: diff.changed.into_iter().map(|s| s.to_string()).collect(),
                },
            ));
            applied_codes.push("retry_strategy_override_applied".into());
        }
    }
    let applied_codes_clone = applied_codes.clone();
    publish_strategy_summary(
        task_id,
        "GitClone",
        effective_follow,
        effective_max,
        &retry_plan,
        applied_codes_clone,
        false,
    );
    publish_strategy_summary(
        task_id,
        "GitClone",
        effective_follow,
        effective_max,
        &retry_plan,
        applied_codes,
        false,
    );
}

pub fn test_emit_adaptive_tls_observability(task_id: uuid::Uuid, kind: &str) {
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
                ip_source: None,
                ip_latency_ms: None,
                ip_selection_stage: None,
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
                        ip_source: None,
                        ip_latency_ms: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use fireworks_collaboration_lib::core::config::{loader, model::AppConfig};
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;
    use uuid::Uuid;

    const FORCE_DISABLE_ENV: &str = "FWC_PROXY_FORCE_DISABLE";

    struct ProxyEnvGuard {
        restored: Vec<(&'static str, String)>,
        force_disable_original: Option<String>,
    }

    impl ProxyEnvGuard {
        fn new() -> Self {
            let mut restored = Vec::new();
            for key in [
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
                "ALL_PROXY",
                "all_proxy",
            ] {
                if let Ok(value) = std::env::var(key) {
                    restored.push((key, value));
                    std::env::remove_var(key);
                }
            }
            let force_disable_original = std::env::var(FORCE_DISABLE_ENV).ok();
            std::env::set_var(FORCE_DISABLE_ENV, "1");
            Self {
                restored,
                force_disable_original,
            }
        }
    }

    impl Drop for ProxyEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.restored {
                std::env::set_var(*key, value);
            }
            match &self.force_disable_original {
                Some(value) => std::env::set_var(FORCE_DISABLE_ENV, value),
                None => std::env::remove_var(FORCE_DISABLE_ENV),
            }
        }
    }

    fn config_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn setup_config(percent: u8) -> tempfile::TempDir {
        let dir = tempdir().expect("temp config dir");
        set_test_config_override(Some(dir.path().to_path_buf()));
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = percent;
        loader::save_at(&cfg, dir.path()).expect("save config");
        dir
    }

    fn collect_rollout(percent: u8) -> bool {
        let _proxy_guard = ProxyEnvGuard::new();
        // 清理可能由其他并行测试留下的事件总线状态
        clear_test_event_bus();

        let temp = setup_config(percent);
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        // 设置测试事件总线（publish_global 会优先使用 TEST_OVERRIDE_BUS）
        set_test_event_bus(bus.clone());

        let repo = "https://github.com/owner/repo";
        let id = Uuid::new_v4();
        let expected_id = id.to_string();
        test_emit_clone_strategy_and_rollout(repo, id);
        let events = bus.take_all();
        clear_test_event_bus();
        set_test_config_override(None);
        drop(temp);
        events
            .into_iter()
            .find_map(|evt| match evt {
                Event::Strategy(StrategyEvent::AdaptiveTlsRollout { id, sampled, .. })
                    if id == expected_id =>
                {
                    Some(sampled)
                }
                _ => None,
            })
            .expect("rollout event")
    }

    #[test]
    fn rollout_event_reflects_sampled_true_when_percent_100() {
        let _guard = config_lock().lock().unwrap();
        assert!(collect_rollout(100));
    }

    #[test]
    fn rollout_event_reflects_sampled_false_when_percent_zero() {
        let _guard = config_lock().lock().unwrap();
        assert!(!collect_rollout(0));
    }
}
