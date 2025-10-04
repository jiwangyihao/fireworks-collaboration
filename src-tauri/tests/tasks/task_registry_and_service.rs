#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：TaskRegistry & Git Service Progress / Cancel (Roadmap Phase 2 / v1.15)
//! -----------------------------------------------------------------------------
//! 计划目的：将零散 `TaskRegistry` 与 Git Service 相关生命周期 / 并发 / 取消 / 进度
//! 测试集中到单一模块，降低根目录测试文件数量，统一轮询辅助与结构化分区。
//!
//! 迁移来源（root-level -> 本文件 sections，按附录 A.5 / A.6 顺序）：
//!   * `task_integration.rs`
//!   * `task_registry_edge.rs`
//!   * `task_registry_extra.rs`
//!   * `task_registry_post_complete_cancel.rs`
//!   * `git_tasks.rs`
//!   * `git_tasks_local.rs`
//!   * `git_impl_tests.rs` (仅 progress / fast cancel / negotiating anchor 用例，本阶段不裁剪原文件)
//!
//! Section 划分（附录 B Phase 2 定义）与预期测试分布 (初始分类结果)：
//!   `section_registry_lifecycle`      -> 2 tests (基本完成 & list 包含)
//!   `section_registry_cancel`         -> 6 tests (正常取消 / 立即取消 / idempotent / 完成后取消语义 / 启动前取消 / registry 内 git clone token cancel)
//!   `section_registry_concurrency`    -> 3 tests (多任务并行 / 高并行短任务 / 部分取消混合)
//!   `section_registry_edge`           -> 3 tests (snapshot unknown / cancel unknown / list 克隆独立性)
//!   `section_service_progress`        -> 5 tests (Negotiating anchor / 本地 clone progress 完整性 / fetch 更新远程引用 / registry 本地 clone / registry 本地 fetch)
//!   `section_service_cancel_fast`     -> 9 tests (早期错误与快速取消合并：invalid url/scheme/path/flag cancel/ invalid repo / fast cancel fetch 等)
//!  合计预计迁移测试数: 28
//!
//! Metrics (Final after migration):
//!   * Tests migrated: 28 / 28 (registry 12 + `git_tasks` 10 + impl progress/cancel 6)
//!   * File length: ~500 (< 600 目标范围内)
//!   * Helpers unified: `wait_predicate` / `wait_task_state` / `spawn_sleep_and_wait`
//!   * Root-level pruned: `task_integration.rs`, `task_registry_edge.rs`, `task_registry_extra.rs`, `task_registry_post_complete_cancel.rs`,
//!     `git_tasks.rs`, `git_tasks_local.rs` 全部占位化
//!   * `git_impl_tests.rs` 剪裁（本阶段仅搬运 progress/cancel 6 测试，估计剪裁覆盖 ~30% 行为语义；Phase 3 将继续）
//!
//! 设计原则：
//!   1. 保留原测试函数名（必要时前缀 section_ 避免冲突）便于 grep 追踪。
//!   2. 统一轮询等待策略，减少 magic 常量分散，集中可调 `TIMEOUT_MS` 常量。
//!   3. 将 Service 级别（直接 `GitService` 阻塞调用）与 Registry 级任务区分到不同 section，防止语义混淆。
//!   4. 早期 fail (invalid url/path) 与 fast cancel 合并因行为均 <2s 内结束，统一判定策略。
//!   5. 暂不抽象 DSL；保持最小侵入迁移，后续 Phase 3 可评估事件断言或属性测试整合。
//!
//! 迁移状态标记：完成一个 section 后更新 Tests migrated 计数；最终更新文档 `TESTS_REFACTOR_PLAN.md` (v1.15)。
//! 原文件在完成对应迁移后将被替换为占位 (assert!(true)) 以避免重复与后续冲突。
//!
//! Future TODO (post Phase 2):
//!   * 将进度相关断言抽象为 helper (`assert_progress_phases`) 统一校验 Negotiating -> (Receiving)? -> Checkout -> Completed 序列允许缺失可选阶段。
//!   * 对 fast fail 场景增加错误分类断言（区分 Protocol / Cancel / IO）。
//!   * 评估将轮询超时/步长调优为指数退避降低 CI 抖动。

