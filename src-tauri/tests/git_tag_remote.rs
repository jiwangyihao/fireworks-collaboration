#![cfg(not(feature = "tauri-app"))]
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add, commit::git_commit, tag::git_tag, remote::{git_remote_add, git_remote_set, git_remote_remove}};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_repo() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-tag-remote-{}", uuid::Uuid::new_v4())) }
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
fn tag_lightweight_success() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases = Vec::<String>::new();
    git_tag(&dest, "v1.0.0", None, false, false, &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "Tagged");
}

#[test]
fn tag_annotated_success() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases = Vec::<String>::new();
    git_tag(&dest, "release-1", Some("release 1"), true, false, &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "AnnotatedTagged");
}

#[test]
fn tag_existing_without_force_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_tag(&dest, "dup", None, false, false, &flag, |_p| {}).unwrap();
    let e = git_tag(&dest, "dup", None, false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn tag_force_overwrites() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_tag(&dest, "force-tag", Some("first"), true, false, &flag, |_p| {}).unwrap();
    // second annotated with different message force overwrite
    git_tag(&dest, "force-tag", Some("second"), true, true, &flag, |_p| {}).unwrap();
}

#[test]
fn tag_annotated_missing_message_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_tag(&dest, "bad", None, true, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn tag_invalid_name_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    for bad in [" ", "bad name", "end/", "..two", "@{sym", "control\u{0007}"] { // include some invalid patterns
        let e = git_tag(&dest, bad, None, false, false, &flag, |_p| {}).unwrap_err();
        assert!(matches!(cat(e), ErrorCategory::Protocol), "{bad} should be Protocol");
    }
}

#[test]
fn tag_without_commit_rejected() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    let e = git_tag(&dest, "v0", None, false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn tag_cancelled_early() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(true); // already cancelled
    let e = git_tag(&dest, "v1", None, false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Cancel));
}

// ===== Remote tests =====
#[test]
fn remote_add_set_remove_success() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases = Vec::<String>::new();
    git_remote_add(&dest, "origin", "https://example.com/repo.git", &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "RemoteAdded");
    phases.clear();
    git_remote_set(&dest, "origin", "https://example.com/other.git", &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "RemoteSet");
    phases.clear();
    git_remote_remove(&dest, "origin", &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert_eq!(phases.last().unwrap(), "RemoteRemoved");
}

#[test]
fn remote_add_duplicate_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_remote_add(&dest, "dup", "https://example.com/a.git", &flag, |_p| {}).unwrap();
    let e = git_remote_add(&dest, "dup", "https://example.com/a.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_set_nonexistent_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_remote_set(&dest, "nope", "https://example.com/x.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_remove_nonexistent_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_remote_remove(&dest, "nope", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_invalid_name_or_url_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    // invalid name
    let e = git_remote_add(&dest, "bad name", "https://example.com/x.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
    // invalid url scheme
    let e2 = git_remote_add(&dest, "ok", "ftp://example.com/x.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e2), ErrorCategory::Protocol));
}

#[test]
fn remote_cancelled() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(true);
    let e = git_remote_add(&dest, "r1", "https://example.com/r.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Cancel));
}

#[test]
fn remote_set_same_url_idempotent() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_remote_add(&dest, "o", "https://example.com/a.git", &flag, |_p| {}).unwrap();
    // set same URL should succeed
    git_remote_set(&dest, "o", "https://example.com/a.git", &flag, |_p| {}).unwrap();
}

#[test]
fn remote_set_updates_url() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_remote_add(&dest, "o2", "https://example.com/old.git", &flag, |_p| {}).unwrap();
    git_remote_set(&dest, "o2", "https://example.com/new.git", &flag, |_p| {}).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let r = repo.find_remote("o2").unwrap();
    assert_eq!(r.url().unwrap(), "https://example.com/new.git");
}

#[test]
fn remote_add_local_path_ok() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    // Use dest itself as a path style remote (not validating existence strongly here, just pattern)
    git_remote_add(&dest, "local", dest.to_string_lossy().as_ref(), &flag, |_p| {}).unwrap();
}

#[test]
fn remote_add_url_with_space_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_remote_add(&dest, "badurl", "https://exa mple.com/repo.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_add_url_with_newline_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_remote_add(&dest, "badn", "https://example.com/repo.git\n", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_add_url_with_tab_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_remote_add(&dest, "badt", "https://example.com/\trepo.git", &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn remote_set_cancelled() {
    let dest = prepare_repo_with_commit();
    let flag_ok = AtomicBool::new(false);
    git_remote_add(&dest, "c1", "https://example.com/x.git", &flag_ok, |_p| {}).unwrap();
    let cancel = AtomicBool::new(true);
    let e = git_remote_set(&dest, "c1", "https://example.com/y.git", &cancel, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Cancel));
}

