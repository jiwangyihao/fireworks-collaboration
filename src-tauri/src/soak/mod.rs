//! Soak testing module for adaptive TLS and git operations.
//!
//! This module provides comprehensive soak testing capabilities to validate
//! the stability and performance of adaptive TLS features, IP pool management,
//! proxy handling, and git operations under sustained load.
//!
//! ## Module Structure
//!
//! - `models`: Data structures for configuration, reporting, and thresholds
//! - `aggregator`: Event processing and metrics aggregation
//! - `runner`: Main soak test execution logic
//! - `tasks`: Git task execution helpers (push, fetch, clone)
//! - `utils`: Utility functions for statistics and I/O operations
//!
//! ## Usage
//!
//! Run from environment variables:
//! ```no_run
//! use fireworks_collaboration_lib::soak;
//! let report = soak::run_from_env().unwrap();
//! ```
//!
//! Or run with explicit options:
//! ```no_run
//! use fireworks_collaboration_lib::soak::{SoakOptions, run};
//! let opts = SoakOptions {
//!     iterations: 20,
//!     ..Default::default()
//! };
//! let report = run(opts).unwrap();
//! ```

mod aggregator;
mod models;
mod runner;
mod tasks;
mod utils;

// Re-export public API
pub use aggregator::{IpPoolStats, ProxyStats, SoakAggregator};
pub use models::{
    build_comparison_summary, AlertsSummary, AutoDisableSummary, ComparisonSummary,
    FallbackSummary, FieldStats, IpPoolSummary, OperationSummary, ProxySummary, SoakOptions,
    SoakOptionsSnapshot, SoakReport, SoakThresholds, ThresholdCheck, ThresholdSummary,
    TimingSummary, TotalsSummary,
};
pub use runner::{run, run_from_env};
