use super::super::TaskRegistry;
use crate::core::config::model::{AppConfig, RetryCfg};
use crate::core::git::default_impl::opts::{
    StrategyHttpOverride, StrategyRetryOverride, StrategyTlsOverride,
};
use crate::core::git::errors::{ErrorCategory, GitError};
use crate::core::tasks::model::TaskErrorEvent;
use crate::core::tasks::retry::{categorize, RetryPlan};
use crate::events::structured::{
    publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
};
use uuid::Uuid;

pub(super) fn cancellation_error(id: Uuid, kind: &'static str) -> TaskErrorEvent {
    TaskErrorEvent::from_parts(id, kind, ErrorCategory::Cancel, "user canceled", None)
}

pub(super) fn handle_cancel(
    registry: &TaskRegistry,
    app: &Option<crate::events::emitter::AppHandle>,
    id: &Uuid,
    kind: &'static str,
) {
    registry.emit_error_if_app(app, || cancellation_error(*id, kind));
    registry.mark_canceled(app, id);
}

pub(super) fn report_failure(
    registry: &TaskRegistry,
    app: &Option<crate::events::emitter::AppHandle>,
    id: &Uuid,
    kind: &'static str,
    error: &GitError,
    attempt: Option<u32>,
    fallback: &'static str,
) {
    let category = categorize(error);
    registry.emit_error_if_app(app, || {
        TaskErrorEvent::from_parts(*id, kind, category, format!("{}", error), attempt)
    });
    registry.mark_failed(app, id, fallback);
}

pub(super) fn runtime_config() -> AppConfig {
    let mut cfg =
        crate::core::config::loader::load_or_init().unwrap_or_else(|_| AppConfig::default());
    if let Ok(v) = std::env::var("FWC_PARTIAL_FILTER_SUPPORTED") {
        if v == "1" {
            cfg.partial_filter_supported = true;
        }
    }
    if let Ok(v) = std::env::var("FWC_PARTIAL_FILTER_CAPABLE") {
        if v == "1" {
            cfg.partial_filter_supported = true;
        }
    }
    cfg
}

pub(super) fn emit_strategy_summary(
    _app: &Option<crate::events::emitter::AppHandle>,
    id: uuid::Uuid,
    kind: &str,
    http: (bool, u8),
    retry: &RetryPlan,
    tls: (bool, bool),
    codes: Vec<String>,
    has_filter: bool,
) {
    publish_global(StructuredEvent::Strategy(
        StructuredStrategyEvent::Summary {
            id: id.to_string(),
            kind: kind.to_string(),
            http_follow: http.0,
            http_max: http.1,
            retry_max: retry.max,
            retry_base_ms: retry.base_ms,
            retry_factor: retry.factor,
            retry_jitter: retry.jitter,
            tls_insecure: tls.0,
            tls_skip_san: tls.1,
            applied_codes: codes,
            filter_requested: has_filter,
        },
    ));
}

impl TaskRegistry {
    pub(super) fn runtime_config() -> AppConfig {
        runtime_config()
    }

    pub(super) fn emit_strategy_summary(
        app: &Option<crate::events::emitter::AppHandle>,
        id: uuid::Uuid,
        kind: &str,
        http: (bool, u8),
        retry: &RetryPlan,
        tls: (bool, bool),
        codes: Vec<String>,
        has_filter: bool,
    ) {
        emit_strategy_summary(app, id, kind, http, retry, tls, codes, has_filter)
    }

    pub fn decide_partial_fallback(
        depth_applied: Option<u32>,
        filter_requested: Option<&str>,
        capability_supported: bool,
    ) -> Option<(String, bool)> {
        if filter_requested.is_none() {
            return None;
        }
        if capability_supported {
            return None;
        }
        let shallow = depth_applied.is_some();
        let msg = if shallow {
            "partial filter unsupported; fallback=shallow (depth retained)".to_string()
        } else {
            "partial filter unsupported; fallback=full".to_string()
        };
        Some((msg, shallow))
    }

    pub fn apply_http_override(
        kind: &str,
        id: &uuid::Uuid,
        global: &AppConfig,
        override_http: Option<&StrategyHttpOverride>,
    ) -> (bool, u8, bool, Option<String>) {
        let mut follow = global.http.follow_redirects;
        let mut max_r = global.http.max_redirects;
        let mut changed = false;
        let mut conflict: Option<String> = None;
        if let Some(o) = override_http {
            if let Some(f) = o.follow_redirects {
                if f != follow {
                    follow = f;
                    changed = true;
                }
            }
            if let Some(m) = o.max_redirects {
                let m_clamped = m.min(20) as u8;
                if m_clamped != max_r {
                    max_r = m_clamped;
                    changed = true;
                }
            }
            if !follow && max_r > 0 {
                conflict = Some(format!(
                    "followRedirects=false => force maxRedirects=0 (was {})",
                    max_r
                ));
                if max_r != 0 {
                    max_r = 0;
                    changed = true;
                }
            }
        }
        if changed {
            tracing::info!(
                target = "strategy",
                task_kind = %kind,
                task_id = %id,
                follow_redirects = %follow,
                max_redirects = %max_r,
                "http override applied"
            );
        }
        (follow, max_r, changed, conflict)
    }

