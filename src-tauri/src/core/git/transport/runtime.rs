use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::core::config::model::HttpCfg;

const SAMPLE_CAP: usize = 20;
const SAMPLE_WINDOW_SECS: u64 = 120;
const MIN_SAMPLES: usize = 5;

use metrics::{describe_counter, register_counter};
use once_cell::sync::OnceCell;

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy)]
pub struct AutoDisableConfig {
    pub threshold_pct: u8,
    pub cooldown_sec: u64,
}

impl AutoDisableConfig {
    pub fn from_http_cfg(cfg: &HttpCfg) -> Self {
        Self {
            threshold_pct: cfg.auto_disable_fake_threshold_pct.min(100),
            cooldown_sec: cfg.auto_disable_fake_cooldown_sec,
        }
    }

    fn is_enabled(&self) -> bool {
        self.threshold_pct > 0 && self.cooldown_sec > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoDisableEvent {
    Triggered {
        threshold_pct: u8,
        cooldown_secs: u32,
    },
    Recovered,
}

#[derive(Default)]
struct AutoDisableState {
    samples: VecDeque<Sample>,
    disabled_until: Option<Instant>,
}

#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    failed: bool,
}

fn state() -> &'static Mutex<AutoDisableState> {
    static STATE: OnceLock<Mutex<AutoDisableState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AutoDisableState::default()))
}

fn triggered_counter() -> &'static metrics::Counter {
    static COUNTER: OnceCell<metrics::Counter> = OnceCell::new();
    COUNTER.get_or_init(|| {
        describe_counter!(
            "adaptive_tls_auto_disable_triggered_total",
            "Number of times adaptive TLS fake SNI was temporarily disabled"
        );
        register_counter!("adaptive_tls_auto_disable_triggered_total")
    })
}

fn recovered_counter() -> &'static metrics::Counter {
    static COUNTER: OnceCell<metrics::Counter> = OnceCell::new();
    COUNTER.get_or_init(|| {
        describe_counter!(
            "adaptive_tls_auto_disable_recovered_total",
            "Number of times adaptive TLS fake SNI auto-disable recovered"
        );
        register_counter!("adaptive_tls_auto_disable_recovered_total")
    })
}

#[cfg(test)]
static TRIGGERED_METRIC_CALLS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static RECOVERED_METRIC_CALLS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
fn note_triggered_metric() {
    TRIGGERED_METRIC_CALLS.fetch_add(1, Ordering::SeqCst);
}

#[cfg(not(test))]
fn note_triggered_metric() {}

#[cfg(test)]
fn note_recovered_metric() {
    RECOVERED_METRIC_CALLS.fetch_add(1, Ordering::SeqCst);
}

#[cfg(not(test))]
fn note_recovered_metric() {}

#[cfg(test)]
pub fn test_reset_metric_counters() {
    TRIGGERED_METRIC_CALLS.store(0, Ordering::SeqCst);
    RECOVERED_METRIC_CALLS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub fn test_metric_counter_values() -> (u64, u64) {
    (
        TRIGGERED_METRIC_CALLS.load(Ordering::SeqCst),
        RECOVERED_METRIC_CALLS.load(Ordering::SeqCst),
    )
}

fn window_duration() -> Duration {
    Duration::from_secs(SAMPLE_WINDOW_SECS)
}

pub fn is_fake_disabled(cfg: &AutoDisableConfig) -> bool {
    if !cfg.is_enabled() {
        return false;
    }
    let mut guard = state().lock().expect("auto disable mutex poisoned");
    if let Some(deadline) = guard.disabled_until {
        if deadline <= Instant::now() {
            guard.disabled_until = None;
            guard.samples.clear();
            return false;
        }
        return true;
    }
    false
}

pub fn record_fake_attempt(cfg: &AutoDisableConfig, failed: bool) -> Option<AutoDisableEvent> {
    if !cfg.is_enabled() {
        return None;
    }
    record_fake_attempt_with_now(cfg, failed, Instant::now())
}

fn record_fake_attempt_with_now(
    cfg: &AutoDisableConfig,
    failed: bool,
    now: Instant,
) -> Option<AutoDisableEvent> {
    let mut guard = state().lock().expect("auto disable mutex poisoned");
    if let Some(deadline) = guard.disabled_until {
        if deadline <= now {
            guard.disabled_until = None;
            guard.samples.clear();
            recovered_counter().increment(1);
            note_recovered_metric();
            return Some(AutoDisableEvent::Recovered);
        }
        guard.samples.clear();
        return None;
    }

    prune_expired(&mut guard, now);
    push_sample(&mut guard, Sample { at: now, failed });

    let total = guard.samples.len();
    if total < MIN_SAMPLES {
        return None;
    }
    let failures = guard.samples.iter().filter(|s| s.failed).count();
    let ratio = (failures as f64 / total as f64) * 100.0;
    if failures == 0 {
        return None;
    }
    if ratio >= f64::from(cfg.threshold_pct) {
        let cooldown = Duration::from_secs(cfg.cooldown_sec);
        guard.disabled_until = Some(now + cooldown);
        guard.samples.clear();
        triggered_counter().increment(1);
        note_triggered_metric();
        return Some(AutoDisableEvent::Triggered {
            threshold_pct: cfg.threshold_pct,
            cooldown_secs: cfg.cooldown_sec as u32,
        });
    }
    None
}

fn prune_expired(state: &mut AutoDisableState, now: Instant) {
    let window = window_duration();
    while let Some(front) = state.samples.front() {
        if now.duration_since(front.at) > window {
            state.samples.pop_front();
        } else {
            break;
        }
    }
}

fn push_sample(state: &mut AutoDisableState, sample: Sample) {
    if state.samples.len() >= SAMPLE_CAP {
        state.samples.pop_front();
    }
    state.samples.push_back(sample);
}

#[cfg(any(test, not(feature = "tauri-app")))]
pub(crate) fn reset_auto_disable_internal() {
    if let Ok(mut guard) = state().lock() {
        guard.samples.clear();
        guard.disabled_until = None;
    }
}

#[cfg(any(test, not(feature = "tauri-app")))]
pub(crate) fn auto_disable_guard_internal() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
pub(crate) fn test_auto_disable_guard() -> &'static Mutex<()> {
    auto_disable_guard_internal()
}