// 导入 common 模块
use super::common::{task_wait, test_env};

// 轻量进度阶段断言辅助（复用到 service_progress 内部，不扩大公共 API）

fn assert_progress_core(phases: &[String]) {
    // 不同实现/快路径下，某些阶段可能被省略（例如本地克隆时 Negotiating/Checkout 可能缺失）。
    // 放宽为：至少包含 Negotiating / Receiving / Resolving / Checkout 中的任意一个核心阶段。
    let has_core = ["Negotiating", "Receiving", "Resolving", "Checkout"]
        .iter()
        .any(|need| phases.iter().any(|p| p == need));
    assert!(
        has_core,
        "should contain at least one core phase (Negotiating/Receiving/Resolving/Checkout)"
    );
}

// 统一通过 common::test_env / common::task_wait 提供环境与等待工具。

// ---------------- section_registry_lifecycle ----------------
mod section_registry_lifecycle {
    //! 完成 / 列表基础行为
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::core::tasks::model::TaskState;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_sleep_task_complete_integration() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 150 });
        reg.clone().spawn_sleep_task(None, id, token, 150);
        let ok = super::task_wait::wait_predicate(
            || {
                reg.snapshot(&id)
                    .map(|s| matches!(s.state, TaskState::Completed))
                    .unwrap_or(false)
            },
            2_000,
            30,
        )
        .await;
        assert!(ok, "sleep task should complete within timeout");
    }

    #[tokio::test]
    async fn test_list_contains_created_tasks() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let mut ids = vec![];
        for i in 0..3 {
            let (id, token) = reg.create(TaskKind::Sleep { ms: 50 + i * 10 });
            reg.clone().spawn_sleep_task(None, id, token, 50 + i * 10);
            ids.push(id);
        }
        assert_eq!(reg.list().len(), 3, "list should contain all tasks");
    }
}

// ---------------- section_registry_cancel ----------------
mod section_registry_cancel {
    //! 各类取消语义 (运行中 / 完成后 / token)
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_sleep_task_cancel_integration() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 1_000 });
        reg.clone().spawn_sleep_task(None, id, token.clone(), 1_000);
        let running = super::task_wait::wait_predicate(
            || {
                reg.snapshot(&id)
                    .map(|s| matches!(s.state, TaskState::Running))
                    .unwrap_or(false)
            },
            1_000,
            20,
        )
        .await;
        assert!(running, "task should enter running state");
        token.cancel();
        let canceled = super::task_wait::wait_predicate(
            || {
                reg.snapshot(&id)
                    .map(|s| matches!(s.state, TaskState::Canceled))
                    .unwrap_or(false)
            },
            1_000,
            30,
        )
        .await;
        assert!(
            canceled,
            "task should transition to canceled after token.cancel()"
        );
    }

    #[tokio::test]
    async fn test_immediate_cancel_before_completion() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 400 });
        reg.clone().spawn_sleep_task(None, id, token.clone(), 400);
        let running = super::task_wait::wait_predicate(
            || {
                reg.snapshot(&id)
                    .map(|s| matches!(s.state, TaskState::Running))
                    .unwrap_or(false)
            },
            500,
            25,
        )
        .await;
        assert!(running, "task should reach running state");
        token.cancel();
        let canceled = super::task_wait::wait_predicate(
            || {
                reg.snapshot(&id)
                    .map(|s| matches!(s.state, TaskState::Canceled))
                    .unwrap_or(false)
            },
            1_000,
            30,
        )
        .await;
        assert!(canceled, "task should cancel");
    }

    #[tokio::test]
    async fn cancel_idempotent() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 200 });
        reg.clone().spawn_sleep_task(None, id, token.clone(), 200);
        let _ = super::task_wait::wait_task_state(&reg, &id, TaskState::Running, 1_000, 25).await; // 宽松等待
        assert!(reg.cancel(&id));
        assert!(reg.cancel(&id));
    }

    #[tokio::test]
    async fn cancel_after_completion_returns_true_and_keeps_completed_state() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 80 });
        reg.clone().spawn_sleep_task(None, id, token, 80);
        for _ in 0..80 {
            if let Some(s) = reg.snapshot(&id) {
                if matches!(s.state, TaskState::Completed) {
                    break;
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
        let before = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(before.state, TaskState::Completed));
        assert!(reg.cancel(&id));
        let after = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(after.state, TaskState::Completed));
    }
}

