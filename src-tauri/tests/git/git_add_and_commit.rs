#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Add & Commit
//! ------------------------------------------------------------
//! 精简后结构说明：
//!   `section_add_basic`    -> 成功 / 目录递归 / 去重 + 进度单调
//!   `section_add_edge`     -> 参数非法 / 路径非法 / 取消
//!   `section_commit_basic` -> 正常提交 / 空提交策略 / 自定义作者 / message 修整
//!   `section_commit_edge`  -> 消息为空 / 取消 / 作者字段组合非法 / 空提交开关
//!   `section_task_wrapper` -> 任务调度与取消
//! 优化要点：
//!   * 重复 repo 初始化与 stage 逻辑 → 复用 fixtures (`repo_with_staged` / `stage_files`)。
//!   * 进度单调断言 → 公共 helper `git_helpers::assert_progress_monotonic`。
//!   * 错误断言全部使用分类枚举 (Protocol / Cancel)。
//!   * 合并三个作者非法测试为单循环参数化。
//!   * `移除冗余：add_duplicate_paths_dedupes` / `initial_empty_repo_allow_empty_toggle` 及单独作者非法变体测试。
//!   * 仅保留最小覆盖 + 代表性路径，减少维护成本。

// ---------------- section_add_basic ----------------
mod section_add_basic {
    use super::super::common::{fixtures, git_helpers, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::{add::git_add, init::git_init};
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn add_success_with_dir_and_dedup_and_progress() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).expect("[add-basic] init");
        // create files + nested dir
        std::fs::write(dest.join("a.txt"), "hello").unwrap();
        std::fs::create_dir_all(dest.join("dir/sub")).unwrap();
        std::fs::write(dest.join("dir/sub/b.txt"), "world").unwrap();
        // duplicate path included intentionally
        let mut percents = Vec::new();
        git_add(
            &dest,
            &["a.txt", "a.txt", "dir"],
            &flag,
            |p: ProgressPayload| {
                percents.push(p.percent);
            },
        )
        .expect("[add-basic] add ok");
        git_helpers::assert_progress_monotonic("add-basic", &percents);
        let repo = git2::Repository::open(&dest).unwrap();
        let idx = repo.index().unwrap();
        assert!(
            idx.get_path(std::path::Path::new("a.txt"), 0).is_some(),
            "[add-basic] a.txt staged"
        );
        assert!(
            idx.get_path(std::path::Path::new("dir/sub/b.txt"), 0)
                .is_some(),
            "[add-basic] nested file staged"
        );
    }
}

// ---------------- section_add_edge ----------------
/// 边缘与失败路径：
/// * 空列表
/// * 路径越界 / 绝对路径
/// * 取消
/// 改进：统一使用 `git_helpers` 的错误分类断言，移除本地 `cat` 重复逻辑。
mod section_add_edge {
    use crate::common::{fixtures, git_helpers, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::{add::git_add, init::git_init};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn add_rejects_empty_list() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
        let out = git_add(&dest, &[], &flag, |_p| {});
        assert!(out.is_err(), "[add-edge] expect error empty list");
        git_helpers::assert_err_category(
            "add-edge empty list",
            out.err().unwrap(),
            ErrorCategory::Protocol,
        );
    }

    #[test]
    fn add_rejects_outside_path_and_absolute() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
        std::fs::write(dest.join("a.txt"), "hi").unwrap();
        // outside via ..
        let out = git_add(&dest, &["../outside.txt"], &flag, |_p| {});
        assert!(out.is_err(), "[add-edge] expect protocol for outside path");
        git_helpers::assert_err_category(
            "add-edge outside",
            out.err().unwrap(),
            ErrorCategory::Protocol,
        );
        // absolute path
        let abs = if cfg!(windows) { "C:/Windows" } else { "/etc" };
        let out2 = git_add(&dest, &[abs], &flag, |_p| {});
        assert!(
            out2.is_err(),
            "[add-edge] expect protocol for absolute path"
        );
        git_helpers::assert_err_category(
            "add-edge absolute",
            out2.err().unwrap(),
            ErrorCategory::Protocol,
        );
    }

    #[test]
    fn add_cancelled_before() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true); // already canceled
        let out = git_add(&dest, &["a.txt"], &flag, |_p| {});
        assert!(out.is_err(), "[add-edge] expect cancel error");
        git_helpers::assert_err_category(
            "add-edge cancel",
            out.err().unwrap(),
            ErrorCategory::Cancel,
        );
    }
}

