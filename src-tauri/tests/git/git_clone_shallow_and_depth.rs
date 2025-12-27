//! 聚合测试：Git Clone Shallow & Depth
//! -----------------------------------
//! 精简后分区：
//!   `section_basic_shallow`  -> 浅克隆 vs 全量克隆（参数化）
//!   `section_invalid_depth`  -> 非法 depth 失败（矩阵循环）
//!   `section_deepen`         -> 多步 deepen 序列单循环
//!   `section_local_ignore`   -> 本地路径忽略 depth（clone/fetch）
//! `已移除：file_url` 占位（过时占位，等待真实支持后再引入专门测试）。
//! 抽象：helpers 模块统一构建线性仓库、执行 shallow/deepen、task 等待。
//! 未来：
//!   * 增加对象计数/差异断言（替换当前非严格 >= 检查）
//!   * 事件 DSL 引入后增加最小子序列验证 shallow/deepen 行为
//!   * 与 fetch 聚合共享 deepen 逻辑（当前已局部对齐）

// ---------------- helpers ----------------
mod helpers {
    use super::super::common::{
        fixtures::{path_slug, shallow_file_lines},
        repo_factory::rev_count,
        repo_factory::RepoBuilder,
    };
    use fireworks_collaboration_lib::core::git::{service::GitService, DefaultGitService};
    use fireworks_collaboration_lib::core::tasks::model::TaskState;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    pub fn build_linear_repo(commits: u32) -> PathBuf {
        let mut b = RepoBuilder::new();
        for i in 1..=commits {
            b = b.with_commit(format!("f{i}.txt"), format!("{i}"), format!("c{i}"));
        }
        b.build().path
    }

    pub fn shallow_clone(origin: &Path, depth: Option<u32>) -> PathBuf {
        let dest = std::env::temp_dir().join(format!(
            "fwc-shallow-{}-{}",
            depth.map(|d| d.to_string()).unwrap_or("full".into()),
            path_slug(uuid::Uuid::new_v4().to_string())
        ));
        let svc = DefaultGitService::new();
        let cancel = AtomicBool::new(false);
        svc.clone_blocking(
            origin.to_string_lossy().as_ref(),
            &dest,
            depth,
            &cancel,
            |_p| {},
        )
        .expect("clone");
        dest
    }

    pub fn deepen_once(origin: &Path, dest: &Path, to: u32) {
        let svc = DefaultGitService::new();
        let cancel = AtomicBool::new(false);
        svc.fetch_blocking(
            origin.to_string_lossy().as_ref(),
            dest,
            Some(to),
            &cancel,
            |_p| {},
        )
        .expect("deepen fetch");
    }

    pub fn revs(dest: &Path) -> u32 {
        rev_count(dest)
    }
    pub fn shallow_lines(dest: &Path) -> Vec<String> {
        shallow_file_lines(dest)
    }

    pub fn wait_state(reg: &TaskRegistry, id: uuid::Uuid, target: TaskState, max_ms: u64) -> bool {
        let mut elapsed = 0;
        while elapsed < max_ms {
            if let Some(s) = reg.snapshot(&id) {
                if s.state == target {
                    return true;
                }
            }
            std::thread::sleep(Duration::from_millis(25));
            elapsed += 25;
        }
        false
    }
}

// ---------------- section_basic_shallow ----------------
mod section_basic_shallow {
    use super::helpers::*;
    use crate::common::test_env;
    #[test]
    fn shallow_vs_full_parameterized() {
        test_env::init_test_env();
        let origin = build_linear_repo(5);
        for (depth, min_commits, note) in [(Some(1u32), 1u32, "shallow"), (None, 5u32, "full")] {
            let dest = shallow_clone(&origin, depth);
            let c = revs(&dest);
            if let Some(d) = depth {
                assert!(
                    (1..=5).contains(&c),
                    "[basic:{note}] depth={d} commits bounds got {c}"
                );
            } else {
                assert!(
                    c >= min_commits,
                    "[basic:{note}] expected >= {min_commits} got {c}"
                );
            }
        }
    }
}

