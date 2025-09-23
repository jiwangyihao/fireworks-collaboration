//! Transport metrics scaffolding (P3.0).
//!
//! Intentionally minimal: collects coarse timing segments in-memory. No
//! background export or event emission yet – subsequent P3.x stages will hook
//! into this.
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
    fn dur_to_ms(d: Duration) -> u32 { d.as_millis().min(u128::from(u32::MAX)) as u32 }
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
    pub fn new() -> Self { Self { start: Instant::now(), connect_start: None, tls_start: None, first_byte_at: None, finished: false, capture: TimingCapture::default() } }
    pub fn mark_connect_start(&mut self) { self.connect_start = Some(Instant::now()); }
    pub fn mark_connect_end(&mut self) { if let Some(s) = self.connect_start { self.capture.connect_ms = Some(TimingCapture::dur_to_ms(s.elapsed())); } }
    pub fn mark_tls_start(&mut self) { self.tls_start = Some(Instant::now()); }
    pub fn mark_tls_end(&mut self) { if let Some(s) = self.tls_start { self.capture.tls_ms = Some(TimingCapture::dur_to_ms(s.elapsed())); } }
    pub fn mark_first_byte(&mut self) { if self.first_byte_at.is_none() { self.first_byte_at = Some(Instant::now()); self.capture.first_byte_ms = Some(TimingCapture::dur_to_ms(self.start.elapsed())); } }
    pub fn finish(&mut self) { if !self.finished { self.capture.total_ms = Some(TimingCapture::dur_to_ms(self.start.elapsed())); self.finished = true; } }
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
    GLOBAL_COLLECTOR.get().cloned().unwrap_or_else(|| Arc::new(NoopCollector))
}

// Thread-local staging area for active timing capture of a single transport attempt.
thread_local! {
    static TL_TIMING: std::cell::RefCell<Option<TimingCapture>> = const { std::cell::RefCell::new(None) };
    static TL_USED_FAKE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    static TL_FALLBACK_STAGE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
    static TL_CERT_FP_CHANGED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub fn tl_reset() {
    TL_TIMING.with(|c| *c.borrow_mut() = None);
    TL_USED_FAKE.with(|c| c.set(None));
    TL_FALLBACK_STAGE.with(|c| c.set(None));
    TL_CERT_FP_CHANGED.with(|c| c.set(None));
}

pub fn tl_set_used_fake(v: bool) { TL_USED_FAKE.with(|c| c.set(Some(v))); }
pub fn tl_set_fallback_stage(s: &'static str) { TL_FALLBACK_STAGE.with(|c| c.set(Some(s))); }
pub fn tl_set_cert_fp_changed(changed: bool) { TL_CERT_FP_CHANGED.with(|c| c.set(Some(changed))); }
pub fn tl_set_timing(cap: &TimingCapture) { TL_TIMING.with(|c| *c.borrow_mut() = Some(*cap)); }
pub fn tl_mark_first_byte() {
    TL_TIMING.with(|_c| { /* marker for potential future inline updates (currently no-op) */ });
}

pub struct TimingSnapshot {
    pub timing: Option<TimingCapture>,
    pub used_fake: Option<bool>,
    pub fallback_stage: Option<&'static str>,
    pub cert_fp_changed: Option<bool>,
}

pub fn tl_snapshot() -> TimingSnapshot {
    TimingSnapshot {
        timing: TL_TIMING.with(|c| *c.borrow()),
        used_fake: TL_USED_FAKE.with(|c| c.get()),
        fallback_stage: TL_FALLBACK_STAGE.with(|c| c.get()),
        cert_fp_changed: TL_CERT_FP_CHANGED.with(|c| c.get()),
    }
}

/// Convenience: whether metrics should be captured (runtime config flag). We load
/// config on demand (cheap: small file / memory) – caching is unnecessary here.
pub fn metrics_enabled() -> bool {
    // Test override (only in test builds) for deterministic gating validation
    #[cfg(test)]
    {
        use std::sync::atomic::{AtomicU8, Ordering};
        // 0 = no override, 1 = force false, 2 = force true
        extern "Rust" {
            static TEST_METRICS_OVERRIDE: AtomicU8;
        }
        let v = unsafe { TEST_METRICS_OVERRIDE.load(Ordering::Relaxed) };
        if v == 1 { return false; }
        if v == 2 { return true; }
    }
    load_or_init().map(|c:AppConfig| c.tls.metrics_enabled).unwrap_or(true)
}

/// Record and store into thread-local only if enabled.
pub fn finish_and_store(rec: &mut TimingRecorder) {
    if !metrics_enabled() { return; }
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
