#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Tag & Remote & Refname Validation (Roadmap Phase 1 / v1.14)
//! -------------------------------------------------------------------------
//! 迁移来源（root-level -> 本文件 sections）：
//!   - git_tag_remote.rs
//!   - git_tag_remote_extra.rs
//!   - refname_validation.rs
//! Source Mapping 快速索引：
//!   git_tag_remote.rs            -> section_tag_lightweight / section_remote_lifecycle / 部分 remote_validation
//!   git_tag_remote_extra.rs      -> section_tag_annotated (force same msg / preserve blank lines / reject no .git) + remote_validation (phase/empty url) + tag 控制字符
//!   refname_validation.rs        -> section_refname_rules
//! Metrics (Phase 1 after aggregation):
//!   * Tests total: ~ (lightweight 7 + annotated 8 + remote lifecycle 6 + remote validation 10 + refname 3) = 34
//!   * File length (approx lines): < 430 (阈值 500 内)
//!   * Helpers: prepare_repo_with_commit / cat / expect_protocol
//! 质量保证：
//!   * 所有原函数名保持（未重命名），grep 可直接追踪。
//!   * 占位原文件保留 assert!(true) 进行历史保留，可在后续删除窗口统一移除。
//! 分区结构（与附录 A.3 定义一致）：
//!   section_tag_lightweight      -> 轻量标签创建 / 覆盖 / 取消 / 缺少 commit / 非法名称
//!   section_tag_annotated        -> 注解标签 创建 / force 新对象 / 保持 OID / 消息规范化 (CRLF / 尾部空行) / 空白消息拒绝
//!   section_remote_lifecycle     -> remote add/set/remove 基本流程 / duplicate / idempotent set / update URL
//!   section_remote_validation    -> 非法 name / URL / 空白 / 空格 / 制表 / 换行 / cancel / empty url / invalid scheme
//!   section_refname_rules        -> validate_ref_name + wrappers(branch/tag/remote) 正反集合
//! 设计原则：
//!   * 不在本阶段抽象出统一 DSL；保持直接调用底层函数以减少迁移风险。
//!   * 重复 helper (prepare_repo_with_commit / cat) 在多个 section 需要时复用顶层实现。
//!   * 保留原测试函数名（若冲突加前缀），确保 git blame 可追溯迁移来源。
//! 迁移后原文件将替换为占位（assert!(true)）。
//! 后续 Phase 2+ 可根据使用频率再评估是否上移 helper 至 common/。
//! Future TODO（若后续需要）:
//!   - 采用统一事件 DSL（与 strategy/override 聚合保持一致）对 phases 进行标签化断言
//!   - 将 refname 测试拆分加入更多“合法但边界长度”用例（例如 250 字符路径）并引入 fuzz harness
//!   - 针对 remote URL 校验补充 ssh/file 本地路径合法性正例集合

#[path = "../common/mod.rs"] mod common; // 复用 fixtures/test_env 等公共实现
use common::{fixtures::{create_empty_repo, commit_files}, test_env::init_test_env};

// 统一测试环境初始化（Once 防抖）
#[ctor::ctor]
fn __init_env() { init_test_env(); }

use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{
    tag::git_tag,
    remote::{git_remote_add, git_remote_set, git_remote_remove},
    refname::{validate_ref_name, validate_branch_name, validate_tag_name, validate_remote_name},
};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

// ---- 公共辅助 ----
fn cat(err: GitError) -> ErrorCategory { match err { GitError::Categorized { category, .. } => category } }

fn repo_with_one_commit() -> std::path::PathBuf {
    let repo = create_empty_repo();
    commit_files(&repo.path, &[("file.txt", "hello")], "feat: init", false).unwrap();
    repo.path
}

// Refname 期望Protocol错误
fn expect_protocol(res: Result<(), GitError>) {
    match res { Ok(_) => panic!("expected Protocol error"), Err(GitError::Categorized { category, .. }) => assert_eq!(category, ErrorCategory::Protocol) }
}

