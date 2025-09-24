#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Init & Repo Layout (Roadmap 12.1)
//! ------------------------------------------------------------
//! 原始来源文件（已迁移并留占位）：
//!   - git_init.rs
//!   - git_local_skeleton.rs
//! 分区结构：
//!   section_basic_init   -> 初始化成功 / 幂等 / 取消
//!   section_repo_layout  -> 尚未实现操作的协议错误占位
//!   section_preflight    -> 路径前置静态校验（文件路径）
//! 设计要点：
//!   * 使用 tests/common 下统一 fixtures 与环境初始化。
//!   * 错误分类断言统一通过 git_helpers，便于后续扩展 / 统计。
//!   * 事件 DSL 计划在阶段 4 引入，本阶段仍直接断言返回分类。
//! 后续可改进：
//!   * 增加更多 preflight 校验（权限 / 已存在非空目录 等）。
//!   * repo_layout 模块后续将被真实实现覆盖或删除。
//!   * 与 add/commit/branch 共享统一的进度事件模式断言 helper。
//!
//! 注意：legacy 占位文件保持以便追溯，待阶段 12 全量完成后可考虑删除。
//! Post-audit: header 规范化 & section 注释补全（12.1 检视）。

// 引入公共测试支持模块（位于 ../common/）。
#[path = "../common/mod.rs"]
mod common;

mod section_basic_init {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;

    use crate::common::{fixtures, test_env};
    use crate::common::git_helpers;

    #[test]
    fn git_init_success_and_idempotent() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).expect("[init-basic] init ok");
        assert!(dest.join(".git").exists(), "[init-basic] expect .git dir after init");
        // second time should be idempotent
        git_init(&dest, &flag, |_p| {}).expect("[init-basic] idempotent");
    }

    // preflight 相关错误场景已移动到 section_preflight。

    #[test]
    fn git_init_cancel_before() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true); // already canceled
        let out = git_init(&dest, &flag, |_p| {});
        assert!(out.is_err(), "[init-basic] expect cancel error");
        git_helpers::assert_err_category("init-basic cancel", out.err().unwrap(), ErrorCategory::Cancel);
    }
}

// 预留：本地 preflight 场景（后续扩展时添加）
mod section_preflight {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use crate::common::{fixtures, test_env};

    use crate::common::git_helpers;

    #[test]
    fn init_fails_when_target_is_file() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        std::fs::create_dir_all(&dest).unwrap();
        let file_path = dest.join("a.txt");
        std::fs::write(&file_path, "hi").unwrap();
        let cancel = AtomicBool::new(false);
        let out = git_init(&file_path, &cancel, |_p| {});
        assert!(out.is_err(), "[preflight] expect protocol error for file path");
        git_helpers::assert_err_category("preflight path-is-file", out.err().unwrap(), ErrorCategory::Protocol);
    }
}

// section_preflight: 本地路径 / 类型校验，目前仅包含“路径是文件”场景，可在后续扩展更多无远端交互的静态前置检查。

// 说明：保持测试函数名称与原实现接近，便于搜索历史；添加了前缀化上下文信息。

// 注意：不在此重新 `mod common`，测试框架会自动编译 `tests/common` 目录；
// 其他测试若需引用，可使用 `use crate::common::fixtures;` 形式。
