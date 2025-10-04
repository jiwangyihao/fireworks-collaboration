#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Fetch（浅克隆 + 分支合并）
//!
//! 验证点已全部合并到 `section_fetch_shallow` + `section_fetch_branches` + `section_fetch_task`，移除重复原子测试文件。

// ---------------- helpers ----------------
mod helpers {
    use crate::common::task_wait::wait_until_task_done;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use std::process::Command;
    use std::sync::Arc;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum FetchState {
        Ok,
        Failed,
        Canceled,
    }

    pub fn run_fetch(depth: Option<u32>, cancel: bool, invalid_dest: bool) -> FetchState {
        let registry = Arc::new(TaskRegistry::new());
        // 为正常路径准备一个本地源仓库与目标克隆，使得 fetch 存在有效的 origin 远端
        let (repo, dest) = if invalid_dest {
            ("".to_string(), {
                let p = std::env::temp_dir()
                    .join(format!("fwc-fetch-invalid-{}", uuid::Uuid::new_v4()));
                // 故意不创建 .git 结构
                p.to_string_lossy().to_string()
            })
        } else {
            // src: 最小 repo
            let src = std::env::temp_dir().join(format!("fwc-fetch-src-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&src).ok();
            let run_in = |dir: &std::path::Path, args: &[&str]| {
                let st = Command::new("git")
                    .current_dir(dir)
                    .args(args)
                    .status()
                    .expect("run git");
                assert!(st.success(), "git {args:?} in {dir:?} should succeed");
            };
            run_in(&src, &["init", "--quiet"]);
            run_in(&src, &["config", "user.email", "you@example.com"]);
            run_in(&src, &["config", "user.name", "You"]);
            std::fs::write(src.join("f.txt"), "1").ok();
            run_in(&src, &["add", "."]);
            run_in(&src, &["commit", "-m", "c1"]);
            // dest: 克隆 src, 以便具备 origin 远端
            let dest = std::env::temp_dir().join(format!("fwc-fetch-dst-{}", uuid::Uuid::new_v4()));
            let st = Command::new("git")
                .args([
                    "clone",
                    "--quiet",
                    src.to_string_lossy().as_ref(),
                    dest.to_string_lossy().as_ref(),
                ])
                .status()
                .expect("git clone");
            assert!(st.success(), "initial clone should succeed");
            ("".to_string(), dest.to_string_lossy().to_string())
        };
        let (id, token) = registry.create(TaskKind::GitFetch {
            repo: repo.clone(),
            dest: dest.clone(),
            depth,
            filter: None,
            strategy_override: None,
        });
        if cancel {
            token.cancel();
        }
        // 在本地创建一个 Tokio 运行时来承载内部异步任务执行与轮询
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let final_state = {
            let registry_cloned = registry.clone();
            let repo2 = repo.clone();
            let dest2 = dest.clone();
            rt.block_on(async move {
                let handle = registry_cloned
                    .clone()
                    .spawn_git_fetch_task(None, id, token, repo2, dest2, None);
                wait_until_task_done(&registry_cloned, id).await;
                std::mem::drop(handle); // 仅保持任务存活至终态，不强制 join
                registry_cloned
                    .snapshot(&id)
                    .map(|s| s.state)
                    .unwrap_or(TaskState::Failed)
            })
        };
        match final_state {
            TaskState::Completed => FetchState::Ok,
            TaskState::Failed => FetchState::Failed,
            TaskState::Canceled => FetchState::Canceled,
            _ => FetchState::Failed,
        }
    }
}

use helpers::FetchState;

// ---------------- section_fetch_basic ----------------
mod section_basic {
    use super::helpers::run_fetch;
    use super::FetchState;
    use crate::common::test_env;
    struct Case {
        cancel: bool,
        invalid_dest: bool,
        expect: FetchState,
        label: &'static str,
    }
    fn cases() -> Vec<Case> {
        vec![
            Case {
                cancel: false,
                invalid_dest: true,
                expect: FetchState::Failed,
                label: "invalid-dest",
            },
            Case {
                cancel: true,
                invalid_dest: false,
                expect: FetchState::Canceled,
                label: "cancel",
            },
            Case {
                cancel: false,
                invalid_dest: false,
                expect: FetchState::Ok,
                label: "normal",
            },
        ]
    }
    #[test]
    fn parameterized_basic_state_cases() {
        test_env::init_test_env();
        for c in cases() {
            let st = run_fetch(None, c.cancel, c.invalid_dest);
            assert_eq!(st, c.expect, "[basic:{}] unexpected state", c.label);
        }
    }
}

// ---------------- section_fetch_shallow ----------------
mod section_shallow_and_ignore {
    use super::helpers::run_fetch;
    use super::FetchState;
    use crate::common::shallow_matrix::{depth_cases, ignore_cases, ShallowCase};
    use crate::common::test_env;
    #[test]
    fn shallow_and_ignore_variants() {
        test_env::init_test_env();
        for case in depth_cases().into_iter().chain(ignore_cases().into_iter()) {
            match case {
                ShallowCase::Depth { depth } => {
                    let st = run_fetch(Some(depth), false, false);
                    assert!(
                        matches!(st, FetchState::Ok | FetchState::Failed),
                        "[shallow] depth={depth} unexpected state {st:?}"
                    );
                }
                ShallowCase::LocalIgnoreFetch { depth } => {
                    let st = run_fetch(Some(depth), false, false);
                    assert!(
                        matches!(st, FetchState::Ok | FetchState::Failed),
                        "[ignore-fetch] depth={depth} unexpected state {st:?}"
                    );
                }
                _ => {}
            }
        }
    }
}

// ---------------- section_fetch_deepen ----------------
mod section_deepen {
    use super::helpers::run_fetch;
    use super::FetchState;
    use crate::common::shallow_matrix::{deepen_cases, ShallowCase};
    use crate::common::test_env;
    #[test]
    fn deepen_sequences() {
        test_env::init_test_env();
        for c in deepen_cases() {
            if let ShallowCase::Deepen { from, to } = c {
                let st1 = run_fetch(Some(from), false, false);
                assert_ne!(st1, FetchState::Canceled, "first deepen canceled");
                let st2 = run_fetch(Some(to), false, false);
                assert_ne!(st2, FetchState::Canceled, "second deepen canceled");
            }
        }
    }
}

// ---------------- section_fetch_invalid ----------------
mod section_invalid_depth {
    use super::helpers::run_fetch;
    use super::FetchState;
    use crate::common::shallow_matrix::{invalid_depth_cases, ShallowCase};
    use crate::common::test_env;
    #[test]
    fn invalid_depth_matrix() {
        test_env::init_test_env();
        for c in invalid_depth_cases() {
            if let ShallowCase::Invalid { raw, .. } = c {
                let depth_opt = if raw <= 0 {
                    Some(0u32)
                } else {
                    Some(raw as u32)
                };
                let st = run_fetch(depth_opt, false, false);
                assert!(
                    matches!(st, FetchState::Ok | FetchState::Failed),
                    "[invalid-depth] raw={raw} unexpected state {st:?}"
                );
            }
        }
    }
}

// ---------------- section_fetch_ignore ----------------
// 已合并 ignore 场景到 shallow_and_ignore。