#[cfg(test)]
pub(crate) fn test_reset_auto_disable() {
    reset_auto_disable_internal();
}

#[cfg(not(feature = "tauri-app"))]
pub mod testing {
    //! Exposes auto-disable helpers for integration tests.
    use super::*;

    pub fn reset_auto_disable() {
        super::reset_auto_disable_internal();
    }

    pub fn auto_disable_guard() -> &'static Mutex<()> {
        super::auto_disable_guard_internal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(threshold: u8, cooldown: u64) -> AutoDisableConfig {
        AutoDisableConfig {
            threshold_pct: threshold,
            cooldown_sec: cooldown,
        }
    }

    #[test]
    fn auto_disable_triggers_when_ratio_exceeds_threshold() {
        let _guard = super::test_auto_disable_guard().lock().unwrap();
        test_reset_auto_disable();
        super::test_reset_metric_counters();
        let cfg = cfg(50, 30);
        let mut now = Instant::now();
        for _ in 0..4 {
            let _ = record_fake_attempt_with_now(&cfg, false, now);
            now += Duration::from_secs(1);
        }
        for i in 0..4 {
            let evt = record_fake_attempt_with_now(&cfg, true, now);
            if i < 3 {
                assert!(evt.is_none(), "expected no trigger on failure {}", i + 1);
            } else {
                assert!(matches!(
                    evt,
                    Some(AutoDisableEvent::Triggered {
                        threshold_pct: 50,
                        cooldown_secs: 30
                    })
                ));
            }
            now += Duration::from_secs(1);
        }
        assert!(is_fake_disabled(&cfg));
        let (triggered, recovered) = super::test_metric_counter_values();
        assert_eq!(triggered, 1);
        assert_eq!(recovered, 0);
    }

    #[test]
    fn auto_disable_recovers_after_cooldown() {
        let _guard = super::test_auto_disable_guard().lock().unwrap();
        test_reset_auto_disable();
        super::test_reset_metric_counters();
        let cfg = cfg(50, 1);
        let base = Instant::now();
        let mut triggered_at: Option<Instant> = None;
        for i in 0..10 {
            let attempt_time = base + Duration::from_secs(i as u64);
            let failed = i % 2 == 1;
            if let Some(evt) = record_fake_attempt_with_now(&cfg, failed, attempt_time) {
                assert!(matches!(evt, AutoDisableEvent::Triggered { .. }));
                triggered_at = Some(attempt_time);
                break;
            }
        }
        let trigger_time = triggered_at.expect("expected auto-disable trigger");
        assert!(is_fake_disabled(&cfg));
        let recovery_time = trigger_time + Duration::from_secs(2);
        assert!(matches!(
            record_fake_attempt_with_now(&cfg, false, recovery_time),
            Some(AutoDisableEvent::Recovered)
        ));
        assert!(!is_fake_disabled(&cfg));
        let (triggered, recovered) = super::test_metric_counter_values();
        assert_eq!(triggered, 1);
        assert_eq!(recovered, 1);
    }

    #[test]
    fn disabled_feature_returns_none() {
        let _guard = super::test_auto_disable_guard().lock().unwrap();
        test_reset_auto_disable();
        super::test_reset_metric_counters();
        let cfg = cfg(0, 30);
        assert!(!is_fake_disabled(&cfg));
        assert!(record_fake_attempt(&cfg, true).is_none());
        let (triggered, recovered) = super::test_metric_counter_values();
        assert_eq!(triggered, 0);
        assert_eq!(recovered, 0);
    }
}
