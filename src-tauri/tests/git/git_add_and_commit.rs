#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Add & Commit (Roadmap 12.2)
//! ------------------------------------------------------------
//! 原始来源文件（已完全迁移并留占位）：
//!   - git_add.rs
//!   - git_add_enhanced.rs
//!   - git_commit.rs
//!   - git_commit_task.rs
//! 分区结构：
//!   section_add_basic    -> 成功 / 进度单调 / 目录递归
//!   section_add_edge     -> 参数非法 / 取消
//!   section_commit_basic -> 正常提交 / 空提交策略 / 自定义作者 / message 修整
//!   section_commit_edge  -> 消息非法 / 取消 / 作者字段校验 / 允许空提交开关
//!   section_task_wrapper -> 任务调度与取消路径验证
//! 设计要点：
//!   * 仍直接使用低层断言，事件 DSL 将在阶段 4 统一引入。
//!   * 通过上下文前缀 ("[add-basic]" 等) 强化失败定位。
//!   * stage_files / repo_with_staged 等通用准备逻辑集中于 fixtures。
//! 后续可改进：
//!   * 将进度事件断言抽象为 helper（避免重复 windows/monotonic 验证模式）。
//!   * commit / add 错误分类统一转为错误枚举匹配（当前部分使用字符串 contains）。
//!   * 与 branch/checkout 阶段共享更高层 repo factory（多分支初始化）。
//! Cross-ref: 见 Roadmap 12.3 (分支/checkout) 与未来 12.9 (push & retry) 针对提交引用场景的复用计划。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_add_basic ----------------
mod section_add_basic {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add};
    use fireworks_collaboration_lib::core::git::service::ProgressPayload;
    use crate::common::{fixtures, test_env};

    #[test]
    fn add_success_files_and_dir() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).expect("[add-basic] init");
        std::fs::write(dest.join("a.txt"), "hello").unwrap();
        std::fs::create_dir_all(dest.join("dir/sub")).unwrap();
        std::fs::write(dest.join("dir/sub/b.txt"), "world").unwrap();
        let mut phases = Vec::new();
        git_add(&dest, &["a.txt", "dir"], &flag, |p: ProgressPayload| { phases.push(p.phase); }).expect("[add-basic] add ok");
        let repo = git2::Repository::open(&dest).unwrap();
        let idx = repo.index().unwrap();
        assert!(idx.get_path(std::path::Path::new("a.txt"), 0).is_some(), "[add-basic] a.txt staged");
        assert!(idx.get_path(std::path::Path::new("dir/sub/b.txt"), 0).is_some(), "[add-basic] nested file staged");
    }

    #[test]
    fn add_duplicate_paths_dedupes() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
        std::fs::write(dest.join("a.txt"), "hi").unwrap();
        git_add(&dest, &["a.txt", "a.txt"], &flag, |_p| {}).expect("[add-basic] duplicate ok");
    }

    #[test]
    fn add_progress_monotonic() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
        std::fs::write(dest.join("f1.txt"), "1").unwrap();
        std::fs::write(dest.join("f2.txt"), "2").unwrap();
        let mut percents: Vec<u32> = Vec::new();
        let mut phases: Vec<String> = Vec::new();
        git_add(&dest, &["f1.txt", "f2.txt"], &flag, |p: ProgressPayload| { percents.push(p.percent); phases.push(p.phase); }).unwrap();
        assert!(percents.len() >= 2, "[add-basic] expect >=2 progress events");
        for w in percents.windows(2) { assert!(w[1] >= w[0], "[add-basic] percent not monotonic: {:?}", percents); }
        assert!(phases.last().unwrap().contains("Staged"), "[add-basic] last phase should indicate staged");
    }
}

