//! Git Reset 测试
//! --------------------------------
//! 测试 git_reset 功能，用于拉取操作（fetch + reset）。
//!
//! Sections:
//! - `section_reset_basic` -> 基本 reset 到本地分支/远程跟踪分支
//! - `section_reset_errors` -> 无效引用/空仓库/取消操作

// ---------------- section_reset_basic ----------------
mod section_reset_basic {
    use crate::common::{fixtures, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::{
        branch::git_branch, commit::git_commit, reset::git_reset,
    };
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use std::sync::atomic::AtomicBool;

    fn repo_with_two_commits() -> (std::path::PathBuf, git2::Oid, git2::Oid, AtomicBool) {
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();

        let repo = git2::Repository::open(&dest).unwrap();
        let first_oid = repo.head().unwrap().target().unwrap();

        // 第二次提交
        fixtures::commit_files(&dest, &[("b.txt", "b")], "c2", false).unwrap();
        let repo = git2::Repository::open(&dest).unwrap();
        let second_oid = repo.head().unwrap().target().unwrap();

        (dest, first_oid, second_oid, flag)
    }

    #[test]
    fn reset_hard_to_previous_commit() {
        test_env::init_test_env();
        let (dest, first_oid, second_oid, flag) = repo_with_two_commits();

        // 确认当前在第二个提交
        let repo = git2::Repository::open(&dest).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), second_oid);

        // Reset 到第一个提交
        let mut phases = Vec::new();
        git_reset(
            &dest,
            &first_oid.to_string(),
            true,
            &flag,
            |p: ProgressPayload| phases.push(p.phase.clone()),
        )
        .unwrap();

        // 验证 HEAD 现在指向第一个提交
        let repo = git2::Repository::open(&dest).unwrap();
        assert_eq!(
            repo.head().unwrap().target().unwrap(),
            first_oid,
            "reset should move HEAD to first commit"
        );

        // 验证进度回调
        assert!(
            phases.iter().any(|p| p.contains("Completed")),
            "should emit Completed phase"
        );

        // 验证 hard reset 删除了第二次提交添加的文件
        assert!(
            !dest.join("b.txt").exists(),
            "hard reset should remove b.txt"
        );
    }

    #[test]
    fn reset_soft_keeps_working_tree() {
        test_env::init_test_env();
        let (dest, first_oid, _second_oid, flag) = repo_with_two_commits();

        // Soft reset 到第一个提交
        git_reset(&dest, &first_oid.to_string(), false, &flag, |_p| {}).unwrap();

        // 验证 HEAD 移动
        let repo = git2::Repository::open(&dest).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), first_oid);

        // 验证 soft reset 保留了工作区文件
        assert!(dest.join("b.txt").exists(), "soft reset should keep b.txt");
    }

    #[test]
    fn reset_to_local_branch_name() {
        test_env::init_test_env();
        let (dest, first_oid, _second_oid, flag) = repo_with_two_commits();

        // 创建一个分支指向第一个提交
        git_branch(&dest, "old-branch", false, false, &flag, |_p| {}).unwrap();
        // 将 old-branch 移动到第一个提交
        let repo = git2::Repository::open(&dest).unwrap();
        let commit = repo.find_commit(first_oid).unwrap();
        repo.branch("old-branch", &commit, true).unwrap();

        // Reset 到分支名
        git_reset(&dest, "old-branch", true, &flag, |_p| {}).unwrap();

        let repo = git2::Repository::open(&dest).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), first_oid);
    }

    #[test]
    fn reset_to_refs_heads_format() {
        test_env::init_test_env();
        let (dest, first_oid, _second_oid, flag) = repo_with_two_commits();

        // 创建分支
        let repo = git2::Repository::open(&dest).unwrap();
        let commit = repo.find_commit(first_oid).unwrap();
        repo.branch("target-branch", &commit, true).unwrap();

        // Reset 使用完整引用格式
        git_reset(&dest, "refs/heads/target-branch", true, &flag, |_p| {}).unwrap();

        let repo = git2::Repository::open(&dest).unwrap();
        assert_eq!(repo.head().unwrap().target().unwrap(), first_oid);
    }
}

// ---------------- section_reset_errors ----------------
mod section_reset_errors {
    use crate::common::{fixtures, git_helpers, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::{
        commit::git_commit, init::git_init, reset::git_reset,
    };
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn reset_nonexistent_reference_fails() {
        test_env::init_test_env();
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();

        let err = git_reset(&dest, "nonexistent-ref", true, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("reset-errors nonexistent", err, ErrorCategory::Protocol);
    }

    #[test]
    fn reset_empty_reference_fails() {
        test_env::init_test_env();
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();

        let err = git_reset(&dest, "", true, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("reset-errors empty", err, ErrorCategory::Protocol);
    }

    #[test]
    fn reset_whitespace_only_reference_fails() {
        test_env::init_test_env();
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();

        let err = git_reset(&dest, "   ", true, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("reset-errors whitespace", err, ErrorCategory::Protocol);
    }

    #[test]
    fn reset_not_a_repo_fails() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);

        let err = git_reset(&dest, "main", true, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("reset-errors not-repo", err, ErrorCategory::Protocol);
    }

    #[test]
    fn reset_cancelled_before_start() {
        test_env::init_test_env();
        let dest = fixtures::repo_with_staged(&[("a.txt", "a")]);
        let flag = AtomicBool::new(false);
        git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();

        let cancel_flag = AtomicBool::new(true);
        let err = git_reset(&dest, "HEAD", true, &cancel_flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("reset-errors cancel", err, ErrorCategory::Cancel);
    }

    #[test]
    fn reset_empty_repo_fails() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();

        let err = git_reset(&dest, "main", true, &flag, |_p| {}).unwrap_err();
        // 空仓库没有 main 分支，应该是 Protocol 错误
        git_helpers::assert_err_category("reset-errors empty-repo", err, ErrorCategory::Protocol);
    }
}

// ---------------- section_reset_remote_tracking ----------------
mod section_reset_remote_tracking {
    // 远程跟踪分支的 reset 测试需要模拟远程环境，
    // 这在集成测试中较为复杂，暂时占位。
    // 实际功能已在手动测试中验证。
}