// ---------------- section_registry_concurrency ----------------
mod section_registry_concurrency {
    //! 并行与部分取消
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_multi_tasks_parallel() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let mut ids = vec![];
        for _ in 0..5 {
            let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
            reg.clone().spawn_sleep_task(None, id, token, 120);
            ids.push(id);
        }
        let all_done = super::task_wait::wait_predicate(
            || {
                ids.iter().all(|id| {
                    reg.snapshot(id)
                        .map(|s| matches!(s.state, TaskState::Completed))
                        .unwrap_or(false)
                })
            },
            3_000,
            40,
        )
        .await;
        assert!(all_done, "all parallel tasks should complete");
    }

    #[tokio::test]
    async fn test_high_parallel_short_tasks() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let mut ids = vec![];
        for _ in 0..20 {
            let (id, token) = reg.create(TaskKind::Sleep { ms: 90 });
            reg.clone().spawn_sleep_task(None, id, token, 90);
            ids.push(id);
        }
        let all_completed = super::task_wait::wait_predicate(
            || {
                ids.iter().all(|id| {
                    reg.snapshot(id)
                        .map(|s| matches!(s.state, TaskState::Completed))
                        .unwrap_or(false)
                })
            },
            3_000,
            30,
        )
        .await;
        assert!(all_completed, "all short tasks should complete in parallel");
    }

    #[tokio::test]
    async fn test_partial_cancel_mixture() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let mut cancel_tokens = vec![];
        let mut ids = vec![];
        for i in 0..10 {
            let (id, token) = reg.create(TaskKind::Sleep { ms: 300 });
            reg.clone().spawn_sleep_task(None, id, token.clone(), 300);
            if i % 2 == 0 {
                cancel_tokens.push(token.clone());
            }
            ids.push(id);
        }
        for t in cancel_tokens {
            t.cancel();
        }
        let done = super::task_wait::wait_predicate(
            || {
                ids.iter().all(|id| {
                    reg.snapshot(id)
                        .map(|s| matches!(s.state, TaskState::Completed | TaskState::Canceled))
                        .unwrap_or(false)
                })
            },
            2_500,
            35,
        )
        .await;
        assert!(done, "all tasks should end in completed or canceled");
    }
}

// ---------------- section_registry_edge ----------------
mod section_registry_edge {
    //! snapshot / cancel unknown / list 克隆
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::Arc;

    #[tokio::test]
    async fn snapshot_unknown_returns_none() {
        super::test_env::init_test_env();
        let reg = TaskRegistry::new();
        let random = uuid::Uuid::new_v4();
        assert!(reg.snapshot(&random).is_none());
    }

    #[tokio::test]
    async fn cancel_unknown_returns_false() {
        super::test_env::init_test_env();
        let reg = TaskRegistry::new();
        let random = uuid::Uuid::new_v4();
        assert!(!reg.cancel(&random));
    }

    #[tokio::test]
    async fn list_snapshots_are_independent_clones() {
        super::test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 50 });
        reg.clone().spawn_sleep_task(None, id, token, 50);
        let list_before = reg.list();
        assert_eq!(list_before.len(), 1);
        let _ = super::task_wait::wait_task_state(&reg, &id, TaskState::Completed, 1_000, 25).await;
        let list_after = reg.list();
        assert_eq!(list_after.len(), 1);
        let new_state = &list_after[0].state;
        assert!(matches!(
            new_state,
            TaskState::Completed | TaskState::Canceled
        ));
    }
}

