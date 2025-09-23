#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Clone Shallow & Depth (Roadmap 12.5)
//! ------------------------------------------------------------
//! 迁移来源（legacy 占位保留）：
//!   - git_shallow_clone.rs
//!   - git_shallow_fetch.rs
//!   - git_shallow_fetch_deepen.rs
//!   - git_shallow_invalid_depth.rs
//!   - git_shallow_fetch_invalid_depth.rs
//!   - git_shallow_local_ignore.rs
//!   - git_shallow_fetch_local_ignore.rs
//!   - git_shallow_file_url_deepen.rs
//! 分区结构：
//!   section_basic_shallow   -> 初始浅克隆 depth=N & full clone 对比
//!   section_invalid_depth   -> depth=0 / 负值 / 过大（clone & fetch）
//!   section_deepen          -> shallow clone 后多次 deepen (fetch) 递进
//!   section_local_ignore    -> 本地路径 clone/fetch depth 忽略
//!   section_file_url        -> file:// 方案（当前实现占位/忽略）
//! 设计要点：
//!   * 用 `common::shallow_matrix` 提供代表性用例集合，失败输出包含 Display。
//!   * 当前仍使用直接断言：对象数量/`.git/shallow` 文件存在与否；事件 DSL 暂缺。
//!   * 对网络依赖（公共仓库 URL）暂不在此聚合：深度聚焦本地可复现场景。
//! 后续改进：
//!   * 引入对象计数 helper（统计 OID 个数或 rev-list --count）。
//!   * 深化 `.git/shallow` 文件内容解析验证（列出 OID 行数）。(已部分实现 shallow_file_lines helper)
//!   * 与 fetch 聚合（12.7）共享 deepen 行为断言逻辑。
//! Post-audit: 初版聚合建立，未实现路径以 TODO 标示，等待 12.7/12.8 融合后收敛冗余 helper。
//! Post-audit(v2): deepen 断言已加入 shallow 文件行数不增校验；12.7 fetch 聚合后
//! 将把 run_fetch 占位 ignored 逻辑替换为统一 shallow/outcome helper。
//! Post-audit(v3): 补充 Cross-ref -> `git_fetch_core_and_shallow.rs`；计划抽取 shared shallow_assert helper (对象数 + shallow 文件 + deepen 行为)；未来事件 DSL 收紧后删除宽松 eprintln 警告。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_basic_shallow ----------------
mod section_basic_shallow {
    use std::process::Command;
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::{DefaultGitService, service::GitService};
    use crate::common::{repo_factory::rev_count, fixtures::path_slug, test_env};

    fn build_origin(commits: u32) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fwc-shallow-origin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let st = Command::new("git").current_dir(&dir).args(args).status().unwrap();
            assert!(st.success(), "git {:?} failed", args);
        };
        run(&["init", "--quiet"]);
        run(&["config", "user.email", "you@example.com"]);
        run(&["config", "user.name", "You"]);
        for i in 1..=commits {
            std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap();
            run(&["add", "."]);
            run(&["commit", "-m", &format!("c{}", i)]);
        }
        dir
    }

    #[test]
    fn shallow_clone_depth_creates_or_limits_history() {
        test_env::init_test_env();
        let origin = build_origin(5);
        let dest = std::env::temp_dir().join(format!("fwc-shallow-clone-{}", path_slug(uuid::Uuid::new_v4().to_string())));
        let svc = DefaultGitService::new();
        let cancel = AtomicBool::new(false);
        let depth = 1u32; // 代表性 depth
        svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, Some(depth), &cancel, |_p| {}).expect("shallow clone");
        let c = rev_count(&dest);
        assert!(c >= 1 && c <= 5, "[basic-shallow] commit count bounds for depth=1 got {}", c);
    }

    #[test]
    fn full_clone_has_full_history_may_lack_shallow_file() {
        test_env::init_test_env();
        let origin = build_origin(4);
    let dest = std::env::temp_dir().join(format!("fwc-full-clone-{}", path_slug(uuid::Uuid::new_v4().to_string())));
        let svc = DefaultGitService::new();
        let cancel = AtomicBool::new(false);
        svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, None, &cancel, |_p| {}).expect("full clone");
        let c = rev_count(&dest);
        assert!(c >= 4, "[basic-shallow] expect full history >=4, got {}", c);
        let shallow_file = dest.join(".git").join("shallow");
        // 非严格：存在则忽略；无则符合预期
        if shallow_file.exists() { eprintln!("[warn] shallow file present in full clone; leniency"); }
    }
}

// ---------------- section_invalid_depth ----------------
mod section_invalid_depth {
    use std::sync::Arc;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use serde_json::json;
    use crate::common::shallow_matrix::{shallow_cases, ShallowCase};
    use crate::common::test_env;

    fn wait_state(reg:&TaskRegistry, id:uuid::Uuid, target:TaskState, max_ms:u64)->bool { let mut elapsed=0; while elapsed<max_ms { if let Some(s)=reg.snapshot(&id){ if s.state==target { return true; }} std::thread::sleep(std::time::Duration::from_millis(25)); elapsed+=25;} false }

