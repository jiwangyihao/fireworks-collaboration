use super::super::TaskRegistry;
use crate::core::config::model::{AppConfig, RetryCfg};
use crate::core::git::default_impl::opts::{
    StrategyHttpOverride, StrategyRetryOverride,
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
        TaskErrorEvent::from_parts(*id, kind, category, format!("{error}"), attempt)
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
        codes: Vec<String>,
        has_filter: bool,
    ) {
        emit_strategy_summary(app, id, kind, http, retry, codes, has_filter)
    }

    pub fn decide_partial_fallback(
        depth_applied: Option<u32>,
        filter_requested: Option<&str>,
        capability_supported: bool,
    ) -> Option<(String, bool)> {
        filter_requested?;
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
                    "followRedirects=false => force maxRedirects=0 (was {max_r})"
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

}
