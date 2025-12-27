// Transport layer for git operations.
// HTTP-specific transport has been extracted to http_transport module to reduce nesting.
// Public API:
// - ensure_registered
// - maybe_rewrite_https_to_custom
// - set_push_auth_header_value (re-exported from http_transport)

mod fallback;
pub mod fingerprint; // made public for testing
pub mod metrics; // made public for test helpers needing crate::core::git::transport::metrics::*
mod register;
pub mod rewrite; // made public for testing
pub mod runtime; // made public for testing

pub use fallback::{DecisionCtx, FallbackDecision, FallbackReason, FallbackStage};
pub use metrics::{NoopCollector, TimingCapture, TimingRecorder, TransportMetricsCollector};
pub use register::ensure_registered;
pub use rewrite::{decide_https_to_custom, maybe_rewrite_https_to_custom, RewriteDecision};
pub use runtime::{is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent};
// Re-export from http_transport
pub use crate::core::git::http_transport::set_push_auth_header_value;
// P3.2: expose selective metrics thread-local helpers for task registry emission
pub use fingerprint::record_certificate;
pub use metrics::{
    metrics_enabled, tl_push_fallback_event, tl_snapshot, tl_take_fallback_events,
    FallbackEventRecord,
};

pub mod testing {
    //! Aggregated transport testing helpers available to integration tests.
    pub use super::runtime::testing::{auto_disable_guard, reset_auto_disable};
    pub use crate::core::git::http_transport::testing::{
        classify_and_count_fallback, inject_fake_failure, inject_real_failure,
        reset_fallback_counters, reset_injected_failures, snapshot_fallback_counters,
        TestSubtransport,
    };
}