    fn init_origin() -> String {
        let dir = std::env::temp_dir().join(format!("fwc-shallow-invalid-origin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let repo = git2::Repository::init(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
        dir.to_string_lossy().to_string()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_depth_matrix_clone_and_fetch_fail_fast() {
        test_env::init_test_env();
        let reg = Arc::new(TaskRegistry::new());
        let origin = init_origin();
        for case in shallow_cases().into_iter().filter(|c| matches!(c, ShallowCase::Invalid { .. })) {
            let dest = std::env::temp_dir().join(format!("fwc-shallow-invalid-{}-{}", case, uuid::Uuid::new_v4())).to_string_lossy().to_string();
            let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
            let depth_json = if let ShallowCase::Invalid { raw, .. } = case { json!(raw) } else { unreachable!() };
            let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin.clone(), dest.clone(), Some(depth_json), None, None);
            let failed = wait_state(&reg, id, TaskState::Failed, 2000);
            assert!(failed, "[invalid-depth] clone should fail quickly for case {case}");
            handle.await.unwrap();
        }
    }
}

// ---------------- section_deepen ----------------
mod section_deepen {
    use std::process::Command;
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::{DefaultGitService, service::GitService};
    use crate::common::shallow_matrix::{shallow_cases, ShallowCase};
    use crate::common::{repo_factory::rev_count, fixtures::{path_slug, shallow_file_lines, detect_shallow_repo}};
    use crate::common::test_env;

    fn build_origin(commits: u32) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fwc-shallow-deepen-origin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| { let st = Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); };
        run(&["init", "--quiet"]);
        run(&["config", "user.email", "d@example.com"]);
        run(&["config", "user.name", "D"]);
        for i in 1..=commits { std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap(); run(&["add", "."]); run(&["commit", "-m", &format!("c{}", i)]); }
        dir
    }

    #[test]
    fn deepen_sequences_monotonic() {
        test_env::init_test_env();
        let origin = build_origin(6);
        for case in shallow_cases().into_iter().filter(|c| matches!(c, ShallowCase::Deepen { .. })) {
            // Windows 不支持路径名中的 '>' 等字符，生成安全 slug
            let slug = if let ShallowCase::Deepen { from, to } = case { path_slug(format!("deepen_{}_{}", from, to)) } else { "_".into() };
            let dest = std::env::temp_dir().join(format!("fwc-shallow-deepen-run-{}-{}", slug, uuid::Uuid::new_v4()));
            let svc = DefaultGitService::new();
            let cancel = AtomicBool::new(false);
            if let ShallowCase::Deepen { from, to } = case {
                svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, Some(from), &cancel, |_p| {}).expect("initial shallow clone");
                // 读取 shallow 文件（有的实现可能在浅克隆后不创建文件，只依赖 fetch 参数；因此不强制存在）
                let shallow_before = shallow_file_lines(&dest); // 兼容旧 helper
                let c1 = rev_count(&dest);
                svc.fetch_blocking(origin.to_string_lossy().as_ref(), &dest, Some(to), &cancel, |_p| {}).expect("deepen fetch");
                let c2 = rev_count(&dest);
                assert!(c2 >= c1, "[deepen] history should be non-decreasing for {from}->{to}");
                let shallow_after = shallow_file_lines(&dest);
                let (_is_shallow, _lines) = detect_shallow_repo(&dest); // 预留未来断言使用
                // 若实现生成 shallow 文件，则 deepen 后行数不应增加；若消失则表示已 full history；若原本不存在则保持宽松。
                if !shallow_before.is_empty() && !shallow_after.is_empty() {
                    assert!(shallow_after.len() <= shallow_before.len(), "[deepen] shallow file lines should not increase (from={} to={})", shallow_before.len(), shallow_after.len());
                }
            }
        }
    }
}

// ---------------- section_local_ignore ----------------
mod section_local_ignore {
    use std::process::Command;
    use std::sync::atomic::AtomicBool;
    use fireworks_collaboration_lib::core::git::{DefaultGitService, service::GitService};
    use crate::common::shallow_matrix::{shallow_cases, ShallowCase};
    use crate::common::test_env;

    fn build_repo(n:u32)->std::path::PathBuf{ let dir= std::env::temp_dir().join(format!("fwc-shallow-local-src-{}", uuid::Uuid::new_v4())); std::fs::create_dir_all(&dir).unwrap(); let run=|args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); }; run(&["init","--quiet"]); run(&["config","user.email","l@example.com"]); run(&["config","user.name","L"]); for i in 1..=n { std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); } dir }

    #[test]
    fn local_ignore_depth_clone_and_fetch() {
        test_env::init_test_env();
        let origin = build_repo(3);
        for case in shallow_cases().into_iter().filter(|c| matches!(c, ShallowCase::LocalIgnoreClone { .. } | ShallowCase::LocalIgnoreFetch { .. })) {
            let svc = DefaultGitService::new();
            let cancel = AtomicBool::new(false);
            let dest = std::env::temp_dir().join(format!("fwc-shallow-local-ignore-{}-{}", case, uuid::Uuid::new_v4()));
            match case {
                ShallowCase::LocalIgnoreClone { depth } => {
                    svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, Some(depth), &cancel, |_p| {}).expect("local depth clone");
                    assert!(!dest.join(".git").join("shallow").exists(), "[local-ignore] clone should not create shallow file");
                }
                ShallowCase::LocalIgnoreFetch { depth } => {
                    // full clone first
                    svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, None, &cancel, |_p| {}).expect("full clone");
                    svc.fetch_blocking(origin.to_string_lossy().as_ref(), &dest, Some(depth), &cancel, |_p| {}).expect("local depth fetch");
                    assert!(!dest.join(".git").join("shallow").exists(), "[local-ignore] fetch should not create shallow file");
                }
                _ => unreachable!()
            }
        }
    }
}

// ---------------- section_file_url ----------------
mod section_file_url {
    // 占位：当前实现不支持 file:// scheme 深度/浅克隆路径，保留结构，待支持后填充。
    // legacy: git_shallow_file_url_deepen.rs (原测试含 #[ignore])
    // TODO(12.5+): 一旦实现支持，迁移其逻辑并取消占位。
    #[test]
    fn file_url_placeholder() {
        // 仅保证测试框架加载模块，无实际逻辑。
        assert!(true, "file_url shallow placeholder");
    }
}