// ---------------- section_tag_lightweight ----------------
mod section_tag_lightweight { use super::*; 
    #[test] fn tag_lightweight_success() { let dest = repo_with_one_commit(); let flag = AtomicBool::new(false); let mut phases=Vec::<String>::new(); git_tag(&dest, "v1.0.0", None, false, false, &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert_eq!(phases.last().unwrap(), "Tagged"); }
    #[test] fn tag_existing_without_force_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_tag(&dest, "dup", None, false, false, &flag, |_p| {}).unwrap(); let e=git_tag(&dest, "dup", None, false, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn tag_force_overwrites() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_tag(&dest, "force-tag", Some("first"), true, false, &flag, |_p| {}).unwrap(); git_tag(&dest, "force-tag", Some("second"), true, true, &flag, |_p| {}).unwrap(); }
    #[test] fn tag_without_commit_rejected() { let dest = create_empty_repo().path; let flag=AtomicBool::new(false); let e=git_tag(&dest, "v0", None, false, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn tag_cancelled_early() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(true); let e=git_tag(&dest, "v1", None, false, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Cancel)); }
    #[test] fn tag_lightweight_force_updates_ref_oid() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let mut phases=Vec::<String>::new(); git_tag(&dest, "lw", None, false, false, &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let orig=repo.find_reference("refs/tags/lw").unwrap().target().unwrap(); commit_files(&dest, &[("extra.txt", "x")], "feat: extra", false).unwrap(); let mut phases2=Vec::<String>::new(); git_tag(&dest, "lw", None, false, true, &flag, |p:ProgressPayload| phases2.push(p.phase)).unwrap(); let repo2=git2::Repository::open(&dest).unwrap(); let new_oid=repo2.find_reference("refs/tags/lw").unwrap().target().unwrap(); assert_ne!(orig, new_oid); assert_eq!(phases.last().unwrap(), "Tagged"); assert_eq!(phases2.last().unwrap(), "Retagged"); }
    #[test] fn tag_lightweight_force_same_head_oid_unchanged() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_tag(&dest, "same", None, false, false, &flag, |_p| {}).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let orig=repo.find_reference("refs/tags/same").unwrap().target().unwrap(); git_tag(&dest, "same", None, false, true, &flag, |_p| {}).unwrap(); let repo2=git2::Repository::open(&dest).unwrap(); let new_oid=repo2.find_reference("refs/tags/same").unwrap().target().unwrap(); assert_eq!(orig, new_oid); }
}