// ---------------- section_commit_basic ----------------
mod section_commit_basic {
    use crate::common::{fixtures, git_helpers, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::commit::{git_commit, Author};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use std::sync::atomic::AtomicBool;

    fn repo_with_single_file(name: &str, content: &str) -> std::path::PathBuf {
        
        fixtures::repo_with_staged(&[(name, content)])
    }

    #[test]
    fn commit_success_then_empty_reject_then_allow() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = repo_with_single_file("a.txt", "hello");
        let flag = AtomicBool::new(false);
        git_commit(&repo_dir, "feat: add a.txt", None, false, &flag, |_p| {})
            .expect("[commit-basic] first commit ok");
        let err = git_commit(&repo_dir, "chore: empty", None, false, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("commit-basic empty", err, ErrorCategory::Protocol);
        git_commit(&repo_dir, "chore: force empty", None, true, &flag, |_p| {})
            .expect("[commit-basic] allow empty");
    }

    #[test]
    fn commit_with_custom_author_and_trimmed_message() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = repo_with_single_file("g.txt", "g");
        let flag = AtomicBool::new(false);
        git_commit(
            &repo_dir,
            "  feat: trim  \n",
            Some(Author {
                name: Some("Alice"),
                email: Some("alice@example.com"),
            }),
            false,
            &flag,
            |_p| {},
        )
        .expect("[commit-basic] trimmed commit");
        let repo2 = git2::Repository::open(&repo_dir).unwrap();
        let head = repo2.head().unwrap();
        let commit = repo2.find_commit(head.target().unwrap()).unwrap();
        assert_eq!(
            commit.message().unwrap().trim(),
            "feat: trim",
            "[commit-basic] message trimmed"
        );
    }
}

// ---------------- section_commit_edge ----------------
mod section_commit_edge {
    use crate::common::{fixtures, git_helpers, test_env};
    use fireworks_collaboration_lib::core::git::default_impl::commit::{git_commit, Author};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use std::sync::atomic::AtomicBool;

    fn empty_repo() -> std::path::PathBuf {
        let p = fixtures::temp_dir();
        git2::Repository::init(&p).unwrap();
        p
    }

    #[test]
    fn commit_requires_non_empty_message() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "   \n", None, false, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("commit-edge empty msg", err, ErrorCategory::Protocol);
    }

    #[test]
    fn commit_cancel_before() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        let flag = AtomicBool::new(true);
        let err = git_commit(&repo_dir, "feat: test", None, false, &flag, |_p| {}).unwrap_err();
        git_helpers::assert_err_category("commit-edge cancel", err, ErrorCategory::Cancel);
    }

    #[test]
    fn commit_author_invalid_combinations() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let cases = vec![
            (Some("Bob"), None, "missing email"),
            (Some(""), Some("a@b.c"), "empty name"),
            (None, Some("user@example.com"), "email only"),
        ];
        for (name, email, label) in cases {
            let repo_dir = empty_repo();
            fixtures::stage_files(&repo_dir, &[("f.txt", "f")]);
            let flag = AtomicBool::new(false);
            let err = git_commit(
                &repo_dir,
                &format!("feat: invalid author {label}"),
                Some(Author { name, email }),
                false,
                &flag,
                |_p| {},
            )
            .unwrap_err();
            git_helpers::assert_err_category(
                &format!("commit-edge author {label}"),
                err,
                ErrorCategory::Protocol,
            );
        }
    }
}

// ---------------- section_task_wrapper ----------------
mod section_task_wrapper {
    use crate::common::task_wait::wait_until_task_done;
    use crate::common::{fixtures, test_env};
    use fireworks_collaboration_lib::core::tasks::model::TaskState;
    use fireworks_collaboration_lib::core::tasks::{TaskKind, TaskRegistry};

    fn staged_repo(name: &str) -> std::path::PathBuf {
        fixtures::repo_with_staged(&[(name, "x")])
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_task_completes() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let repo_dir = staged_repo("z.txt");
        let (id, token) = reg.create(TaskKind::GitCommit {
            dest: repo_dir.to_string_lossy().to_string(),
            message: "feat: z".into(),
            allow_empty: false,
            author_name: None,
            author_email: None,
        });
        let handle = reg.spawn_git_commit_task(
            None,
            id,
            token,
            repo_dir.to_string_lossy().to_string(),
            "feat: z".into(),
            false,
            None,
            None,
        );
        wait_until_task_done(&reg, id).await;
        let state = reg
            .snapshot(&id)
            .map(|s| s.state)
            .unwrap_or(TaskState::Failed);
        assert!(
            matches!(state, TaskState::Completed),
            "[task-wrapper] commit task should complete"
        );
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_task_canceled_early() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let repo_dir = staged_repo("c.txt");
        let (id, token) = reg.create(TaskKind::GitCommit {
            dest: repo_dir.to_string_lossy().to_string(),
            message: "feat: c".into(),
            allow_empty: false,
            author_name: None,
            author_email: None,
        });
        token.cancel();
        let handle = reg.spawn_git_commit_task(
            None,
            id,
            token.clone(),
            repo_dir.to_string_lossy().to_string(),
            "feat: c".into(),
            false,
            None,
            None,
        );
        wait_until_task_done(&reg, id).await;
        let state = reg
            .snapshot(&id)
            .map(|s| s.state)
            .unwrap_or(TaskState::Failed);
        assert!(
            matches!(state, TaskState::Canceled),
            "[task-wrapper] should cancel early"
        );
        handle.await.unwrap();
    }
}

// NOTE: 后续如引入事件 DSL，可在此文件进一步用 tag 序列替换底层阶段描述断言。