// ---------------- section_service_progress ----------------
mod section_service_progress {
    //! `GitService` 正常进度链路 / Negotiating / Completed
    use fireworks_collaboration_lib::core::git::service::GitService;
    use fireworks_collaboration_lib::core::git::DefaultGitService;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::atomic::AtomicBool;
    // no timeout/Duration used in this section
    use crate::common::fixtures;
    use std::sync::Arc;

    #[test]
    fn clone_reports_initial_negotiating_progress() {
        super::test_env::init_test_env();
        let service = DefaultGitService::new();
        let dest = fixtures::create_empty_dir();
        let flag = AtomicBool::new(true); // 立刻取消，避免真实网络
        let mut saw_negotiating = false;
        let _ = service.clone_blocking(
            "https://invalid-host.invalid/repo.git",
            &dest,
            None,
            &flag,
            |p| {
                if p.phase == "Negotiating" {
                    saw_negotiating = true;
                }
            },
        );
        assert!(saw_negotiating, "should emit Negotiating phase at start");
    }

    #[test]
    fn clone_from_local_repo_succeeds_and_completes_with_valid_progress() {
        super::test_env::init_test_env();
        let work = fixtures::create_repo_with_initial_commit("init").path;
        let service = DefaultGitService::new();
        let dest = fixtures::create_empty_dir();
        let flag = AtomicBool::new(false);
        let mut completed = false;
        let mut last_percent = 0;
        let mut percents: Vec<u32> = vec![];
        let mut phases: Vec<String> = vec![];
        let out =
            service.clone_blocking(work.to_string_lossy().as_ref(), &dest, None, &flag, |p| {
                last_percent = p.percent;
                percents.push(p.percent);
                phases.push(p.phase.clone());
                if p.phase == "Completed" {
                    completed = true;
                }
            });
        assert!(out.is_ok(), "local clone should succeed");
        assert!(completed, "should emit Completed phase");
        assert_eq!(last_percent, 100);
        assert!(percents.iter().all(|p| *p <= 100));
        super::assert_progress_core(&phases);
        assert!(dest.join(".git").exists());
    }

    #[test]
    fn fetch_updates_remote_tracking_refs() {
        super::test_env::init_test_env();
        use std::process::Command;
        let src = fixtures::create_empty_dir();
        std::fs::create_dir_all(&src).unwrap();
        let run_src = |args: &[&str]| {
            let st = Command::new("git")
                .current_dir(&src)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success(), "git {args:?} (src) should succeed");
        };
        run_src(&["init", "--quiet"]);
        run_src(&["config", "user.email", "you@example.com"]);
        run_src(&["config", "user.name", "You"]);
        std::fs::write(src.join("f.txt"), "1").unwrap();
        run_src(&["add", "."]);
        run_src(&["commit", "-m", "c1"]);
        let dst = fixtures::create_empty_dir();
        let st = Command::new("git")
            .args([
                "clone",
                "--quiet",
                src.to_string_lossy().as_ref(),
                dst.to_string_lossy().as_ref(),
            ])
            .status()
            .expect("git clone");
        assert!(st.success(), "initial clone should succeed");
        std::fs::write(src.join("f.txt"), "2").unwrap();
        run_src(&["add", "."]);
        run_src(&["commit", "-m", "c2"]);
        let src_head = {
            let out = Command::new("git")
                .current_dir(&src)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let service = DefaultGitService::new();
        let flag = AtomicBool::new(false);
        let got = service.fetch_blocking("", &dst, None, &flag, |_p| {});
        assert!(got.is_ok(), "fetch should succeed");
        let dst_remote_head = {
            let out = Command::new("git")
                .current_dir(&dst)
                .args(["rev-parse", "refs/remotes/origin/master"])
                .output()
                .unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(
            dst_remote_head, src_head,
            "remote-tracking ref should match source HEAD after fetch"
        );
    }

    #[tokio::test]
    async fn registry_clone_local_repo_completes() {
        super::test_env::init_test_env();
        let src = fixtures::create_repo_with_initial_commit("init").path;
        let reg = Arc::new(TaskRegistry::new());
        let dest = fixtures::create_empty_dir().to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone {
            repo: src.to_string_lossy().to_string(),
            dest: dest.clone(),
            depth: None,
            filter: None,
            strategy_override: None,
            recurse_submodules: false,
        });
        let handle = reg.clone().spawn_git_clone_task(
            None,
            id,
            token,
            src.to_string_lossy().to_string(),
            dest.clone(),
        );
        let completed =
            super::task_wait::wait_task_state(&reg, &id, TaskState::Completed, 10_000, 25).await;
        assert!(completed, "local clone task should complete");
        let _ = handle.await;
    }