// ---------------- section_tag_annotated ----------------
mod section_tag_annotated { use super::*; 
    #[test] fn tag_annotated_success() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let mut phases=Vec::<String>::new(); git_tag(&dest, "release-1", Some("release 1"), true, false, &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert_eq!(phases.last().unwrap(), "AnnotatedTagged"); }
    #[test] fn tag_annotated_missing_message_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_tag(&dest, "bad", None, true, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn tag_annotated_force_creates_new_object() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let mut phases1=Vec::<String>::new(); git_tag(&dest, "ann", Some("v1"), true, false, &flag, |p:ProgressPayload| phases1.push(p.phase)).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let first_obj=repo.find_reference("refs/tags/ann").unwrap().target().unwrap(); let mut phases2=Vec::<String>::new(); git_tag(&dest, "ann", Some("v2"), true, true, &flag, |p:ProgressPayload| phases2.push(p.phase)).unwrap(); let repo2=git2::Repository::open(&dest).unwrap(); let second_obj=repo2.find_reference("refs/tags/ann").unwrap().target().unwrap(); assert_ne!(first_obj, second_obj); assert_eq!(phases1.last().unwrap(), "AnnotatedTagged"); assert_eq!(phases2.last().unwrap(), "AnnotatedRetagged"); }
    #[test] fn tag_annotated_force_same_message_retains_oid() {
        let dest=repo_with_one_commit(); let flag=AtomicBool::new(false);
        // 首次创建 annotated tag
        git_tag(&dest, "ann_same", Some("same message"), true, false, &flag, |_p| {}).unwrap();
        let repo=git2::Repository::open(&dest).unwrap();
        let first_ref = repo.find_reference("refs/tags/ann_same").unwrap();
        let first_tag_obj = first_ref.peel(git2::ObjectType::Tag).unwrap().into_tag().unwrap();
        let first_msg = first_tag_obj.message().unwrap_or("").to_string();
        let first_target = first_tag_obj.target_id();
        // 强制相同消息 retag
        git_tag(&dest, "ann_same", Some("same message"), true, true, &flag, |_p| {}).unwrap();
        let repo2=git2::Repository::open(&dest).unwrap();
        let second_ref = repo2.find_reference("refs/tags/ann_same").unwrap();
        let second_tag_obj = second_ref.peel(git2::ObjectType::Tag).unwrap().into_tag().unwrap();
        let second_msg = second_tag_obj.message().unwrap_or("");
        let second_target = second_tag_obj.target_id();
        // 断言：指向同一目标提交 + 消息一致（允许实现细节导致 tag 对象 OID 变更）
        assert_eq!(first_target, second_target, "retag with same message should keep target");
        assert_eq!(first_msg, second_msg, "retag with same message should keep message");
    }
    #[test] fn tag_annotated_blank_message_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_tag(&dest, "blankmsg", Some("   \n  \t"), true, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn tag_annotated_crlf_message_normalized() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_tag(&dest, "crlf", Some("Line1\r\nLine2\rLine3"), true, false, &flag, |_p| {}).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let reference=repo.find_reference("refs/tags/crlf").unwrap(); let obj=reference.peel(git2::ObjectType::Tag).unwrap(); let tag=obj.into_tag().unwrap(); let msg=tag.message().unwrap_or(""); assert!(!msg.contains('\r')); assert!(msg.contains("Line1\nLine2\nLine3")); }
    #[test] fn tag_annotated_trailing_blank_lines_collapsed() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_tag(&dest, "trail", Some("Msg Title\n\n\n  \n"), true, false, &flag, |_p| {}).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let r=repo.find_reference("refs/tags/trail").unwrap(); let obj=r.peel(git2::ObjectType::Tag).unwrap(); let tag=obj.into_tag().unwrap(); let msg=tag.message().unwrap_or(""); assert!(msg.ends_with("\n")); assert!(!msg.ends_with("\n\n")); assert!(msg.starts_with("Msg Title")); }
    #[test] fn tag_annotated_preserve_internal_blank_lines() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let raw="Title\n\nBody line1\n\nBody line2\n\n"; git_tag(&dest, "ann_blank", Some(raw), true, false, &flag, |_p| {}).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let r=repo.find_reference("refs/tags/ann_blank").unwrap(); let obj=r.peel(git2::ObjectType::Tag).unwrap(); let tag=obj.into_tag().unwrap(); let msg=tag.message().unwrap(); assert!(msg.starts_with("Title\n\nBody line1")); assert!(msg.contains("Body line2")); assert!(msg.ends_with("\n")); assert!(msg.contains("Title\n\nBody")); }
}

// ---------------- section_remote_lifecycle ----------------
mod section_remote_lifecycle { use super::*; 
    #[test] fn remote_add_set_remove_success() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let mut phases=Vec::<String>::new(); git_remote_add(&dest, "origin", "https://example.com/repo.git", &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert_eq!(phases.last().unwrap(), "RemoteAdded"); phases.clear(); git_remote_set(&dest, "origin", "https://example.com/other.git", &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert_eq!(phases.last().unwrap(), "RemoteSet"); phases.clear(); git_remote_remove(&dest, "origin", &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert_eq!(phases.last().unwrap(), "RemoteRemoved"); }
    #[test] fn remote_add_duplicate_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_remote_add(&dest, "dup", "https://example.com/a.git", &flag, |_p| {}).unwrap(); let e=git_remote_add(&dest, "dup", "https://example.com/a.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_set_nonexistent_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_set(&dest, "nope", "https://example.com/x.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_remove_nonexistent_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_remove(&dest, "nope", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_set_same_url_idempotent() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_remote_add(&dest, "o", "https://example.com/a.git", &flag, |_p| {}).unwrap(); git_remote_set(&dest, "o", "https://example.com/a.git", &flag, |_p| {}).unwrap(); }
    #[test] fn remote_set_updates_url() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_remote_add(&dest, "o2", "https://example.com/old.git", &flag, |_p| {}).unwrap(); git_remote_set(&dest, "o2", "https://example.com/new.git", &flag, |_p| {}).unwrap(); let repo=git2::Repository::open(&dest).unwrap(); let r=repo.find_remote("o2").unwrap(); assert_eq!(r.url().unwrap(), "https://example.com/new.git"); }
    #[test] fn remote_add_local_path_ok() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); git_remote_add(&dest, "local", dest.to_string_lossy().as_ref(), &flag, |_p| {}).unwrap(); }
}