// ---------------- section_invalid_depth ----------------
mod section_invalid_depth {
    use super::helpers::{build_linear_repo, wait_state};
    use crate::common::shallow_matrix::{invalid_depth_cases, ShallowCase};
    use crate::common::test_env;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_depth_fail_fast() {
        test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let origin = build_linear_repo(1).to_string_lossy().to_string();
        for case in invalid_depth_cases() {
            if let ShallowCase::Invalid { raw, .. } = case {
                let dest = std::env::temp_dir()
                    .join(format!(
                        "fwc-shallow-invalid-{}-{}",
                        case.describe(),
                        uuid::Uuid::new_v4()
                    ))
                    .to_string_lossy()
                    .to_string();
                let (id, token) = reg.create(TaskKind::GitClone {
                    repo: origin.clone(),
                    dest: dest.clone(),
                    depth: None,
                    filter: None,
                    strategy_override: None,
                    recurse_submodules: false,
                });
                let handle = reg.clone().spawn_git_clone_task_with_opts(
                    None,
                    id,
                    token,
                    origin.clone(),
                    dest.clone(),
                    Some(json!(raw)),
                    None,
                    None,
                    false,
                    None,
                );
                let failed = wait_state(&reg, id, TaskState::Failed, 2000);
                assert!(failed, "[invalid-depth] expected fail-fast for {case}");
                handle.await.unwrap();
            }
        }
    }
}

// ---------------- section_deepen ----------------
mod section_deepen {
    use super::helpers::*;
    use crate::common::shallow_matrix::deepen_cases;
    use crate::common::test_env;
    #[test]
    fn deepen_sequences_monotonic() {
        test_env::init_test_env();
        let origin = build_linear_repo(6);
        for case in deepen_cases() {
            if let crate::common::shallow_matrix::ShallowCase::Deepen { from, to } = case {
                let dest = shallow_clone(&origin, Some(from));
                let shallow_before = shallow_lines(&dest);
                let c1 = revs(&dest);
                deepen_once(&origin, &dest, to);
                let c2 = revs(&dest);
                assert!(c2 >= c1, "[deepen] history non-decreasing {from}->{to}");
                let shallow_after = shallow_lines(&dest);
                if !shallow_before.is_empty() && !shallow_after.is_empty() {
                    assert!(
                        shallow_after.len() <= shallow_before.len(),
                        "[deepen] shallow file lines should not increase ({} -> {})",
                        shallow_before.len(),
                        shallow_after.len()
                    );
                }
            }
        }
    }
}

// ---------------- section_local_ignore ----------------
mod section_local_ignore {
    use super::helpers::*;
    use crate::common::shallow_matrix::{ignore_cases, ShallowCase};
    use crate::common::test_env;
    use fireworks_collaboration_lib::core::git::{service::GitService, DefaultGitService};
    use std::sync::atomic::AtomicBool;
    #[test]
    fn local_ignore_depth_behaviors() {
        test_env::init_test_env();
        let origin = build_linear_repo(3);
        for case in ignore_cases() {
            match case {
                ShallowCase::LocalIgnoreClone { depth } => {
                    let dest = shallow_clone(&origin, Some(depth));
                    assert!(
                        !dest.join(".git").join("shallow").exists(),
                        "[local-ignore] clone should not create shallow file"
                    );
                }
                ShallowCase::LocalIgnoreFetch { depth } => {
                    let dest = shallow_clone(&origin, None); // full clone
                    let svc = DefaultGitService::new();
                    let cancel = AtomicBool::new(false);
                    svc.fetch_blocking(
                        origin.to_string_lossy().as_ref(),
                        &dest,
                        Some(depth),
                        &cancel,
                        |_p| {},
                    )
                    .expect("local depth fetch");
                    assert!(
                        !dest.join(".git").join("shallow").exists(),
                        "[local-ignore] fetch should not create shallow file"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
// file_url 占位已删除：真实支持后再引入带能力检测的用例。