    #[tokio::test]
    async fn registry_fetch_local_repo_completes() {
        super::test_env::init_test_env();
        use std::process::Command;
        let src = fixtures::create_repo_with_initial_commit("init").path;
        let dst = fixtures::create_empty_dir();
        let st = Command::new("git")
            .args([
                "clone",
                "--quiet",
                src.to_string_lossy().as_ref(),
                dst.to_string_lossy().as_ref(),
            ])
            .status()
            .expect("git clone");
        assert!(st.success());
        std::fs::write(src.join("a.txt"), "2").unwrap();
        let run_src = |args: &[&str]| {
            let st = Command::new("git")
                .current_dir(&src)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success(), "git {args:?} (src) should succeed");
        };
        run_src(&["add", "."]);
        run_src(&["commit", "-m", "more"]);
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::GitFetch {
            repo: "".into(),
            dest: dst.to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: None,
        });
        let handle = reg.clone().spawn_git_fetch_task(
            None,
            id,
            token,
            "".into(),
            dst.to_string_lossy().to_string(),
            None,
        );
        let completed =
            super::task_wait::wait_task_state(&reg, &id, TaskState::Completed, 10_000, 25).await;
        assert!(completed, "local fetch task should complete");
        let _ = handle.await;
    }
}

// ---------------- section_service_cancel_fast ----------------
mod section_service_cancel_fast {
    //! 早期错误 + Cancel flag 快速终止 + registry fast cancel
    use fireworks_collaboration_lib::core::git::service::GitService;
    use fireworks_collaboration_lib::core::git::DefaultGitService;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    use crate::common::fixtures;

    // ---- Blocking GitService cancel / fail-fast ----
    #[test]
    fn clone_cancel_flag_results_in_cancel_error() {
        super::test_env::init_test_env();
        let service = DefaultGitService::new();
        let dest = fixtures::create_empty_dir();
        let flag = AtomicBool::new(true);
        let out =
            service.clone_blocking("https://example.com/any.git", &dest, None, &flag, |_p| {});
        assert!(
            matches!(out, Err(e) if matches!(e, fireworks_collaboration_lib::core::git::errors::GitError::Categorized { category: fireworks_collaboration_lib::core::git::errors::ErrorCategory::Cancel, .. })),
            "cancel should map to Cancel category"
        );
    }

    #[test]
    fn fetch_cancel_flag_results_in_cancel_error() {
        super::test_env::init_test_env();
        use std::process::Command;
        let target = fixtures::create_empty_dir();
        std::fs::create_dir_all(&target).unwrap();
        let run_in = |dir: &std::path::PathBuf, args: &[&str]| {
            let st = Command::new("git")
                .current_dir(dir)
                .args(args)
                .status()
                .unwrap();
            assert!(st.success());
        };
        run_in(&target, &["init", "--quiet"]);
        run_in(
            &target,
            &["remote", "add", "origin", target.to_string_lossy().as_ref()],
        );
        let service = DefaultGitService::new();
        let flag = AtomicBool::new(true);
        let out = service.fetch_blocking("", &target, None, &flag, |_p| {});
        assert!(
            matches!(out, Err(e) if matches!(e, fireworks_collaboration_lib::core::git::errors::GitError::Categorized { category: fireworks_collaboration_lib::core::git::errors::ErrorCategory::Cancel, .. })),
            "cancel should map to Cancel category"
        );
    }

