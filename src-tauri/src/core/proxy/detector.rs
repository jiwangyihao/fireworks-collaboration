//! Proxy Failure Detector
//!
//! Implements sliding window-based failure detection for proxy connections.
//! Triggers automatic fallback to direct connection when failure rate exceeds threshold.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single connection attempt record
#[derive(Debug, Clone)]
struct ConnectionAttempt {
    /// Timestamp in Unix seconds
    timestamp: u64,
    /// Whether the attempt was successful
    success: bool,
}

/// Inner state of the failure detector (protected by Mutex)
#[derive(Debug)]
struct FailureDetectorInner {
    /// Sliding window of recent connection attempts
    attempts: Vec<ConnectionAttempt>,
    /// Window duration in seconds
    window_seconds: u64,
    /// Failure rate threshold (0.0 to 1.0)
    threshold: f64,
    /// Whether fallback has been triggered
    fallback_triggered: bool,
}

impl FailureDetectorInner {
    /// Remove attempts outside the current window
    fn prune_old_attempts(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_seconds);
        self.attempts.retain(|attempt| attempt.timestamp >= cutoff);
    }

    /// Calculate current failure rate
    fn calculate_failure_rate(&self) -> f64 {
        if self.attempts.is_empty() {
            return 0.0;
        }

        let failures = self.attempts.iter().filter(|a| !a.success).count();
        failures as f64 / self.attempts.len() as f64
    }

    /// Check if failure rate exceeds threshold
    fn should_trigger_fallback(&self) -> bool {
        !self.fallback_triggered && self.calculate_failure_rate() >= self.threshold
    }
}

/// Proxy failure detector using sliding window statistics
///
/// # Example
/// ```
/// use fireworks_collaboration::core::proxy::ProxyFailureDetector;
///
/// let detector = ProxyFailureDetector::new(300, 0.2); // 5 min window, 20% threshold
///
/// // Report connection attempts
/// detector.report_failure();
/// detector.report_success();
///
/// // Check if fallback should be triggered
/// if detector.should_fallback() {
///     println!("Too many failures, falling back to direct connection");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ProxyFailureDetector {
    inner: Arc<Mutex<FailureDetectorInner>>,
}

impl ProxyFailureDetector {
    /// Create a new failure detector
    ///
    /// # Arguments
    /// * `window_seconds` - Sliding window duration (default: 300 = 5 minutes)
    /// * `threshold` - Failure rate threshold 0.0-1.0 (default: 0.2 = 20%)
    ///
    /// # Validation
    /// - `window_seconds` must be > 0 (falls back to 60 if 0)
    /// - `threshold` is clamped to [0.0, 1.0]
    pub fn new(window_seconds: u64, threshold: f64) -> Self {
        // Validate and correct window_seconds
        let validated_window = if window_seconds == 0 {
            tracing::warn!(
                "Invalid window_seconds=0, using default 60 seconds"
            );
            60
        } else {
            window_seconds
        };

        // Clamp threshold and warn if out of range
        let validated_threshold = if threshold.is_nan() {
            tracing::warn!("Threshold is NaN, using default 0.0");
            0.0
        } else {
            let clamped = threshold.clamp(0.0, 1.0);
            if threshold != clamped {
                tracing::warn!(
                    "Threshold {} out of range [0.0, 1.0], clamped to {}",
                    threshold,
                    clamped
                );
            }
            clamped
        };

        tracing::debug!(
            "Creating ProxyFailureDetector: window={}s, threshold={:.1}%",
            validated_window,
            validated_threshold * 100.0
        );

        Self {
            inner: Arc::new(Mutex::new(FailureDetectorInner {
                attempts: Vec::new(),
                window_seconds: validated_window,
                threshold: validated_threshold,
                fallback_triggered: false,
            })),
        }
    }

    /// Create detector with default settings (5 min window, 20% threshold)
    pub fn default() -> Self {
        Self::new(300, 0.2)
    }

    /// Report a failed connection attempt
    pub fn report_failure(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut inner = self.inner.lock().unwrap();
        inner.prune_old_attempts(now);
        inner.attempts.push(ConnectionAttempt {
            timestamp: now,
            success: false,
        });

        let failure_rate = inner.calculate_failure_rate();
        tracing::debug!(
            "Proxy failure reported: total_attempts={}, failures={}, rate={:.1}%",
            inner.attempts.len(),
            inner.attempts.iter().filter(|a| !a.success).count(),
            failure_rate * 100.0
        );
    }