#[test]
fn remote_remove_cancelled() {
    let dest = prepare_repo_with_commit();
    let flag_ok = AtomicBool::new(false);
    git_remote_add(&dest, "c2", "https://example.com/x.git", &flag_ok, |_p| {}).unwrap();
    let cancel = AtomicBool::new(true);
    let e = git_remote_remove(&dest, "c2", &cancel, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Cancel));
}

// ===== Additional enhanced tests (force & OID changes) =====

#[test]
fn tag_lightweight_force_updates_ref_oid() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    // initial tag
    let mut phases = Vec::<String>::new();
    git_tag(&dest, "lw", None, false, false, &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap();
    // record original oid
    let repo = git2::Repository::open(&dest).unwrap();
    let orig = repo.find_reference("refs/tags/lw").unwrap().target().unwrap();
    // create a new commit so HEAD changes
    std::fs::write(dest.join("extra.txt"), "x").unwrap();
    git_add(&dest, &["extra.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: extra", None, false, &flag, |_p| {}).unwrap();
    let mut phases2 = Vec::<String>::new();
    git_tag(&dest, "lw", None, false, true, &flag, |p:ProgressPayload| phases2.push(p.phase)).unwrap();
    let repo2 = git2::Repository::open(&dest).unwrap();
    let new_oid = repo2.find_reference("refs/tags/lw").unwrap().target().unwrap();
    assert_ne!(orig, new_oid, "force lightweight tag should move ref to new HEAD");
    assert_eq!(phases.last().unwrap(), "Tagged");
    assert_eq!(phases2.last().unwrap(), "Retagged");
}

#[test]
fn tag_annotated_force_creates_new_object() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let mut phases1 = Vec::<String>::new();
    git_tag(&dest, "ann", Some("v1"), true, false, &flag, |p:ProgressPayload| phases1.push(p.phase)).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let first_obj = repo.find_reference("refs/tags/ann").unwrap().target().unwrap();
    // second force annotated (same HEAD but new message) -> new tag object
    let mut phases2 = Vec::<String>::new();
    git_tag(&dest, "ann", Some("v2"), true, true, &flag, |p:ProgressPayload| phases2.push(p.phase)).unwrap();
    let repo2 = git2::Repository::open(&dest).unwrap();
    let second_obj = repo2.find_reference("refs/tags/ann").unwrap().target().unwrap();
    assert_ne!(first_obj, second_obj, "force annotated tag should create new tag object");
    assert_eq!(phases1.last().unwrap(), "AnnotatedTagged");
    assert_eq!(phases2.last().unwrap(), "AnnotatedRetagged");
}

#[test]
fn tag_annotated_blank_message_rejected() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    let e = git_tag(&dest, "blankmsg", Some("   \n  \t"), true, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn tag_lightweight_force_same_head_oid_unchanged() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_tag(&dest, "same", None, false, false, &flag, |_p| {}).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let orig = repo.find_reference("refs/tags/same").unwrap().target().unwrap();
    // force again without new commit -> OID should stay
    git_tag(&dest, "same", None, false, true, &flag, |_p| {}).unwrap();
    let repo2 = git2::Repository::open(&dest).unwrap();
    let new_oid = repo2.find_reference("refs/tags/same").unwrap().target().unwrap();
    assert_eq!(orig, new_oid, "force with identical HEAD should not change OID for lightweight tag");
}

#[test]
fn tag_annotated_crlf_message_normalized() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_tag(&dest, "crlf", Some("Line1\r\nLine2\rLine3"), true, false, &flag, |_p| {}).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let reference = repo.find_reference("refs/tags/crlf").unwrap();
    let obj = reference.peel(git2::ObjectType::Tag).unwrap();
    let tag = obj.into_tag().unwrap();
    let msg = tag.message().unwrap_or("");
    assert!(!msg.contains('\r'), "message should have CR removed");
    assert!(msg.contains("Line1\nLine2\nLine3"), "message should normalize CRLF and CR to LF");
}

#[test]
fn tag_annotated_trailing_blank_lines_collapsed() {
    let dest = prepare_repo_with_commit();
    let flag = AtomicBool::new(false);
    git_tag(&dest, "trail", Some("Msg Title\n\n\n  \n"), true, false, &flag, |_p| {}).unwrap();
    let repo = git2::Repository::open(&dest).unwrap();
    let r = repo.find_reference("refs/tags/trail").unwrap();
    let obj = r.peel(git2::ObjectType::Tag).unwrap();
    let tag = obj.into_tag().unwrap();
    let msg = tag.message().unwrap_or("");
    // 应只有一个结尾换行且末尾没有多余空行
    assert!(msg.ends_with("\n"));
    assert!(!msg.ends_with("\n\n"));
    assert!(msg.starts_with("Msg Title"));
}