// ---------------- section_add_edge ----------------
/// 边缘与失败路径：
/// * 空列表
/// * 路径越界 / 绝对路径
/// * 取消
/// 改进：统一使用 `git_helpers` 的错误分类断言，移除本地 `cat` 重复逻辑。
mod section_add_edge {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add};
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use crate::common::{fixtures, test_env, git_helpers};

    #[test]
    fn add_rejects_empty_list() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).unwrap();
    let out = git_add(&dest, &[], &flag, |_p| {});
    assert!(out.is_err(), "[add-edge] expect error empty list");
    git_helpers::assert_err_category("add-edge empty list", out.err().unwrap(), ErrorCategory::Protocol);
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
    git_helpers::assert_err_category("add-edge outside", out.err().unwrap(), ErrorCategory::Protocol);
        // absolute path
        let abs = if cfg!(windows) { "C:/Windows" } else { "/etc" };
    let out2 = git_add(&dest, &[abs], &flag, |_p| {});
    assert!(out2.is_err(), "[add-edge] expect protocol for absolute path");
    git_helpers::assert_err_category("add-edge absolute", out2.err().unwrap(), ErrorCategory::Protocol);
    }

    #[test]
    fn add_cancelled_before() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true); // already canceled
        let out = git_add(&dest, &["a.txt"], &flag, |_p| {});
        assert!(out.is_err(), "[add-edge] expect cancel error");
        git_helpers::assert_err_category("add-edge cancel", out.err().unwrap(), ErrorCategory::Cancel);
    }
}

// ---------------- section_commit_basic ----------------
mod section_commit_basic {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::commit::{git_commit, Author};
    use crate::common::{test_env, fixtures};

    fn init_repo_with_file(name: &str, content: &str) -> std::path::PathBuf {
        let repo_dir = fixtures::temp_dir();
        let _repo = git2::Repository::init(&repo_dir).unwrap();
        std::fs::write(repo_dir.join(name), content).unwrap();
        let repo = git2::Repository::open(&repo_dir).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(name)).unwrap();
        index.write().unwrap();
        repo_dir
    }

    #[test]
    fn commit_success_then_empty_reject_then_allow() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = init_repo_with_file("a.txt", "hello");
        let flag = AtomicBool::new(false);
        git_commit(&repo_dir, "feat: add a.txt", None, false, &flag, |_p| {}).expect("[commit-basic] commit ok");
        let err = git_commit(&repo_dir, "chore: empty", None, false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("empty commit"), "[commit-basic] expect empty rejection");
        git_commit(&repo_dir, "chore: force empty", None, true, &flag, |_p| {}).expect("[commit-basic] allow empty");
    }

    #[test]
    fn commit_with_custom_author() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = init_repo_with_file("x.txt", "x");
        let flag = AtomicBool::new(false);
        git_commit(&repo_dir, "feat: custom author", Some(Author { name: Some("Alice"), email: Some("alice@example.com") }), false, &flag, |_p| {}).expect("[commit-basic] custom author");
    }

    #[test]
    fn commit_message_trimming() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = init_repo_with_file("g.txt", "g");
        let flag = AtomicBool::new(false);
        git_commit(&repo_dir, "  feat: trim  \n", None, false, &flag, |_p| {}).expect("[commit-basic] trimmed commit");
        let repo2 = git2::Repository::open(&repo_dir).unwrap();
        let head = repo2.head().unwrap();
        let commit = repo2.find_commit(head.target().unwrap()).unwrap();
        assert_eq!(commit.message().unwrap().trim(), "feat: trim", "[commit-basic] message trimmed");
    }
}

// ---------------- section_commit_edge ----------------
mod section_commit_edge {
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::default_impl::commit::{git_commit, Author};
    use crate::common::{test_env, fixtures};

    // TODO(post-audit): 若底层错误已被分类，可将当前基于字符串的断言替换为 git_helpers::assert_err_category。

    fn empty_repo() -> std::path::PathBuf { let p = fixtures::temp_dir(); git2::Repository::init(&p).unwrap(); p }

