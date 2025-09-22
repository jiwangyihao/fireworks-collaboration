#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Branch & Checkout (Roadmap 12.3)
//! ------------------------------------------------------------
//! 来源文件（已迁移后原文件保留占位）：
//!   - git_branch_checkout.rs
//! 分区结构：
//!   section_branch_create    -> 创建 / 列举 / 合法与非法命名 / force move
//!   section_branch_delete    -> 分支删除（当前源文件未提供，留 TODO）
//!   section_checkout_basic   -> 基础 checkout / create=true 行为
//!   section_checkout_dirty   -> 脏工作区（当前源文件未提供，留 TODO）
//!   section_checkout_detached-> 分离 HEAD / tag checkout（当前源文件未提供，留 TODO）
//! 设计要点：
//!   * 初期仍使用直接断言 + 字符串/错误分类匹配；后续事件 DSL 统一化。
//!   * 公共仓库构造与 HEAD/当前分支判断抽象在 repo_factory（若缺失将逐步补齐）。
//!   * 通过前缀标签 ("[branch-create]" / "[checkout-basic]") 提升失败定位效率。
//! Post-audit: 添加无实现场景的清晰 TODO 占位；未来计划在 12.11 前将取消/脏工作区案例集中迁移。
//! Post-audit(v2): 将在 12.11 preconditions/cancel 阶段补充 dirty / detached / delete
//! 具体用例，并抽象 current_branch helper（避免过早封装保持最小集）。
//! Cross-ref: 见 Roadmap 12.2 (add/commit) 与 12.11 (preconditions/cancel) 对脏工作区与取消语义的聚合。
//!
//! TODO(后续阶段)：
//!   - 添加 branch 删除测试用例（若实现暴露）。
//!   - 添加 dirty 工作区与冲突 checkout 测试。
//!   - 添加分离 HEAD / tag checkout 测试。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_branch_create ----------------
mod section_branch_create {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, commit::git_commit, branch::git_branch, add::git_add};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use crate::common::{test_env, fixtures, git_helpers};

    fn prep_repo_with_commit() -> (std::path::PathBuf, AtomicBool) {
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).expect("[branch-create] init");
        std::fs::write(dest.join("a.txt"), "a").unwrap();
        git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
        (dest, flag)
    }

    #[test]
    fn branch_create_and_checkout_success() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        let mut phases: Vec<String> = Vec::new();
        git_branch(&dest, "feature/x", false, false, &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
        assert!(phases.last().unwrap().contains("Branched"), "[branch-create] expect Branched phase");
        fireworks_collaboration_lib::core::git::default_impl::checkout::git_checkout(&dest, "feature/x", false, &flag, |_p| {}).unwrap();
    }

    #[test]
    fn branch_conflict_without_force() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        git_branch(&dest, "dup", false, false, &flag, |_p| {}).unwrap();
    let e = git_branch(&dest, "dup", false, false, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("branch-create duplicate", e, ErrorCategory::Protocol);
    }

    #[test]
    fn branch_force_moves() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        git_branch(&dest, "force-test", false, false, &flag, |_p| {}).unwrap();
        // new commit
        std::fs::write(dest.join("2.txt"), "2").unwrap();
        git_add(&dest, &["2.txt"], &flag, |_p| {}).unwrap();
        git_commit(&dest, "feat: second", None, false, &flag, |_p| {}).unwrap();
        git_branch(&dest, "force-test", false, true, &flag, |_p| {}).unwrap();
    }

    #[test]
    fn branch_cancelled_before_start() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true); // canceled
    let e = git_branch(&dest, "x", false, false, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("branch-create cancel", e, ErrorCategory::Cancel);
    }

    #[test]
    fn branch_invalid_names_rejected() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        for bad in [" ", "a b", "end/", "dot.", "-lead", "a..b", "a\\b"] {
            let err = git_branch(&dest, bad, false, false, &flag, |_p| {}).unwrap_err();
            git_helpers::assert_err_category("branch-create invalid", err, ErrorCategory::Protocol);
        }
    }

    #[test]
    fn branch_creation_without_commit_rejected() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
    let err = git_branch(&dest, "feature/a", false, false, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("branch-create no-base", err, ErrorCategory::Protocol);
    }

    #[test]
    fn branch_force_without_commit_rejected() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
    let err = git_branch(&dest, "main", false, true, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("branch-create force-no-base", err, ErrorCategory::Protocol);
    }

    #[test]
    fn branch_force_move_updates_ref() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        git_branch(&dest, "move", false, false, &flag, |_p| {}).unwrap();
        // second commit
        std::fs::write(dest.join("b.txt"), "b").unwrap();
        git_add(&dest, &["b.txt"], &flag, |_p| {}).unwrap();
        git_commit(&dest, "c2", None, false, &flag, |_p| {}).unwrap();
        let repo = git2::Repository::open(&dest).unwrap();
        let new_head = repo.head().unwrap().target().unwrap();
        git_branch(&dest, "move", false, true, &flag, |_p| {}).unwrap();
        let repo2 = git2::Repository::open(&dest).unwrap();
        let br = repo2.find_branch("move", git2::BranchType::Local).unwrap();
        let tgt = br.into_reference().target().unwrap();
        assert_eq!(tgt, new_head, "[branch-create] force move should update ref");
    }

    #[test]
    fn branch_valid_names_succeed_and_phase_emitted() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        let valids = ["feature/one", "hotfix-123", "refs_ok/level", "abc", "long.name-seg"];
    for v in valids { let mut phases = Vec::new(); let _ = git_branch(&dest, v, false, false, &flag, |p:ProgressPayload| phases.push(p.phase)); assert!(phases.last().unwrap().starts_with("Branched"), "[branch-create] expected Branched phase for {v}"); }
    }

    #[test]
    fn branch_new_invalid_additional_cases() {
        test_env::init_test_env();
        let (dest, flag) = prep_repo_with_commit();
        let invalids = ["/start", "double//slash", "end.lock", "have:colon", "quest?", "star*", "brack[et", "tilda~", "caret^", "at@{sym", "ctrl\u{0007}bell"];
    for bad in invalids { let err = git_branch(&dest, bad, false, false, &flag, |_p| {}).unwrap_err(); git_helpers::assert_err_category("branch-create invalid-ext", err, ErrorCategory::Protocol); }
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
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, commit::git_commit, branch::git_branch, checkout::git_checkout, add::git_add};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use crate::common::{test_env, fixtures, git_helpers};

    fn prep() -> (std::path::PathBuf, AtomicBool) {
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
        std::fs::write(dest.join("a.txt"), "a").unwrap();
        git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
        (dest, flag)
    }

    #[test]
    fn checkout_nonexistent_without_create_fails() {
        test_env::init_test_env();
        let (dest, flag) = prep();
    let e = git_checkout(&dest, "no-such", false, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("checkout-basic no-such", e, ErrorCategory::Protocol);
    }

    #[test]
    fn checkout_create_success() {
        test_env::init_test_env();
        let (dest, flag) = prep();
        git_checkout(&dest, "new-branch", true, &flag, |_p| {}).unwrap();
    }

    #[test]
    fn checkout_create_on_existing_branch_noop_like() {
        test_env::init_test_env();
        let (dest, flag) = prep();
        git_branch(&dest, "dev", false, false, &flag, |_p| {}).unwrap();
        git_checkout(&dest, "dev", true, &flag, |_p| {}).unwrap();
    }

    #[test]
    fn checkout_create_without_commit_rejected() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
    let err = git_checkout(&dest, "newbranch", true, &flag, |_p| {}).unwrap_err();
    git_helpers::assert_err_category("checkout-basic empty-repo", err, ErrorCategory::Protocol);
    }

    #[test]
    fn checkout_cancel_during_operation() {
        test_env::init_test_env();
        let (dest, flag) = prep();
        git_branch(&dest, "dev", false, false, &flag, |_p| {}).unwrap();
        let cancel_flag = AtomicBool::new(true);
        let err = git_checkout(&dest, "dev", false, &cancel_flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("checkout-basic cancel", err, ErrorCategory::Cancel);
    }
}

// ---------------- section_checkout_dirty ----------------
mod section_checkout_dirty {
    // 源文件未包含脏工作区 checkout 测试。后续可添加：
    // - checkout_dirty_conflict_rejected
    // - checkout_dirty_force_overwrites
}

// ---------------- section_checkout_detached ----------------
mod section_checkout_detached {
    // 源文件未包含分离 HEAD / tag checkout 场景。后续可添加：
    // - checkout_tag_detached_head
    // - checkout_specific_commit_detached
}

// 说明：删除/dirty/detached 场景当前保持占位，有助于后续扩展与统一结构。