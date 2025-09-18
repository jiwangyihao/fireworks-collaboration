#![cfg(not(feature = "tauri-app"))]
// Extra edge / branch coverage tests for git_tag & git_remote operations.
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add, commit::git_commit, tag::git_tag, remote::{git_remote_add, git_remote_set}};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_repo() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-tag-remote-extra-{}", uuid::Uuid::new_v4())) }
fn cat(err: GitError) -> ErrorCategory { match err { GitError::Categorized { category, .. } => category } }

fn prepare_repo_with_commit() -> std::path::PathBuf {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("file.txt"), "hello").unwrap();
    git_add(&dest, &["file.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: init", None, false, &flag, |_p| {}).unwrap();
    dest
}

#[test]
fn tag_annotated_force_same_message_retains_oid() {
    // 当 message 与目标 commit、签名均不变时，内容寻址对象 OID 相同；force 只是允许覆盖引用 -> OID 不变。
    // 我们仍应收到 AnnotatedRetagged phase。
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases1 = Vec::<String>::new();
    git_tag(&dest, "ann_same", Some("same message"), true, false, &flag, |p:ProgressPayload| phases1.push(p.phase)).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let first = repo.find_reference("refs/tags/ann_same").unwrap().target().unwrap();
    let mut phases2 = Vec::<String>::new();
    git_tag(&dest, "ann_same", Some("same message"), true, true, &flag, |p:ProgressPayload| phases2.push(p.phase)).unwrap();
    let repo2 = git2::Repository::open(&dest).unwrap();
    let second = repo2.find_reference("refs/tags/ann_same").unwrap().target().unwrap();
    assert_eq!(first, second, "identical annotated force should retain same tag object oid");
    assert_eq!(phases1.last().unwrap(), "AnnotatedTagged");
    assert_eq!(phases2.last().unwrap(), "AnnotatedRetagged");
}

#[test]
fn tag_annotated_preserve_internal_blank_lines() {
    // 内部空行不应被折叠，只处理尾部多余空白
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let raw = "Title\n\nBody line1\n\nBody line2\n\n"; // 末尾多余换行会被压缩为单一结尾换行
    git_tag(&dest, "ann_blank", Some(raw), true, false, &flag, |_p| {}).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let r = repo.find_reference("refs/tags/ann_blank").unwrap();
    let obj = r.peel(git2::ObjectType::Tag).unwrap();
    let tag = obj.into_tag().unwrap();
    let msg = tag.message().unwrap();
    assert!(msg.starts_with("Title\n\nBody line1"));
    assert!(msg.contains("Body line2"));
    assert!(msg.ends_with("\n"));
    // 确认内部的双空行仍保留
    assert!(msg.contains("Title\n\nBody"), "internal blank lines should remain");
}

#[test]
fn tag_reject_repository_without_dot_git_folder() {
    // 直接提供一个新建临时目录, 但不执行 git_init
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    let e = git_tag(&dest, "v1", None, false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_set_no_change_phase_and_effect() {
    // 设置同一个 URL 再次调用, 确认仍成功并使用 RemoteSet phase (原测试覆盖成功；此处额外验证 phase)
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases = Vec::<String>::new();
    git_remote_add(&dest, "o", "https://example.com/a.git", &flag, |_p| {}).unwrap();
    git_remote_set(&dest, "o", "https://example.com/a.git", &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "RemoteSet");
}

#[test]
fn remote_set_reject_empty_url_after_trim() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    // 传入纯空白, validate_remote_url 应返回 Protocol
    let e = git_remote_set(&dest, "origin", "   ", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn tag_invalid_control_chars() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let bad = "bad\u{0001}name";
    let e = git_tag(&dest, bad, None, false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}
