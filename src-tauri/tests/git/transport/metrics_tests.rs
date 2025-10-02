use fireworks_collaboration_lib::core::git::transport::metrics::{
    finish_and_store, metrics_enabled, test_override_metrics_enabled, tl_reset,
    tl_snapshot, TimingRecorder,
};
use std::sync::{Mutex, OnceLock};

fn metrics_env_lock() -> &'static Mutex<()> {
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