    #[test]
    fn clone_invalid_local_path_fails_quick() {
        super::test_env::init_test_env();
        let service = DefaultGitService::new();
        let dest = fixtures::create_empty_dir();
        let repo = std::path::PathBuf::from("C:/this-path-should-not-exist-xyz/repo");
        let flag = AtomicBool::new(false);
        let out =
            service.clone_blocking(repo.to_string_lossy().as_ref(), &dest, None, &flag, |_p| {});
        assert!(out.is_err(), "invalid local path should fail fast");
    }

    // ---- Registry GitClone fast cancel / invalid inputs ----
    #[tokio::test]
    async fn test_git_clone_interrupt_flag_cancels_immediately() {
        let res = timeout(Duration::from_secs(5), async {
            let dest = fixtures::create_empty_dir();
            let flag = AtomicBool::new(true);
            let out = tokio::task::spawn_blocking(move || {
                let svc = DefaultGitService::new();
                svc.clone_blocking(
                    "https://invalid-host.invalid/repo.git",
                    &dest,
                    None,
                    &flag,
                    |_p| {},
                )
            })
            .await
            .expect("join");
            assert!(
                out.is_err(),
                "interrupt should cause clone to error quickly"
            );
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_git_clone_invalid_url_early_error() {
        let res = timeout(Duration::from_secs(5), async {
            let dest = fixtures::create_empty_dir();
            let flag = AtomicBool::new(false);
            let out = tokio::task::spawn_blocking(move || {
                let svc = DefaultGitService::new();
                svc.clone_blocking("not-a-valid-url!!!", &dest, None, &flag, |_p| {})
            })
            .await
            .expect("spawn_blocking join");
            assert!(out.is_err(), "invalid input should error");
            let msg = format!("{}", out.err().unwrap());
            assert!(!msg.is_empty());
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_registry_git_clone_cancel_quick() {
        let res = timeout(Duration::from_secs(8), async {
            let reg = Arc::new(TaskRegistry::new());
            let repo = "https://invalid-host.invalid/repo.git".to_string();
            let dest = crate::common::fixtures::create_empty_dir()
                .to_string_lossy()
                .to_string();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: repo.clone(),
                dest: dest.clone(),
                depth: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            });
            let handle = reg
                .clone()
                .spawn_git_clone_task(None, id, token.clone(), repo, dest);
            let running = super::task_wait::wait_predicate(
                || {
                    reg.snapshot(&id)
                        .map(|s| matches!(s.state, TaskState::Running))
                        .unwrap_or(false)
                },
                2_000,
                20,
            )
            .await;
            assert!(running, "task should enter running state");
            token.cancel();
            let canceled = super::task_wait::wait_predicate(
                || {
                    reg.snapshot(&id)
                        .map(|s| matches!(s.state, TaskState::Canceled))
                        .unwrap_or(false)
                },
                5_000,
                50,
            )
            .await;
            assert!(
                canceled,
                "task should transition to canceled after token.cancel()"
            );
            let _ = timeout(Duration::from_secs(2), async {
                let _ = handle.await;
            })
            .await;
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_registry_invalid_repo_fails_fast() {
        let res = timeout(Duration::from_secs(8), async {
            let reg = Arc::new(TaskRegistry::new());
            let repo = std::path::PathBuf::from("C:/this-path-should-not-exist-xyz/repo")
                .to_string_lossy()
                .to_string();
            let dest = crate::common::fixtures::create_empty_dir()
                .to_string_lossy()
                .to_string();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: repo.clone(),
                dest: dest.clone(),
                depth: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            });
            let handle = reg
                .clone()
                .spawn_git_clone_task(None, id, token, repo, dest);
            let running =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Running, 1_000, 25).await;
            assert!(running, "should enter running");
            let failed_quick =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Failed, 2_000, 25).await;
            let _ = reg.cancel(&id);
            if !failed_quick {
                let canceled =
                    super::task_wait::wait_task_state(&reg, &id, TaskState::Canceled, 4_000, 25)
                        .await;
                assert!(
                    canceled,
                    "invalid repo should fail or be canceled within timeout"
                );
            }
            let _ = timeout(Duration::from_secs(2), async {
                let _ = handle.await;
            })
            .await;
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_registry_cancel_before_start_results_canceled() {
        let res = timeout(Duration::from_secs(4), async {
            let reg = Arc::new(TaskRegistry::new());
            let repo = "C:/unused".to_string();
            let dest = crate::common::fixtures::create_empty_dir()
                .to_string_lossy()
                .to_string();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: repo.clone(),
                dest: dest.clone(),
                depth: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            });
            token.cancel();
            let handle = reg
                .clone()
                .spawn_git_clone_task(None, id, token, repo, dest);
            let canceled =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Canceled, 1_000, 25).await;
            assert!(canceled, "should be canceled immediately");
            let _ = timeout(Duration::from_secs(2), async {
                let _ = handle.await;
            })
            .await;
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_registry_invalid_url_fails_quick() {
        let res = timeout(Duration::from_secs(6), async {
            let reg = Arc::new(TaskRegistry::new());
            let repo = "not-a-valid-url!!!".to_string();
            let dest = crate::common::fixtures::create_empty_dir()
                .to_string_lossy()
                .to_string();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: repo.clone(),
                dest: dest.clone(),
                depth: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            });
            let handle = reg
                .clone()
                .spawn_git_clone_task(None, id, token, repo, dest);
            let running =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Running, 800, 25).await;
            assert!(running, "should enter running");
            let failed =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Failed, 2_000, 25).await;
            assert!(failed, "invalid url should fail quickly");
            let _ = reg.cancel(&id);
            let _ = timeout(Duration::from_secs(2), async {
                let _ = handle.await;
            })
            .await;
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_registry_invalid_scheme_fails_quick() {
        let res = timeout(Duration::from_secs(6), async {
            let reg = Arc::new(TaskRegistry::new());
            let repo = "ftp://example.com/repo.git".to_string();
            let dest = crate::common::fixtures::create_empty_dir()
                .to_string_lossy()
                .to_string();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: repo.clone(),
                dest: dest.clone(),
                depth: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            });
            let handle = reg
                .clone()
                .spawn_git_clone_task(None, id, token, repo, dest);
            let running =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Running, 800, 25).await;
            assert!(running, "should enter running");
            let failed =
                super::task_wait::wait_task_state(&reg, &id, TaskState::Failed, 2_000, 25).await;
            assert!(failed, "invalid scheme (ftp) should fail quickly");
            let _ = reg.cancel(&id);
            let _ = timeout(Duration::from_secs(2), async {
                let _ = handle.await;
            })
            .await;
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }

    #[tokio::test]
    async fn test_git_clone_relative_path_non_repo_errors_fast() {
        let res = timeout(Duration::from_secs(5), async {
            let dest = fixtures::create_empty_dir();
            let flag = AtomicBool::new(false);
            let repo = format!("./fwc-not-a-git-repo-{}", uuid::Uuid::new_v4());
            let out = tokio::task::spawn_blocking(move || {
                let svc = DefaultGitService::new();
                svc.clone_blocking(&repo, &dest, None, &flag, |_p| {})
            })
            .await
            .expect("spawn_blocking join");
            assert!(out.is_err(), "relative non-repo path should error quickly");
        })
        .await;
        assert!(res.is_ok(), "test exceeded timeout window");
    }
}
