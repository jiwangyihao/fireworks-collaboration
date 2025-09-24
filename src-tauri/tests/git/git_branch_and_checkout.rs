#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Branch & Checkout
//! --------------------------------
//! 精简后 sections:
//!   section_branch_create  -> 创建 / 无基提交 / invalid / force move / cancel
//!   section_checkout_basic -> create / idempotent / 不存在/空仓/取消
//! 占位：delete / dirty / detached 预留最小注释，未来扩展。
//! 优化点：
//!   * 合并 invalid name & 扩展 invalid 列表。
//!   * 合并 no-base (force 与非 force) 场景参数化。
//!   * 合并 valid names 与 checkout 成功为单测试（对第一个新建分支执行 checkout）。
//!   * 删除冗余的 `branch_force_moves`，保留引用更新验证。
//!   * 使用 `fixtures::commit_files` & `repo_with_staged` 减少重复。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_branch_create ----------------
mod section_branch_create {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, branch::git_branch, commit::git_commit};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use crate::common::{test_env, fixtures, git_helpers};

    fn repo_with_first_commit() -> (std::path::PathBuf, AtomicBool) {
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
        (dest, flag)
    }

    mod helpers {
        use super::*;
        pub(super) fn branch_op(dest: &std::path::Path, name: &str, force: bool, flag: &AtomicBool) -> Result<(), fireworks_collaboration_lib::core::git::errors::GitError> {
            git_branch(dest, name, false, force, flag, |_p| {})
        }
    }

    #[test]
    fn branch_valid_and_checkout_and_duplicate() {
        use fireworks_collaboration_lib::core::git::default_impl::checkout::git_checkout;
        test_env::init_test_env();
        let (dest, flag) = repo_with_first_commit();
        let valids = ["feature/one", "hotfix-123", "refs_ok/level", "abc", "long.name-seg"];
        // create all branches & validate phase emission
        for (i, v) in valids.iter().enumerate() {
            let mut phases = Vec::new();
            git_branch(&dest, v, false, false, &flag, |p: ProgressPayload| phases.push(p.phase.clone())).unwrap();
            assert!(phases.last().unwrap().starts_with("Branched"), "[branch-create] expected Branched phase for {v}");
            if i == 0 { git_checkout(&dest, v, false, &flag, |_p| {}).unwrap(); }
        }
        // duplicate create should fail
    let e = helpers::branch_op(&dest, valids[0], false, &flag).unwrap_err();
        git_helpers::assert_err_category("branch-create duplicate", e, ErrorCategory::Protocol);
    }

    #[test]
    fn branch_invalid_names_combined() {
        test_env::init_test_env();
        let (dest, flag) = repo_with_first_commit();
        let invalids = [
            " ", "a b", "end/", "dot.", "-lead", "a..b", "a\\b", // 原始
            "/start", "double//slash", "end.lock", "have:colon", "quest?", "star*", "brack[et", "tilda~", "caret^", "at@{sym", "ctrl\u{0007}bell" // 扩展
        ];
    for bad in invalids { let err = helpers::branch_op(&dest, bad, false, &flag).unwrap_err(); git_helpers::assert_err_category("branch-create invalid", err, ErrorCategory::Protocol); }
    }

    #[test]
    fn branch_creation_without_commit_force_and_nonforce() {
        test_env::init_test_env();
        for force in [false, true] {
            let dest = fixtures::temp_dir();
            let flag = AtomicBool::new(false);
            git_init(&dest, &flag, |_p| {}).unwrap();
            let err = helpers::branch_op(&dest, if force { "main" } else { "feature/a" }, force, &flag).unwrap_err();
            git_helpers::assert_err_category("branch-create no-base", err, ErrorCategory::Protocol);
        }
    }

    #[test]
    fn branch_cancelled_before_start() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true);
    let e = helpers::branch_op(&dest, "x", false, &flag).unwrap_err();
        git_helpers::assert_err_category("branch-create cancel", e, ErrorCategory::Cancel);
    }

    #[test]
    fn branch_force_move_updates_ref_once() {
        test_env::init_test_env();
        let (dest, flag) = repo_with_first_commit();
    helpers::branch_op(&dest, "move", false, &flag).unwrap();
        // 第二次提交后 force 移动
        fixtures::commit_files(&dest, &[("b.txt", "b")], "c2", false).unwrap();
        let repo = git2::Repository::open(&dest).unwrap();
        let new_head = repo.head().unwrap().target().unwrap();
    helpers::branch_op(&dest, "move", true, &flag).unwrap();
        let repo2 = git2::Repository::open(&dest).unwrap();
        let br = repo2.find_branch("move", git2::BranchType::Local).unwrap();
        let tgt = br.into_reference().target().unwrap();
        assert_eq!(tgt, new_head, "[branch-create] force move should update ref");
    }
}

// ---------------- section_branch_delete ----------------
mod section_branch_delete {
    // 当前源测试未包含删除场景，后续若实现公开添加：
    // - branch_delete_success
    // - branch_delete_protected_denied
    // 占位模块保持空。
}

// ---------------- section_checkout_basic ----------------
mod section_checkout_basic {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, commit::git_commit, branch::git_branch, checkout::git_checkout};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use crate::common::{test_env, fixtures, git_helpers};

    fn repo_with_commit() -> (std::path::PathBuf, AtomicBool) {
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
        (dest, flag)
    }

    mod helpers {
        use super::*;
        pub(super) fn checkout_op(dest: &std::path::Path, name: &str, create: bool, flag: &AtomicBool) -> Result<(), fireworks_collaboration_lib::core::git::errors::GitError> { git_checkout(dest, name, create, flag, |_p| {}) }
    }

    #[test]
    fn checkout_create_and_idempotent() {
        test_env::init_test_env();
        let (dest, flag) = repo_with_commit();
    helpers::checkout_op(&dest, "dev", true, &flag).unwrap();
        // 再次 create=true 应幂等
    helpers::checkout_op(&dest, "dev", true, &flag).unwrap();
    }

    #[test]
    fn checkout_nonexistent_without_create_fails() {
        test_env::init_test_env();
        let (dest, flag) = repo_with_commit();
    let e = helpers::checkout_op(&dest, "no-such", false, &flag).unwrap_err();
        git_helpers::assert_err_category("checkout-basic no-such", e, ErrorCategory::Protocol);
    }

    #[test]
    fn checkout_create_without_commit_rejected() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
    let err = helpers::checkout_op(&dest, "newbranch", true, &flag).unwrap_err();
        git_helpers::assert_err_category("checkout-basic empty-repo", err, ErrorCategory::Protocol);
    }

    #[test]
    fn checkout_cancel_during_operation() {
        test_env::init_test_env();
        let (dest, flag) = repo_with_commit();
        git_branch(&dest, "dev", false, false, &flag, |_p| {}).unwrap();
        let cancel_flag = AtomicBool::new(true);
    let err = helpers::checkout_op(&dest, "dev", false, &cancel_flag).unwrap_err();
        git_helpers::assert_err_category("checkout-basic cancel", err, ErrorCategory::Cancel);
    }
}

// ---------------- section_checkout_dirty ----------------
mod section_checkout_dirty { /* 占位：后续添加脏工作区场景 */ }

// ---------------- section_checkout_detached ----------------
mod section_checkout_detached { /* 占位：后续添加 detached HEAD 场景 */ }

// NOTE: delete/dirty/detached 保持轻量占位，避免未来功能扩展产生大 diff。