    pub fn apply_retry_override(
        global: &RetryCfg,
        override_retry: Option<&StrategyRetryOverride>,
    ) -> (RetryPlan, bool) {
        let mut plan: RetryPlan = global.clone().into();
        let mut changed = false;
        if let Some(o) = override_retry {
            if let Some(m) = o.max {
                if m != plan.max {
                    plan.max = m;
                    changed = true;
                }
            }
            if let Some(b) = o.base_ms {
                if b as u64 != plan.base_ms {
                    plan.base_ms = b as u64;
                    changed = true;
                }
            }
            if let Some(f) = o.factor {
                if (f as f64) != plan.factor {
                    plan.factor = f as f64;
                    changed = true;
                }
            }
            if let Some(j) = o.jitter {
                if j != plan.jitter {
                    plan.jitter = j;
                    changed = true;
                }
            }
        }
        if changed {
            tracing::info!(
                target = "strategy",
                retry_max = plan.max,
                retry_base_ms = plan.base_ms,
                retry_factor = plan.factor,
                retry_jitter = plan.jitter,
                "retry override applied"
            );
        }
        (plan, changed)
    }

    pub fn apply_tls_override(
        kind: &str,
        id: &uuid::Uuid,
        global: &AppConfig,
        override_tls: Option<&StrategyTlsOverride>,
    ) -> (bool, bool, bool, Option<String>) {
        let mut insecure = global.tls.insecure_skip_verify;
        let mut skip_san = global.tls.skip_san_whitelist;
        let mut changed = false;
        let mut conflict: Option<String> = None;
        if let Some(o) = override_tls {
            if let Some(v) = o.insecure_skip_verify {
                if v != insecure {
                    insecure = v;
                    changed = true;
                }
            }
            if let Some(v) = o.skip_san_whitelist {
                if v != skip_san {
                    skip_san = v;
                    changed = true;
                }
            }
            if insecure && skip_san {
                conflict =
                    Some("insecureSkipVerify=true normalizes skipSanWhitelist=false".to_string());
                if skip_san {
                    skip_san = false;
                    changed = true;
                }
            }
        }
        if changed {
            tracing::info!(
                target = "strategy",
                task_kind = %kind,
                task_id = %id,
                insecure_skip_verify = %insecure,
                skip_san_whitelist = %skip_san,
                "tls override applied"
            );
        }
        (insecure, skip_san, changed, conflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn no_http_override() {
        let global = AppConfig::default();
        let (f, m, changed, conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, None);
        assert_eq!(f, global.http.follow_redirects);
        assert_eq!(m, global.http.max_redirects);
        assert!(!changed);
        assert!(conflict.is_none());
    }

    #[test]
    fn http_override_changes() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride {
            follow_redirects: Some(!global.http.follow_redirects),
            max_redirects: Some(3),
            ..Default::default()
        };
        let (f, m, changed, conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        if !global.http.follow_redirects {
            assert_eq!(f, true);
        }
        if f == false {
            assert_eq!(m, 0);
            assert!(conflict.is_some());
        } else {
            assert_eq!(m, 3);
            assert!(conflict.is_none());
        }
        assert!(changed);
    }

    #[test]
    fn http_override_clamp_applies() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride {
            follow_redirects: None,
            max_redirects: Some(99),
            ..Default::default()
        };
        let (_f, m, changed, _conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(m, 20);
        assert!(changed);
    }

    #[test]
    fn no_retry_override() {
        let global = RetryCfg::default();
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, None);
        assert_eq!(plan.max, global.max);
        assert_eq!(plan.base_ms, global.base_ms);
        assert!(!changed);
    }

    #[test]
    fn retry_override_changes() {
        let mut global = RetryCfg::default();
        global.max = 6;
        let over = StrategyRetryOverride {
            max: Some(3),
            base_ms: Some(500),
            factor: Some(2.0),
            jitter: Some(false),
        };
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
        assert!(changed);
        assert_eq!(plan.max, 3);
        assert_eq!(plan.base_ms, 500);
        assert!((plan.factor - 2.0).abs() < f64::EPSILON);
        assert!(!plan.jitter);
    }

    #[test]
    fn no_tls_override() {
        let global = AppConfig::default();
        let (ins, skip, changed, conflict) =
            TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, None);
        assert_eq!(ins, global.tls.insecure_skip_verify);
        assert_eq!(skip, global.tls.skip_san_whitelist);
        assert!(!changed);
        assert!(conflict.is_none());
    }

    #[test]
    fn tls_override_insecure_only() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride {
            insecure_skip_verify: Some(!global.tls.insecure_skip_verify),
            skip_san_whitelist: None,
        };
        let (ins, skip, changed, conflict) =
            TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(ins, !global.tls.insecure_skip_verify);
        assert_eq!(skip, global.tls.skip_san_whitelist);
        assert!(changed);
        assert!(conflict.is_none());
    }

    #[test]
    fn tls_override_skip_san_only() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride {
            insecure_skip_verify: None,
            skip_san_whitelist: Some(!global.tls.skip_san_whitelist),
        };
        let (ins, skip, changed, conflict) =
            TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(ins, global.tls.insecure_skip_verify);
        assert_eq!(skip, !global.tls.skip_san_whitelist);
        assert!(changed);
        assert!(conflict.is_none());
    }

    #[test]
    fn tls_override_both_changed() {
        let mut global = AppConfig::default();
        global.tls.insecure_skip_verify = false;
        global.tls.skip_san_whitelist = false;
        let over = StrategyTlsOverride {
            insecure_skip_verify: Some(true),
            skip_san_whitelist: Some(true),
        };
        let (ins, skip, changed, conflict) =
            TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert!(changed);
        assert!(ins);
        assert!(!skip);
        assert!(conflict.is_some());
    }

    #[test]
    fn tls_global_config_not_mutated() {
        let global = AppConfig::default();
        let over = StrategyTlsOverride {
            insecure_skip_verify: Some(true),
            skip_san_whitelist: Some(true),
        };
        let _ = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert!(!global.tls.insecure_skip_verify);
        assert!(!global.tls.skip_san_whitelist);
    }
}
