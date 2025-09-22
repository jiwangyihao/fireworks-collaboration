//! Property-based tests for HTTP strategy override normalization.
use proptest::prelude::*;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::tasks::TaskRegistry;

proptest! {
    #[test]
    fn http_override_conflict_normalizes(follow in any::<bool>(), raw_max in 0u32..25) {
        let mut global = AppConfig::default();
        // choose a base different from follow to increase variation
        global.http.follow_redirects = !follow;
        let over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyHttpOverride {
            follow_redirects: Some(follow),
            max_redirects: Some(raw_max as u32),
            ..Default::default()
        };
        let (_f, m, _changed, conflict) = TaskRegistry::apply_http_override("GitClone", &uuid::Uuid::nil(), &global, Some(&over));
        // clamp rule: raw_max is clamped to <=20
        prop_assert!(m <= 20, "clamped max should be <=20, got {m}");
        if follow == false && m > 0 { // conflict rule: follow=false => max=0
            prop_assert!(conflict.is_some(), "expected conflict message when follow=false and m>0");
        }
        if conflict.is_some() { prop_assert_eq!(m, 0, "conflict must normalize max to 0"); }
    }
}
