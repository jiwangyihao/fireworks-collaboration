#![cfg(not(feature = "tauri-app"))]
use proptest::prelude::*;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use uuid::Uuid;

proptest! {
    // 属性：insecure=true 时 skipSanWhitelist 最终应为 false（规范化），并报告 conflict；insecure=false 时保持输入值。
    #[test]
    fn tls_override_conflict_normalization(insecure in proptest::bool::ANY, skip in proptest::bool::ANY) {
        let mut global = AppConfig::default();
        global.tls.insecure_skip_verify = false; // baseline
        global.tls.skip_san_whitelist = false;
        let over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyTlsOverride {
            insecure_skip_verify: Some(insecure),
            skip_san_whitelist: Some(skip),
        };
    let (_ins, skip_eff, changed, conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
        if insecure {
            assert!(!skip_eff, "when insecure=true skipSan must normalize to false");
            if skip { assert!(conflict.is_some(), "conflict expected when both insecure and skip requested"); } else { assert!(conflict.is_none(), "no conflict when insecure=true but skip=false"); }
        } else {
            assert_eq!(skip_eff, skip, "when insecure=false skip preserved");
            assert!(conflict.is_none(), "no conflict when insecure=false");
        }
        // changed 仅当任意有效字段变化或规范化发生
        let normalized_change = insecure && skip; // skip 被强制归一化为 false
        let expect_changed = (insecure != global.tls.insecure_skip_verify) || (!insecure && skip != global.tls.skip_san_whitelist) || normalized_change;
        assert_eq!(changed, expect_changed, "changed flag semantic");
        // 全局未被修改
        assert!(!global.tls.insecure_skip_verify);
        assert!(!global.tls.skip_san_whitelist);
    }
}
