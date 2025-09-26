#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Clone Core
//! --------------------------------
//! 精简策略：
//! - 合并单参数与组合矩阵为统一参数用例循环。
//! - 仅保留一个取消测试（早取消）。
//! - 删除当前无实际效用的 preflight 占位测试（真实实现接入后可重新添加）。
//! - 使用公共 helper `assert_clone_events` 保持断言一致。
//! - 为每个参数组合生成标签，失败时输出上下文。
//! 后续扩展：
//! - 引入分类断言（Protocol/Cancel）。
//! - 与 shallow / partial filter 文件共享 depth & filter 矩阵。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_params_and_behavior ----------------
mod section_params_and_behavior {
    use crate::common::git_scenarios::{
        run_clone, CloneParams, _run_clone_with_cancel, assert_clone_events,
    };
    use std::sync::atomic::AtomicBool;

    #[derive(Debug, Clone, Copy)]
    struct Case {
        depth: Option<u32>,
        filter: Option<&'static str>,
    }
    impl Case {
        fn label(&self) -> String {
            format!("depth={:?},filter={:?}", self.depth, self.filter)
        }
    }
    fn cases() -> Vec<Case> {
        vec![
            Case {
                depth: None,
                filter: None,
            },
            Case {
                depth: Some(1),
                filter: None,
            },
            Case {
                depth: None,
                filter: Some("blob:none"),
            },
            Case {
                depth: Some(1),
                filter: Some("blob:none"),
            },
            Case {
                depth: Some(0),
                filter: None,
            }, // 非法 depth (占位实现仍只检查事件)
        ]
    }

    #[test]
    fn clone_parameter_cases_emit_events() {
        for c in cases() {
            let out = run_clone(&CloneParams {
                depth: c.depth,
                filter: c.filter.map(|s| s.to_string()),
            });
            assert_clone_events(&format!("clone-core case {}", c.label()), &out);
        }
    }

    #[test]
    fn clone_cancel_early() {
        let cancel = AtomicBool::new(true);
        let out = _run_clone_with_cancel(
            &CloneParams {
                depth: Some(1),
                filter: None,
            },
            &cancel,
        );
        // 早取消：占位实现可能尚未产生活动事件，因此只要求目标目录已创建。
        assert!(
            out.dest.exists(),
            "[clone-core cancel-early] dest should exist"
        );
        // 若未来实现保证至少一个事件，可改为 assert_clone_events。
    }
}

// NOTE: 真实 shallow / partial 行为加入后，此文件将只保留核心参数与取消路径，具体特性在专用测试文件覆盖。
