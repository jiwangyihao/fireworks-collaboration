//! Transport metrics scaffolding (P3.0).
//!
//! Intentionally minimal: collects coarse timing segments in-memory. No
//! background export or event emission yet – subsequent P3.x stages will hook
//! into this.
use std::cell::RefCell;
use std::mem;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::core::config::loader::load_or_init;
use crate::core::config::model::AppConfig;

/// Timing points we care about for adaptive TLS.
#[derive(Debug, Default, Clone, Copy)]
pub struct TimingCapture {
    pub connect_ms: Option<u32>,
    pub tls_ms: Option<u32>,
    pub first_byte_ms: Option<u32>,
    pub total_ms: Option<u32>,
}

impl TimingCapture {
    fn dur_to_ms(d: Duration) -> u32 {
        d.as_millis().min(u128::from(u32::MAX)) as u32
    }
}

/// Builder style recorder – not thread safe (owned per request/stream).
pub struct TimingRecorder {
    start: Instant,
    connect_start: Option<Instant>,
    tls_start: Option<Instant>,
    first_byte_at: Option<Instant>,
    finished: bool,
    pub capture: TimingCapture,
}

impl TimingRecorder {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            connect_start: None,
            tls_start: None,
            first_byte_at: None,
            finished: false,
            capture: TimingCapture::default(),
        }
    }
    pub fn mark_connect_start(&mut self) {
        self.connect_start = Some(Instant::now());
    }
    pub fn mark_connect_end(&mut self) {
        if let Some(s) = self.connect_start {
            self.capture.connect_ms = Some(TimingCapture::dur_to_ms(s.elapsed()));
        }
    }
    pub fn mark_tls_start(&mut self) {
        self.tls_start = Some(Instant::now());
    }
    pub fn mark_tls_end(&mut self) {
        if let Some(s) = self.tls_start {
            self.capture.tls_ms = Some(TimingCapture::dur_to_ms(s.elapsed()));
        }
    }
    pub fn mark_first_byte(&mut self) {
        if self.first_byte_at.is_none() {
            self.first_byte_at = Some(Instant::now());
            self.capture.first_byte_ms = Some(TimingCapture::dur_to_ms(self.start.elapsed()));
        }
    }
    pub fn finish(&mut self) {
        if !self.finished {
            self.capture.total_ms = Some(TimingCapture::dur_to_ms(self.start.elapsed()));
            self.finished = true;
        }
    }
}

/// Trait for a collector sink (future: aggregate / stats). P3.0 only keeps
/// last capture when `finish` called.
pub trait TransportMetricsCollector: Send + Sync + 'static {
    fn record(&self, _cap: &TimingCapture) {}
}

/// No-op singleton collector.
#[derive(Default)]
pub struct NoopCollector;
impl TransportMetricsCollector for NoopCollector {}

/// Global (swap-able) metrics collector. P3.2 keeps an in-memory last-capture list
/// potential for later histogram export; we start simple.
static GLOBAL_COLLECTOR: OnceLock<Arc<dyn TransportMetricsCollector>> = OnceLock::new();

pub fn set_global_collector(c: Arc<dyn TransportMetricsCollector>) {
    let _ = GLOBAL_COLLECTOR.set(c); // ignore if already set
}

fn collector() -> Arc<dyn TransportMetricsCollector> {
    GLOBAL_COLLECTOR
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(NoopCollector))
}

// Thread-local staging area for active timing capture of a single transport attempt.
#[derive(Debug, Clone)]
pub enum FallbackEventRecord {
    Transition {
        from: &'static str,
        to: &'static str,
        reason: String,
    },
    AutoDisable {
        enabled: bool,
        threshold_pct: u8,
        cooldown_secs: u32,
    },
}

