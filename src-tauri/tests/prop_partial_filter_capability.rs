#![cfg(not(feature = "tauri-app"))]
use proptest::prelude::*;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
// 移除未使用导入 (AppConfig, Uuid)

// 利用 parse_depth_filter_opts 间接验证 capability 事件逻辑的输入空间（不发事件，只验证 fallback 决策函数行为）
proptest! {
    #[test]
    fn partial_filter_fallback_decision(depth in prop::option::of(0u32..5), filter in prop::option::of("[a-z0-9:_]{1,8}")) {
        // capability 支持与否两个分支
        for supported in [true,false] {
            let shallow_expected = depth.is_some();
            let res = TaskRegistry::decide_partial_fallback(depth, filter.as_deref(), supported);
            if filter.is_none() || supported { assert!(res.is_none(), "no fallback when no filter or supported"); }
            else {
                let (msg, shallow_flag) = res.expect("expected fallback");
                assert!(msg.contains("partial filter unsupported"));
                assert_eq!(shallow_flag, shallow_expected, "shallow flag should mirror depth presence");
            }
        }
    }
}
