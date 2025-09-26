// Split transport module without changing public API or logic.
// Public API remains:
// - ensure_registered
// - maybe_rewrite_https_to_custom
// - set_push_auth_header_value

mod fallback;
mod fingerprint;
#[path = "http/mod.rs"]
mod http;
pub mod metrics; // made public for test helpers needing crate::core::git::transport::metrics::*
mod register;
mod rewrite;
mod runtime;

pub use fallback::{DecisionCtx, FallbackDecision, FallbackReason, FallbackStage};
pub use http::set_push_auth_header_value;
pub use metrics::{NoopCollector, TimingCapture, TimingRecorder, TransportMetricsCollector};
pub use register::ensure_registered;
pub use rewrite::maybe_rewrite_https_to_custom;
pub use runtime::{is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent};
// P3.2: expose selective metrics thread-local helpers for task registry emission
pub use fingerprint::record_certificate;
pub use metrics::{
    metrics_enabled, tl_push_fallback_event, tl_snapshot, tl_take_fallback_events,
    FallbackEventRecord,
};
#[cfg(test)]
pub use runtime::test_auto_disable_guard;

// Test-only helpers re-export for ease of access in tests modules
#[cfg(test)]
pub use http::{
    test_classify_and_count_fallback, test_reset_fallback_counters, test_snapshot_fallback_counters,
};
#[cfg(test)]
pub use runtime::test_reset_auto_disable;
