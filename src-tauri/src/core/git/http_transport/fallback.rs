use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::git::transport::{FallbackReason, FallbackStage};

static FALLBACK_TLS_TOTAL: AtomicU64 = AtomicU64::new(0);
static FALLBACK_VERIFY_TOTAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn stage_label(stage: FallbackStage) -> &'static str {
    match stage {
        FallbackStage::Fake => "Fake",
        FallbackStage::Real => "Real",
        FallbackStage::Default => "Default",
        FallbackStage::None => "None",
    }
}

pub(super) fn reason_label(reason: FallbackReason) -> &'static str {
    match reason {
        FallbackReason::EnterFake => "EnterFake",
        FallbackReason::FakeHandshakeError => "FakeHandshakeError",
        FallbackReason::SkipFakePolicy => "SkipFakePolicy",
        FallbackReason::RealFailed => "RealFailed",
    }
}

pub(super) fn classify_and_count_fallback(err_msg: &str) -> &'static str {
    let em = err_msg.to_ascii_lowercase();
    // rustls 错误文本约定：General("SAN whitelist mismatch") 或域名不符等 -> Verify；其他握手/IO -> Tls
    if em.contains("whitelist")
        || em.contains("san")
        || em.contains("name")
        || em.contains("verify")
        || em.contains("pin")
    {
        FALLBACK_VERIFY_TOTAL.fetch_add(1, Ordering::Relaxed);
        "Verify"
    } else {
        FALLBACK_TLS_TOTAL.fetch_add(1, Ordering::Relaxed);
        "Tls"
    }
}

pub(crate) fn reset_fallback_counters_internal() {
    FALLBACK_TLS_TOTAL.store(0, Ordering::Relaxed);
    FALLBACK_VERIFY_TOTAL.store(0, Ordering::Relaxed);
}

pub(crate) fn snapshot_fallback_counters_internal() -> (u64, u64) {
    (
        FALLBACK_TLS_TOTAL.load(Ordering::Relaxed),
        FALLBACK_VERIFY_TOTAL.load(Ordering::Relaxed),
    )
}

pub(super) mod injection {
    use git2::Error;
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    use crate::core::git::transport::FallbackStage;

    fn fake_queue() -> &'static Mutex<VecDeque<String>> {
        static Q: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
        Q.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn real_queue() -> &'static Mutex<VecDeque<String>> {
        static Q: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
        Q.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    pub fn inject(stage: FallbackStage, msg: String) {
        let queue = match stage {
            FallbackStage::Fake => fake_queue(),
            FallbackStage::Real => real_queue(),
            _ => return,
        };
        if let Ok(mut guard) = queue.lock() {
            guard.push_back(msg);
        }
    }

    pub fn take(stage: FallbackStage) -> Option<Error> {
        let queue = match stage {
            FallbackStage::Fake => fake_queue(),
            FallbackStage::Real => real_queue(),
            _ => return None,
        };
        let msg = queue.lock().ok().and_then(|mut g| g.pop_front());
        msg.map(|m| Error::from_str(&m))
    }

    pub fn reset() {
        if let Ok(mut g) = fake_queue().lock() {
            g.clear();
        }
        if let Ok(mut g) = real_queue().lock() {
            g.clear();
        }
    }
}

pub(crate) fn inject_fake_failure_internal(msg: impl Into<String>) {
    injection::inject(FallbackStage::Fake, msg.into());
}

pub(crate) fn inject_real_failure_internal(msg: impl Into<String>) {
    injection::inject(FallbackStage::Real, msg.into());
}

pub(crate) fn reset_injected_failures_internal() {
    injection::reset();
}

#[cfg(test)]
pub(crate) fn test_inject_fake_failure(msg: impl Into<String>) {
    inject_fake_failure_internal(msg);
}

#[cfg(test)]
pub(crate) fn test_inject_real_failure(msg: impl Into<String>) {
    inject_real_failure_internal(msg);
}

#[cfg(test)]
pub(crate) fn test_reset_fallback_counters() {
    reset_fallback_counters_internal();
}

#[cfg(test)]
pub(crate) fn test_reset_injected_failures() {
    reset_injected_failures_internal();
}

#[cfg(test)]
pub(crate) fn test_snapshot_fallback_counters() -> (u64, u64) {
    snapshot_fallback_counters_internal()
}

pub mod testing {
    //! Helper functions for integration tests focusing on fallback behaviour.
    pub fn reset_fallback_counters() {
        super::reset_fallback_counters_internal();
    }

    pub fn snapshot_fallback_counters() -> (u64, u64) {
        super::snapshot_fallback_counters_internal()
    }

    pub fn classify_and_count_fallback(err_msg: &str) -> &'static str {
        super::classify_and_count_fallback(err_msg)
    }

    pub fn inject_fake_failure(msg: impl Into<String>) {
        super::inject_fake_failure_internal(msg);
    }

    pub fn inject_real_failure(msg: impl Into<String>) {
        super::inject_real_failure_internal(msg);
    }

    pub fn reset_injected_failures() {
        super::reset_injected_failures_internal();
    }
}

#[cfg(test)]
pub(crate) fn test_classify_and_count_fallback(err: &str) -> &'static str {
    // 当启用 tauri-app 特性时，testing 模块不会被编译；直接调用内部函数即可。
    #[cfg(not(feature = "tauri-app"))]
    {
        return testing::classify_and_count_fallback(err);
    }
    #[cfg(feature = "tauri-app")]
    {
        return crate::core::git::http_transport::fallback::classify_and_count_fallback(err);
    }
}