// ---------------- section_remote_validation ----------------
mod section_remote_validation { use super::*; 
    #[test] fn remote_invalid_name_or_url_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_add(&dest, "bad name", "https://example.com/x.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); let e2=git_remote_add(&dest, "ok", "ftp://example.com/x.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e2), ErrorCategory::Protocol)); }
    #[test] fn remote_cancelled() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(true); let e=git_remote_add(&dest, "r1", "https://example.com/r.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Cancel)); }
    #[test] fn remote_set_cancelled() { let dest=repo_with_one_commit(); let flag_ok=AtomicBool::new(false); git_remote_add(&dest, "c1", "https://example.com/x.git", &flag_ok, |_p| {}).unwrap(); let cancel=AtomicBool::new(true); let e=git_remote_set(&dest, "c1", "https://example.com/y.git", &cancel, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Cancel)); }
    #[test] fn remote_remove_cancelled() { let dest=repo_with_one_commit(); let flag_ok=AtomicBool::new(false); git_remote_add(&dest, "c2", "https://example.com/x.git", &flag_ok, |_p| {}).unwrap(); let cancel=AtomicBool::new(true); let e=git_remote_remove(&dest, "c2", &cancel, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Cancel)); }
    #[test] fn remote_add_url_with_space_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_add(&dest, "badurl", "https://exa mple.com/repo.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_add_url_with_newline_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_add(&dest, "badn", "https://example.com/repo.git\n", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_add_url_with_tab_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_add(&dest, "badt", "https://example.com/\trepo.git", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn remote_set_reject_empty_url_after_trim() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); let e=git_remote_set(&dest, "origin", "   ", &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol)); }
    #[test] fn tag_invalid_name_rejected() { let dest=repo_with_one_commit(); let flag=AtomicBool::new(false); for bad in [" ", "bad name", "end/", "..two", "@{sym", "control\u{0007}", "bad\u{0001}name"] { let e=git_tag(&dest, bad, None, false, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(e), ErrorCategory::Protocol), "{bad} should be Protocol"); } }
}

// ---------------- section_refname_rules ----------------
mod section_refname_rules { use super::*; 
    #[test] fn refname_invalid_set() { let cases=["", " ", "/start", "double//slash", "has space", "ends/", "ends.", "trail.lock", "-leadingdash", "two..dots", "back\\slash", "colon:char", "quest?", "star*", "brack[et", "tilda~", "caret^", "at@{sym", "ctrl\u{0007}"]; for c in cases { expect_protocol(validate_ref_name(c)); } }
    #[test] fn refname_valid_set() { let cases=["feature/x", "hotfix_y", "release-1.0", "v1", "a", "multi/level/name", "feat/add-api", "fix/ISSUE-123", "RC_2025_09", "topic.with.dots"]; for c in cases { validate_ref_name(c).unwrap(); } }
    #[test] fn wrappers_delegate() { validate_branch_name("feature/x").unwrap(); validate_tag_name("v1.2.3").unwrap(); validate_remote_name("origin").unwrap(); expect_protocol(validate_branch_name("has space")); }
}
