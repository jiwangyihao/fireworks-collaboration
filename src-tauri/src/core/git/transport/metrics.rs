//! Transport metrics scaffolding (P3.0).
//!
//! Intentionally minimal: collects coarse timing segments in-memory. No
//! background export or event emission yet – subsequent P3.x stages will hook
//! into this.
use std::time::{Duration, Instant};

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
