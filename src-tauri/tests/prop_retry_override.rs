//! Property-based tests for retry override diff behavior.
use proptest::prelude::*;
use fireworks_collaboration_lib::core::config::model::RetryCfg;
use fireworks_collaboration_lib::core::git::default_impl::opts::StrategyRetryOverride;
use fireworks_collaboration_lib::tasks::TaskRegistry;

proptest! {
    #[test]
    fn retry_override_changes_detected(max in 0u32..10, base_ms in 50u32..2000, factor in 1.0f64..3.0f64, jitter in any::<bool>()) {
        let mut global = RetryCfg::default();
        global.max = 5; // fixed baseline
        global.base_ms = 300;
        global.factor = 1.5;
        global.jitter = true;
        let over = StrategyRetryOverride { max: Some(max), base_ms: Some(base_ms), factor: Some(factor as f32), jitter: Some(jitter) };
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
        if plan.max == global.max && plan.base_ms == global.base_ms && (plan.factor - global.factor).abs() < f64::EPSILON && plan.jitter == global.jitter {
            prop_assert!(!changed, "no field changed but changed=true");
        } else {
            prop_assert!(changed, "some field changed but changed=false");
        }
        // factor range maintained
        prop_assert!(plan.factor >= 0.0, "factor should stay non-negative");
    }
}