    /// Report a successful connection attempt
    pub fn report_success(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut inner = self.inner.lock().unwrap();
        inner.prune_old_attempts(now);
        inner.attempts.push(ConnectionAttempt {
            timestamp: now,
            success: true,
        });

        let failure_rate = inner.calculate_failure_rate();
        tracing::debug!(
            "Proxy success reported: total_attempts={}, failures={}, rate={:.1}%",
            inner.attempts.len(),
            inner.attempts.iter().filter(|a| !a.success).count(),
            failure_rate * 100.0
        );
    }

    /// Check if fallback should be triggered
    ///
    /// Returns true if failure rate >= threshold and fallback not yet triggered
    pub fn should_fallback(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        let should_trigger = inner.should_trigger_fallback();
        
        if should_trigger {
            let failure_rate = inner.calculate_failure_rate();
            tracing::warn!(
                "Fallback threshold exceeded: rate={:.1}% >= threshold={:.1}%",
                failure_rate * 100.0,
                inner.threshold * 100.0
            );
        }
        
        should_trigger
    }

    /// Mark fallback as triggered (prevents repeated triggers)
    ///
    /// This should be called after `should_fallback()` returns true and
    /// the fallback action has been initiated. Prevents the detector from
    /// repeatedly triggering fallback for the same failure condition.
    ///
    /// # Example
    /// ```no_run
    /// # use fireworks_collaboration::core::proxy::ProxyFailureDetector;
    /// # let detector = ProxyFailureDetector::new(300, 0.2);
    /// if detector.should_fallback() {
    ///     // Initiate fallback to direct connection
    ///     detector.mark_fallback_triggered();
    ///     println!("Fallback triggered");
    /// }
    /// ```
    pub fn mark_fallback_triggered(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.fallback_triggered = true;
        tracing::debug!("Fallback marked as triggered");
    }

    /// Reset the detector state (for recovery)
    ///
    /// Clears all connection attempts and resets the fallback trigger flag.
    /// This should be called when the proxy is manually recovered or when
    /// automatic recovery is confirmed successful (P5.5).
    ///
    /// # Example
    /// ```no_run
    /// # use fireworks_collaboration::core::proxy::ProxyFailureDetector;
    /// # let detector = ProxyFailureDetector::new(300, 0.2);
    /// // After successful recovery
    /// detector.reset();
    /// println!("Detector reset for fresh start");
    /// ```
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.attempts.clear();
        inner.fallback_triggered = false;
        tracing::info!("Failure detector reset");
    }

    /// Get current failure statistics
    ///
    /// Returns a snapshot of the current sliding window statistics.
    /// Old attempts outside the window are automatically pruned before
    /// calculating statistics.
    ///
    /// # Returns
    /// `FailureStats` containing:
    /// - `total_attempts`: Number of attempts in current window
    /// - `failures`: Number of failed attempts
    /// - `failure_rate`: Current failure rate (0.0 to 1.0)
    /// - `window_seconds`: Sliding window duration
    /// - `threshold`: Configured failure rate threshold
    /// - `fallback_triggered`: Whether fallback has been triggered
    ///
    /// # Example
    /// ```no_run
    /// # use fireworks_collaboration::core::proxy::ProxyFailureDetector;
    /// # let detector = ProxyFailureDetector::new(300, 0.2);
    /// let stats = detector.get_stats();
    /// println!("Failure rate: {:.1}%", stats.failure_rate * 100.0);
    /// println!("Attempts: {}/{}", stats.failures, stats.total_attempts);
    /// ```
    pub fn get_stats(&self) -> FailureStats {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut inner = self.inner.lock().unwrap();
        inner.prune_old_attempts(now);

        let total_attempts = inner.attempts.len();
        let failures = inner.attempts.iter().filter(|a| !a.success).count();
        let failure_rate = inner.calculate_failure_rate();

        FailureStats {
            total_attempts,
            failures,
            failure_rate,
            window_seconds: inner.window_seconds,
            threshold: inner.threshold,
            fallback_triggered: inner.fallback_triggered,
        }
    }
}

/// Failure statistics snapshot
#[derive(Debug, Clone)]
pub struct FailureStats {
    /// Total attempts in current window
    pub total_attempts: usize,
    /// Number of failures in current window
    pub failures: usize,
    /// Current failure rate (0.0 to 1.0)
    pub failure_rate: f64,
    /// Window duration in seconds
    pub window_seconds: u64,
    /// Failure rate threshold
    pub threshold: f64,
    /// Whether fallback has been triggered
    pub fallback_triggered: bool,
}
