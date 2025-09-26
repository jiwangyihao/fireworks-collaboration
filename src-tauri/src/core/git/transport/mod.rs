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

pub use fallback::{DecisionCtx, FallbackDecision, FallbackReason, FallbackStage};
pub use http::set_push_auth_header_value;
pub use metrics::{NoopCollector, TimingCapture, TimingRecorder, TransportMetricsCollector};
pub use register::ensure_registered;
pub use rewrite::maybe_rewrite_https_to_custom;
// P3.2: expose selective metrics thread-local helpers for task registry emission
pub use fingerprint::record_certificate;
pub use metrics::{metrics_enabled, tl_snapshot};

// Test-only helpers re-export for ease of access in tests modules
#[cfg(test)]
pub use http::{
    test_classify_and_count_fallback, test_reset_fallback_counters, test_snapshot_fallback_counters,
};
