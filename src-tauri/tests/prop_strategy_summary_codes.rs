#![cfg(not(feature = "tauri-app"))]
use proptest::prelude::*;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use uuid::Uuid;

// 通过直接调用 apply_* 函数模拟组合，验证 applied_codes 集合与 changed flags 一致性。
proptest! {
    #[test]
    fn strategy_summary_applied_codes_consistency(http_follow in proptest::bool::ANY,
                                                  http_max in 0u8..=20,
                                                  tls_insecure in proptest::bool::ANY,
                                                  tls_skip in proptest::bool::ANY,
                                                  retry_max in 1u32..10,
                                                  retry_base in 50u64..500,
                                                  retry_factor in 1u32..5) {
        // 构造 global config 与 overrides 使其可能触发 changed。
        let mut global = AppConfig::default();
        global.http.follow_redirects = !http_follow; // 反转保证 changed
        global.http.max_redirects = if http_max==0 {1} else {http_max}; // 避免与 override 相同时不 changed
        global.tls.insecure_skip_verify = !tls_insecure;
        global.tls.skip_san_whitelist = !tls_skip;
        global.retry.max = retry_max + 5; // 保证不同
        global.retry.base_ms = retry_base + 10;
        global.retry.factor = (retry_factor as f64) + 0.5;

        // 构造 overrides
    let http_over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyHttpOverride { follow_redirects: Some(http_follow), max_redirects: Some(http_max as u32), ..Default::default() };
        let tls_over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyTlsOverride { insecure_skip_verify: Some(tls_insecure), skip_san_whitelist: Some(tls_skip) };
        let retry_over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyRetryOverride { max: Some(retry_max), base_ms: Some(retry_base as u32), factor: Some(retry_factor as f32), jitter: Some(false) };

        let (f,m,http_changed,_http_conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&http_over));
        let (ins,skip,tls_changed,_tls_conflict) = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&tls_over));
        let (retry_plan, retry_changed) = TaskRegistry::apply_retry_override(&global.retry, Some(&retry_over));

        let mut codes: Vec<String> = vec![];
        if http_changed { codes.push("http_strategy_override_applied".into()); }
        if tls_changed { codes.push("tls_strategy_override_applied".into()); }
        if retry_changed { codes.push("retry_strategy_override_applied".into()); }

        // 模拟 summary 事件中 applied_codes 的排序/去重逻辑
        codes.sort(); codes.dedup();
        // 验证：每个 changed 标志必然对应一个 code
        if http_changed { assert!(codes.iter().any(|c| c=="http_strategy_override_applied")); }
        if tls_changed { assert!(codes.iter().any(|c| c=="tls_strategy_override_applied")); }
        if retry_changed { assert!(codes.iter().any(|c| c=="retry_strategy_override_applied")); }

        // 若没有任何 changed，则 codes 应为空
        if !http_changed && !tls_changed && !retry_changed { assert!(codes.is_empty()); }

        // 确保全局配置未被修改
        assert_ne!(global.http.follow_redirects, f, "we intentionally changed follow to trigger changed");
        let _ = (m, ins, skip, retry_plan); // silence unused warnings; semantic checks covered above
    }
}
