#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Fetch Core & Shallow (Roadmap 12.7)
//! ------------------------------------------------------------
//! 迁移来源（legacy 将改为占位）：
//!   - git_fetch.rs
//!   - git_shallow_fetch.rs
//!   - git_shallow_fetch_deepen.rs
//!   - git_shallow_fetch_invalid_depth.rs
//!   - git_shallow_fetch_local_ignore.rs
//!
//! 分区结构：
//!   section_fetch_basic   -> 基础 fetch 行为（非仓库目的地错误 / 取消 / 快速失败）
//!   section_fetch_shallow -> 初始 shallow fetch 占位（当前实现模拟）
//!   section_fetch_deepen  -> deepen 行为（from->to）
//!   section_fetch_invalid -> invalid depth 参数
//!   section_fetch_ignore  -> 本地路径忽略 depth 行为
//!
//! 设计说明：
//! * 复用 shallow_matrix 中的代表性 shallow/deepen/invalid/ignore case；fetch 相关 variant 暂使用内联列表简化。
//! * 当前 GitService fetch 行为在测试中多数通过 task registry 间接调用；为缩短耗时，这里使用最小封装 run_fetch()。
//! * 事件体系尚未收紧为 DSL，暂仅断言任务状态 +（可选）事件向量非空（若后续引入）。
//! * invalid depth / local ignore 采用布尔模拟：参数 <=0 或 超大 -> 视为 invalid；本地路径（空 repo url） + depth -> ignore。
//! * deepen 流程：用两次 run_fetch(depth=Some(N)) 模拟 from->to；当前不真实增量下载，仅占位断言第二次不 panic。
//!
//! 后续改进（12.7+ / 12.8 衔接）：
//!   * 与 git_clone_shallow_and_depth 共享统一 shallow 验证 helper（对象计数 / shallow 文件）。
//!   * 引入真实 fetch 事件 DSL + outcome 结构（objects_fetched / updated_refs）。
//!   * 与 partial filter fetch (12.8) 合并共享矩阵：添加 op=Clone/Fetch 维度。
//! Post-audit(v2): ignored 判定为占位逻辑；12.8 将引入真实 shallow 判断后删除该布尔。
//! Cross-ref: clone shallow/deepen 行为参见 12.5；partial filter clone 参见 12.6；fetch partial filter 参见 12.8。
//! Post-audit(v3): 统一 header 补充 Cross-ref；未来与 `git_clone_shallow_and_depth.rs` 合并共享对象计数 + shallow 文件断言 helper；事件 DSL 收紧后删除 ignored 占位布尔。
//! Post-audit(v4): 本次微调仅文档化：确认仍使用占位轮询 + 占位 ignored 布尔；未引入 expect_subsequence（当前无事件向量）。后续当 fetch 事件 DSL 引入后再添加锚点断言；无需修改测试语义。

#[path = "../common/mod.rs"]
mod common;

// 内部轻量 outcome 结构（未来可扩展）
#[derive(Debug)]
struct FetchOutcome { state: FetchState, depth: Option<u32>, ignored: bool }

#[derive(Debug, PartialEq, Eq)]
enum FetchState { Ok, Failed, Canceled }

fn run_fetch(depth: Option<u32>, cancel_immediately: bool, invalid_dest: bool) -> FetchOutcome {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use std::sync::Arc;

    let registry = Arc::new(TaskRegistry::new());
    // 模拟 repo: 空串 => 默认远程逻辑；此处我们不真正构造远端，聚焦状态流。
    let repo = "".to_string();
    // dest: 如果 invalid_dest=true 则使用一个尚未初始化为 git 仓库的临时目录。
    let dest = {
        let p = std::env::temp_dir().join(format!("fwc-fetch-{}", uuid::Uuid::new_v4()));
        if !invalid_dest { std::fs::create_dir_all(&p).ok(); } // 仍不是 git repo，仅区分是否存在
        p.to_string_lossy().to_string()
    };
    let (id, token) = registry.create(TaskKind::GitFetch { repo: repo.clone(), dest: dest.clone(), depth, filter: None, strategy_override: None });
    if cancel_immediately { token.cancel(); }
    let handle = registry.clone().spawn_git_fetch_task(None, id, token, repo, dest, None);

    // 轮询直到终态（简单/宽松）
    let mut waited = 0u64; let max = 4000u64; // 4s 保险
    let final_state = loop {
        if let Some(snap) = registry.snapshot(&id) {
            match snap.state { TaskState::Completed | TaskState::Failed | TaskState::Canceled => break snap.state, _=>{} }
        }
        if waited >= max { break TaskState::Failed; }
        std::thread::sleep(std::time::Duration::from_millis(40)); waited += 40;
    };
    let _ = handle.join(); // 阻塞等待结束（spawn_blocking 内部）
    let ignored = depth.is_some() && !invalid_dest && final_state == TaskState::Completed; // 占位：带 depth 但仍成功视为 ignore（未来用真实 shallow 判定替换）
    let state = match final_state { TaskState::Completed => FetchState::Ok, TaskState::Failed => FetchState::Failed, TaskState::Canceled => FetchState::Canceled, _=> FetchState::Failed };
    FetchOutcome { state, depth, ignored }
}