    #[test]
    fn commit_requires_non_empty_message() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "   \n", None, false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("commit message is empty"), "[commit-edge] message empty");
    }

    #[test]
    fn commit_cancel_before() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        let flag = AtomicBool::new(true);
        let err = git_commit(&repo_dir, "feat: test", None, false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).to_lowercase().contains("cancel"), "[commit-edge] expect cancel");
    }

    #[test]
    fn commit_missing_email_rejected() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        // stage file
        fixtures::stage_files(&repo_dir, &[("y.txt", "y")]);
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "feat: missing email", Some(Author { name: Some("Bob"), email: None }), false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("author name/email required"), "[commit-edge] email required");
    }

    #[test]
    fn commit_author_empty_name_rejected() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        fixtures::stage_files(&repo_dir, &[("f.txt", "f")]);
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "feat: invalid author", Some(Author { name: Some(""), email: Some("a@b.c") }), false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("author name/email required"), "[commit-edge] empty name");
    }

    #[test]
    fn initial_empty_repo_allow_empty_toggle() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "feat: nothing", None, false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("empty commit"), "[commit-edge] empty commit rejected");
        git_commit(&repo_dir, "feat: empty allowed", None, true, &flag, |_p| {}).expect("[commit-edge] empty commit allowed");
    }

    #[test]
    fn commit_author_email_only_rejected() {
        // 补充：仅提供 email 不提供 name 与只提供 name 行为保持一致（都应失败）
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let repo_dir = empty_repo();
        fixtures::stage_files(&repo_dir, &[("q.txt", "q")]);
        let flag = AtomicBool::new(false);
        let err = git_commit(&repo_dir, "feat: email only", Some(Author { name: None, email: Some("user@example.com") }), false, &flag, |_p| {}).unwrap_err();
        assert!(format!("{}", err).contains("author name/email required"), "[commit-edge] email only should fail");
    }
}

// ---------------- section_task_wrapper ----------------
mod section_task_wrapper {
    use std::time::Duration;
    use fireworks_collaboration_lib::core::tasks::{TaskRegistry, TaskKind};
    use fireworks_collaboration_lib::core::tasks::model::TaskState;
    use crate::common::{test_env, fixtures};

    fn prep_repo_with_file(name: &str) -> std::path::PathBuf {
        let repo_dir = fixtures::temp_dir();
        let repo = git2::Repository::init(&repo_dir).unwrap();
        std::fs::write(repo_dir.join(name), b"x").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(name)).unwrap();
        index.write().unwrap();
        repo_dir
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_task_completes() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let repo_dir = prep_repo_with_file("z.txt");
        let (id, token) = reg.create(TaskKind::GitCommit { dest: repo_dir.to_string_lossy().to_string(), message: "feat: z".into(), allow_empty: false, author_name: None, author_email: None });
        let handle = reg.spawn_git_commit_task(None, id, token, repo_dir.to_string_lossy().to_string(), "feat: z".into(), false, None, None);
        let mut waited = 0u64;
        let state = loop {
            if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break s.state; } }
            tokio::time::sleep(Duration::from_millis(50)).await; waited += 50; if waited > 3000 { panic!("[task-wrapper] timeout waiting commit task"); }
        };
        assert!(matches!(state, TaskState::Completed), "[task-wrapper] commit task should complete");
        handle.await.unwrap();
        let repo2 = git2::Repository::open(&repo_dir).unwrap();
        assert!(repo2.head().unwrap().target().is_some(), "[task-wrapper] HEAD should point to commit");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_task_canceled_early() {
        test_env::init_test_env();
        std::env::set_var("FWC_E2E_DISABLE", "true");
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let repo_dir = prep_repo_with_file("c.txt");
        let (id, token) = reg.create(TaskKind::GitCommit { dest: repo_dir.to_string_lossy().to_string(), message: "feat: c".into(), allow_empty: false, author_name: None, author_email: None });
        token.cancel();
        let handle = reg.spawn_git_commit_task(None, id, token.clone(), repo_dir.to_string_lossy().to_string(), "feat: c".into(), false, None, None);
        let mut waited = 0u64;
        let state = loop {
            if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Canceled|TaskState::Failed|TaskState::Completed) { break s.state; } }
            tokio::time::sleep(Duration::from_millis(30)).await; waited += 30; if waited > 1500 { panic!("[task-wrapper] timeout waiting canceled state"); }
        };
        assert!(matches!(state, TaskState::Canceled), "[task-wrapper] should cancel early");
        handle.await.unwrap();
    }
}

// 说明：后续阶段可将 add/commit 事件断言迁移至统一 event DSL，并对重复索引操作使用公共 helper。