thread_local! {
    static TL_TIMING: RefCell<Option<TimingCapture>> = const { RefCell::new(None) };
    static TL_USED_FAKE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static TL_FALLBACK_STAGE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
    static TL_CERT_FP_CHANGED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static TL_IP_STRATEGY: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
    static TL_IP_SOURCE: RefCell<Option<String>> = const { RefCell::new(None) };
    static TL_IP_LATENCY: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    static TL_FALLBACK_EVENTS: RefCell<Vec<FallbackEventRecord>> = const { RefCell::new(Vec::new()) };
    // P5.3: Proxy-related thread-local fields
    static TL_USED_PROXY: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static TL_PROXY_TYPE: RefCell<Option<String>> = const { RefCell::new(None) };
    static TL_PROXY_LATENCY: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    static TL_CUSTOM_TRANSPORT_DISABLED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub fn tl_reset() {
    TL_TIMING.with(|c| *c.borrow_mut() = None);
    TL_USED_FAKE.with(|c| c.set(None));
    TL_FALLBACK_STAGE.with(|c| c.set(None));
    TL_CERT_FP_CHANGED.with(|c| c.set(None));
    TL_IP_STRATEGY.with(|c| c.set(None));
    TL_IP_SOURCE.with(|c| *c.borrow_mut() = None);
    TL_IP_LATENCY.with(|c| c.set(None));
    TL_FALLBACK_EVENTS.with(|c| c.borrow_mut().clear());
    // P5.3: Reset proxy fields
    TL_USED_PROXY.with(|c| c.set(None));
    TL_PROXY_TYPE.with(|c| *c.borrow_mut() = None);
    TL_PROXY_LATENCY.with(|c| c.set(None));
    TL_CUSTOM_TRANSPORT_DISABLED.with(|c| c.set(None));
}

pub fn tl_set_used_fake(v: bool) {
    TL_USED_FAKE.with(|c| c.set(Some(v)));
}
pub fn tl_set_fallback_stage(s: &'static str) {
    TL_FALLBACK_STAGE.with(|c| c.set(Some(s)));
}
pub fn tl_set_cert_fp_changed(changed: bool) {
    TL_CERT_FP_CHANGED.with(|c| c.set(Some(changed)));
}
pub fn tl_set_ip_selection(
    strategy: Option<&'static str>,
    source: Option<String>,
    latency_ms: Option<u32>,
) {
    TL_IP_STRATEGY.with(|c| c.set(strategy));
    TL_IP_LATENCY.with(|c| c.set(latency_ms));
    TL_IP_SOURCE.with(|cell| *cell.borrow_mut() = source);
}

/// P5.3: Set proxy usage information
pub fn tl_set_proxy_usage(
    used: bool,
    proxy_type: Option<String>,
    latency_ms: Option<u32>,
    custom_transport_disabled: bool,
) {
    TL_USED_PROXY.with(|c| c.set(Some(used)));
    TL_PROXY_TYPE.with(|cell| *cell.borrow_mut() = proxy_type);
    TL_PROXY_LATENCY.with(|c| c.set(latency_ms));
    TL_CUSTOM_TRANSPORT_DISABLED.with(|c| c.set(Some(custom_transport_disabled)));
}

pub fn tl_set_timing(cap: &TimingCapture) {
    TL_TIMING.with(|c| *c.borrow_mut() = Some(*cap));
}
pub fn tl_mark_first_byte() {
    TL_TIMING.with(|_c| { /* marker for potential future inline updates (currently no-op) */ });
}

pub struct TimingSnapshot {
    pub timing: Option<TimingCapture>,
    pub used_fake: Option<bool>,
    pub fallback_stage: Option<&'static str>,
    pub cert_fp_changed: Option<bool>,
    pub ip_strategy: Option<&'static str>,
    pub ip_source: Option<String>,
    pub ip_latency_ms: Option<u32>,
    // P5.3: Proxy-related fields
    pub used_proxy: Option<bool>,
    pub proxy_type: Option<String>,
    pub proxy_latency_ms: Option<u32>,
    pub custom_transport_disabled: Option<bool>,
}

pub fn tl_snapshot() -> TimingSnapshot {
    TimingSnapshot {
        timing: TL_TIMING.with(|c| *c.borrow()),
        used_fake: TL_USED_FAKE.with(|c| c.get()),
        fallback_stage: TL_FALLBACK_STAGE.with(|c| c.get()),
        cert_fp_changed: TL_CERT_FP_CHANGED.with(|c| c.get()),
        ip_strategy: TL_IP_STRATEGY.with(|c| c.get()),
        ip_source: TL_IP_SOURCE.with(|c| c.borrow().clone()),
        ip_latency_ms: TL_IP_LATENCY.with(|c| c.get()),
        // P5.3: Read proxy fields
        used_proxy: TL_USED_PROXY.with(|c| c.get()),
        proxy_type: TL_PROXY_TYPE.with(|c| c.borrow().clone()),
        proxy_latency_ms: TL_PROXY_LATENCY.with(|c| c.get()),
        custom_transport_disabled: TL_CUSTOM_TRANSPORT_DISABLED.with(|c| c.get()),
    }
}

pub fn tl_push_fallback_event(evt: FallbackEventRecord) {
    TL_FALLBACK_EVENTS.with(|cell| cell.borrow_mut().push(evt));
}

pub fn tl_take_fallback_events() -> Vec<FallbackEventRecord> {
    TL_FALLBACK_EVENTS.with(|cell| mem::take(&mut *cell.borrow_mut()))
}

/// Convenience: whether metrics should be captured (runtime config flag). We load
/// config on demand (cheap: small file / memory) – caching is unnecessary here.
pub fn metrics_enabled() -> bool {
    if let Ok(v) = std::env::var("FWC_TEST_FORCE_METRICS") {
        match v.as_str() {
            "0" | "false" | "False" | "FALSE" => return false,
            "1" | "true" | "True" | "TRUE" => return true,
            _ => {}
        }
    }
    // Test override (only in test builds) for deterministic gating validation
    #[cfg(test)]
    {
        use std::sync::atomic::{AtomicU8, Ordering};
        // 0 = no override, 1 = force false, 2 = force true
        extern "Rust" {
            static TEST_METRICS_OVERRIDE: AtomicU8;
        }
        let v = unsafe { TEST_METRICS_OVERRIDE.load(Ordering::Relaxed) };
        if v == 1 {
            return false;
        }
        if v == 2 {
            return true;
        }
    }
    load_or_init()
        .map(|c: AppConfig| c.tls.metrics_enabled)
        .unwrap_or(true)
}

/// Record and store into thread-local only if enabled.
pub fn finish_and_store(rec: &mut TimingRecorder) {
    if !metrics_enabled() {
        return;
    }
    rec.finish();
    tl_set_timing(&rec.capture);
    collector().record(&rec.capture);
}

// Test-only override API
#[cfg(test)]
use std::sync::atomic::{AtomicU8, Ordering};
#[cfg(test)]
#[no_mangle]
static TEST_METRICS_OVERRIDE: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
pub fn test_override_metrics_enabled(v: Option<bool>) {
    match v {
        None => TEST_METRICS_OVERRIDE.store(0, Ordering::Relaxed),
        Some(false) => TEST_METRICS_OVERRIDE.store(1, Ordering::Relaxed),
        Some(true) => TEST_METRICS_OVERRIDE.store(2, Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_respects_metrics_enabled_flag() {
        let _lock = metrics_env_lock().lock().unwrap();
        
        tl_reset();
        test_override_metrics_enabled(Some(false));
        let mut recorder = TimingRecorder::new();
        finish_and_store(&mut recorder);
        let snap_disabled = tl_snapshot();
        assert!(
            snap_disabled.timing.is_none(),
            "timing should remain unset when metrics disabled"
        );

        tl_reset();
        test_override_metrics_enabled(Some(true));
        let mut recorder_enabled = TimingRecorder::new();
        finish_and_store(&mut recorder_enabled);
        let snap_enabled = tl_snapshot();
        assert!(
            snap_enabled.timing.is_some(),
            "timing should be captured when metrics enabled"
        );

        tl_reset();
        test_override_metrics_enabled(None);
    }

    fn metrics_env_lock() -> &'static std::sync::Mutex<()> {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        prev: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                prev: std::env::var("FWC_TEST_FORCE_METRICS").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(v) => std::env::set_var("FWC_TEST_FORCE_METRICS", v),
                None => std::env::remove_var("FWC_TEST_FORCE_METRICS"),
            }
        }
    }

    #[test]
    fn metrics_enabled_env_override_takes_precedence() {
        let _lock = metrics_env_lock().lock().unwrap();
        let _guard = EnvGuard::new();

        std::env::set_var("FWC_TEST_FORCE_METRICS", "0");
        assert!(!metrics_enabled(), "env=0 should disable metrics");

        std::env::set_var("FWC_TEST_FORCE_METRICS", "1");
        assert!(metrics_enabled(), "env=1 should enable metrics");

        std::env::remove_var("FWC_TEST_FORCE_METRICS");
    }
}
