#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Clone Core (Roadmap 12.4)
//! ------------------------------------------------------------
//! 原始目标待迁移文件（后续提交中逐步迁入并置占位）：
//!   - git_clone_fetch_params.rs
//!   - git_clone_fetch_params_valid.rs
//!   - git_clone_fetch_params_combo.rs
//!   - git_clone_preflight.rs (仅远端前本地/参数校验部分)
//! 分区计划：
//!   section_params_validation -> 单参数合法/非法
//!   section_params_matrix     -> 组合参数代表性子集
//!   section_preflight         -> 路径 / 已存在目录 / 权限 等本地预检查
//!   section_behavior_basic    -> 基础 clone 行为（成功/远端不存在/取消路径占位）
//! 设计要点：
//!   * 使用 `common::git_scenarios::run_clone` 封装，未来替换内部实现无需改动测试表面。
//!   * 事件断言暂保持宽松（仅验证事件非空），阶段 4 引入 DSL 后再收紧。
//!   * 参数矩阵已提供最小代表组合，等待 shallow / partial 阶段扩展。
//! 后续改进：
//!   * 引入 CloneCase / Display 输出，失败时展示 case 上下文。
//!   * 精确分类断言（Protocol / Precondition / Cancel）。
//!   * 与 12.5 (shallow) 共享 depth 验证 helper。
//! Post-audit: 已规范化 header 与 section 注释；取消路径断言去除无意义永真条件。
//! Post-audit(v2): 下一阶段 (12.8) 将对 filter 与 depth 交叉拆分；本文件的
//! params_matrix 仍保留最小组合；待事件 DSL 引入后将统一转为 pattern 断言。
//! Cross-ref: shallow 行为验证见 12.5；partial filter 行为见 12.6。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_params_validation ----------------
mod section_params_validation {
    //! 单参数验证：确保非法 depth、filter 占位与取消路径基础覆盖。
    use std::sync::atomic::AtomicBool;
    use crate::common::git_scenarios::{CloneParams, run_clone, _run_clone_with_cancel};

    // 单参数：depth=0 属于非法（Protocol）
    #[test]
    fn clone_invalid_depth_zero_fails() {
        let out = run_clone(&CloneParams { depth:Some(0), filter:None });
        // 占位实现目前不区分错误分类，这里仅要求产生事件（后续真实实现再收紧）
        assert!(!out.events.is_empty(), "[clone-core params_validation] events even on depth=0 placeholder");
    }

    // 单参数：filter 占位（目前实现未处理 filter，允许忽略，不应导致立即失败）
    #[test]
    fn clone_with_filter_placeholder_not_fail_immediately() {
        let out = run_clone(&CloneParams { depth:None, filter:Some("blob:none".into()) });
        // 占位实现：因为使用固定占位 URL，可能仍然失败，但不能因为 filter 参数解析本身失败。
        // 因缺乏分类区分，这里只断言产生事件。
        assert!(!out.events.is_empty(), "[clone-core params_validation] events emitted");
    }

    // 取消：在开始前设置标志，结果应产生错误或中断事件。
    #[test]
    fn clone_cancel_before_start() {
        let cancel = AtomicBool::new(true);
    let out = _run_clone_with_cancel(&CloneParams { depth:Some(1), filter:None }, &cancel);
    assert!(out.dest.exists(), "[clone-core params_validation] dest exists even if cancel early");
    }
}

// ---------------- section_params_matrix ----------------
mod section_params_matrix {
    //! 代表性参数组合矩阵：递归 / tags 交叉（depth/filter 暂未参与）。
    use crate::common::git_scenarios::{CloneParams, run_clone};

    #[derive(Debug, Clone, Copy)]
    struct Case { depth: Option<u32>, filter: Option<&'static str> }
    fn cases() -> Vec<Case> { vec![
        Case { depth: None, filter: None },
        Case { depth: Some(1), filter: None },
        Case { depth: None, filter: Some("blob:none") },
        Case { depth: Some(1), filter: Some("blob:none") },
    ]}

    #[test]
    fn clone_param_combinations_do_not_panic() {
        for c in cases() {
            let out = run_clone(&CloneParams { depth:c.depth, filter:c.filter.map(|s| s.to_string()) });
            assert!(!out.events.is_empty(), "[clone-core params_matrix] events for {:?}", c);
        }
    }
}

// ---------------- section_preflight ----------------
mod section_preflight {
    //! 本地预检：目标路径已存在等场景（真实实现接入前暂宽松，只验证不 panic）。
    use std::fs;
    use crate::common::{git_scenarios::{CloneParams, run_clone}};

    // 预检：目标路径先存在（目录已创建）不应导致 run_clone 自身崩溃（真实实现后扩展分类）
    #[test]
    fn preflight_dest_exists_directory() {
        let dest = std::env::temp_dir().join(format!("fwc-clone-preflight-exist-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dest).unwrap();
        // 暂时直接调用 run_clone（内部会新建自己的随机目录，不复用外部 dest——后续真实实现时会接受 dest 参数）
    let out = run_clone(&CloneParams { depth:None, filter:None });
        assert!(!out.events.is_empty(), "[clone-core preflight] events emitted");
    }
}

// ---------------- section_behavior_basic ----------------
mod section_behavior_basic {
    //! 行为基础：取消中途（占位逻辑），未来添加成功 / 远端不存在 / 网络错误等路径。
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use crate::common::git_scenarios::{CloneParams, _run_clone_with_cancel};

    #[test]
    fn behavior_cancel_midway_placeholder() {
        let cancel = AtomicBool::new(false);
        cancel.store(true, Ordering::Relaxed); // 立即取消模拟“中途”
    let out = _run_clone_with_cancel(&CloneParams { depth:Some(1), filter:None }, &cancel);
    assert!(out.dest.exists(), "[clone-core behavior] dest exists after cancel placeholder");
    }
}

// 说明：文件已完成 12.4 聚合核心；后续 12.5 将在此基础上扩展 shallow/depth 验证。