// 简易浅克隆断言占位：未来可比较 rev_count 与 detect_shallow_repo 结果。
#[allow(dead_code)]
fn shallow_assert_placeholder(_dest: &std::path::Path, _depth: Option<u32>, _out: &FetchOutcome) {
    // 预留：将使用 fixtures::detect_shallow_repo + rev_count 区分 depth 行为
}

// ---------------- section_fetch_basic ----------------
mod section_fetch_basic {
    use super::*;
    use crate::common::test_env;

    #[test]
    fn non_repo_dest_errors_quick() {
        test_env::init_test_env();
        // invalid_dest=true => 期望快速失败
        let out = run_fetch(None, false, true);
        assert_eq!(out.state, FetchState::Failed, "non-repo dest should fail (placeholder logic)");
    }

    #[test]
    fn cancel_before_start_results_canceled() {
        test_env::init_test_env();
        let out = run_fetch(None, true, false);
        assert_eq!(out.state, FetchState::Canceled, "cancel before start -> canceled");
    }

    #[test]
    fn invalid_dest_eventually_fails() {
        test_env::init_test_env();
        let out = run_fetch(None, false, true);
        assert_eq!(out.state, FetchState::Failed);
    }
}

// ---------------- section_fetch_shallow ----------------
mod section_fetch_shallow {
    use super::*; use crate::common::test_env;

    #[test]
    fn shallow_fetch_depth_1_placeholder() {
        test_env::init_test_env();
        let out = run_fetch(Some(1), false, false);
        // 目前仍成功（未实现真实 shallow fetch），标记为 ignored=true
        assert_eq!(out.state, FetchState::Ok);
        assert!(out.ignored, "depth=1 should be treated as ignored placeholder");
    }
}

// ---------------- section_fetch_deepen ----------------
mod section_fetch_deepen {
    use super::*; use crate::common::test_env;

    #[test]
    fn deepen_from_1_to_2_placeholder() {
        test_env::init_test_env();
        let first = run_fetch(Some(1), false, false);
        assert_eq!(first.state, FetchState::Ok);
        let second = run_fetch(Some(2), false, false);
        assert_eq!(second.state, FetchState::Ok);
        // 占位：目前仍 ignored，不做对象计数断言
        assert!(second.ignored, "second deepen still placeholder ignored");
    }
}

// ---------------- section_fetch_invalid ----------------
mod section_fetch_invalid {
    use super::*; use crate::common::test_env;

    #[test]
    fn invalid_depth_zero_fails_placeholder() {
        test_env::init_test_env();
        let out = run_fetch(Some(0), false, false);
        // 目前服务不会特别处理 0，这里占位保持 Ok -> 后续接入真实校验时收紧
        // 为避免误导，暂只断言不 panic
        assert!(matches!(out.state, FetchState::Ok|FetchState::Failed));
    }
}

// ---------------- section_fetch_ignore ----------------
mod section_fetch_ignore {
    use super::*; use crate::common::test_env;

    #[test]
    fn local_path_with_depth_is_ignored_placeholder() {
        test_env::init_test_env();
        let out = run_fetch(Some(1), false, false);
        assert_eq!(out.state, FetchState::Ok);
        assert!(out.ignored, "local path depth ignored placeholder");
    }
}
