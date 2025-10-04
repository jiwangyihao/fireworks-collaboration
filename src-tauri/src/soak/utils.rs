use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::{Builder, Runtime};

use super::models::{FieldStats, SoakReport};

/// Compute field statistics from a vector of u32 values.
pub fn compute_field_stats(values: &[u32]) -> Option<FieldStats> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let count = sorted.len();
    let min = sorted[0];
    let max = sorted[count - 1];
    let sum: u64 = sorted.iter().map(|&v| v as u64).sum();
    let avg = sum as f64 / count as f64;
    let p50 = percentile(&sorted, 0.5);
    let p95 = percentile(&sorted, 0.95);
    Some(FieldStats {
        count,
        min,
        max,
        avg,
        p50,
        p95,
    })
}

/// Calculate percentile from a sorted array.
pub fn percentile(sorted: &[u32], q: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let pos = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[pos.clamp(0, sorted.len() - 1)]
}

/// Build a multi-threaded Tokio runtime.
pub fn build_runtime() -> Result<Runtime> {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .map_err(|e| anyhow!(e))
}

/// Convert `SystemTime` to Unix timestamp in seconds.
pub fn system_time_to_unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

/// Parse environment variable as f64.
pub fn parse_env_f64(key: &str) -> Option<f64> {
    std::env::var(key).ok().and_then(|v| v.parse::<f64>().ok())
}

/// Parse environment variable as u64.
pub fn parse_env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok().and_then(|v| v.parse::<u64>().ok())
}

/// Setup git identity environment variables if not already set.
pub fn setup_git_identity() {
    if std::env::var("GIT_AUTHOR_NAME").is_err() {
        std::env::set_var("GIT_AUTHOR_NAME", "fwc-soak");
    }
    if std::env::var("GIT_AUTHOR_EMAIL").is_err() {
        std::env::set_var("GIT_AUTHOR_EMAIL", "fwc-soak@example.com");
    }
    if std::env::var("GIT_COMMITTER_NAME").is_err() {
        std::env::set_var("GIT_COMMITTER_NAME", "fwc-soak");
    }
    if std::env::var("GIT_COMMITTER_EMAIL").is_err() {
        std::env::set_var("GIT_COMMITTER_EMAIL", "fwc-soak@example.com");
    }
}

/// Generate a simple timestamp string (Unix seconds).
pub fn chrono_like_timestamp() -> String {
    let now = SystemTime::now();
    let secs = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    format!("{secs}")
}

/// Load baseline report from JSON file.
pub fn load_baseline_report(path: &Path) -> Result<SoakReport> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("read baseline report: {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("parse baseline report: {}", path.display()))
}

/// Write soak report to JSON file.
pub fn write_report(path: &Path, report: &SoakReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir: {}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json).with_context(|| format!("write report file: {}", path.display()))?;
    Ok(())
}
