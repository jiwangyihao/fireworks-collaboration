// Split transport module without changing public API or logic.
// Public API remains:
// - ensure_registered
// - maybe_rewrite_https_to_custom
// - set_push_auth_header_value

#[path = "http/mod.rs"]
mod http;
mod fallback;
pub mod metrics; // made public for test helpers needing crate::core::git::transport::metrics::*
mod fingerprint;
mod register;
mod rewrite;

pub use http::set_push_auth_header_value;
pub use register::ensure_registered;
pub use rewrite::maybe_rewrite_https_to_custom;
pub use fallback::{FallbackDecision, FallbackStage, FallbackReason, DecisionCtx};
pub use metrics::{TimingRecorder, TimingCapture, TransportMetricsCollector, NoopCollector};
// P3.2: expose selective metrics thread-local helpers for task registry emission
pub use metrics::{tl_snapshot, metrics_enabled};
pub use fingerprint::record_certificate;
