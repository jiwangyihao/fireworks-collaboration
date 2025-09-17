use std::{fs, path::PathBuf};

use uuid::Uuid;
use fireworks_collaboration_lib::core::git::default_impl::commit::{git_commit, Author};

fn temp_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fwc_commit_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn git_commit_success_and_empty_reject() {
    // disable e2e net
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    // init repository via direct git2
    let repo = git2::Repository::init(&repo_dir).expect("init");
    // create a file and add to index
    let file_path = repo_dir.join("a.txt");
    fs::write(&file_path, b"hello").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();

    // call commit impl directly
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    git_commit(&repo_dir, "feat: add a.txt", None, false, &interrupt, |_p| {}).expect("commit ok");

    // second attempt without changes should be rejected (empty commit)
    let err = git_commit(&repo_dir, "chore: empty", None, false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).contains("empty commit"), "expect empty commit rejection");

    // allowing empty commit should succeed
    git_commit(&repo_dir, "chore: force empty", None, true, &interrupt, |_p| {}).expect("empty commit allowed");
}

#[test]
fn git_commit_requires_message() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let _repo = git2::Repository::init(&repo_dir).unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let err = git_commit(&repo_dir, "   \n", None, false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).contains("commit message is empty"));
}

#[test]
fn git_commit_cancel_before() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let _repo = git2::Repository::init(&repo_dir).unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(true); // immediately canceled
    let err = git_commit(&repo_dir, "feat: test", None, false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).to_lowercase().contains("cancel"));
}

#[test]
fn git_commit_with_custom_author() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    // create file
    std::fs::write(repo_dir.join("x.txt"), b"x").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("x.txt")).unwrap();
    index.write().unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    git_commit(&repo_dir, "feat: custom author", Some(Author { name: Some("Alice"), email: Some("alice@example.com") }), false, &interrupt, |_p| {}).expect("custom author commit");
}

#[test]
fn git_commit_author_missing_email_rejected() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("y.txt"), b"y").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("y.txt")).unwrap();
    index.write().unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let err = git_commit(&repo_dir, "feat: missing email", Some(Author { name: Some("Bob"), email: None }), false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).contains("author name/email required"));
}

#[test]
fn git_commit_initial_empty_repo_reject_and_allow() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let _repo = git2::Repository::init(&repo_dir).unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    // no staged changes: reject when allow_empty=false
    let err = git_commit(&repo_dir, "feat: nothing", None, false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).contains("empty commit"));
    // allow empty commit
    git_commit(&repo_dir, "feat: empty allowed", None, true, &interrupt, |_p| {}).expect("empty commit allowed at initial repo");
}

#[test]
fn git_commit_author_empty_string_rejected() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("f.txt"), b"f").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("f.txt")).unwrap();
    index.write().unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let err = git_commit(&repo_dir, "feat: invalid author", Some(Author { name: Some(""), email: Some("a@b.c") }), false, &interrupt, |_p| {}).unwrap_err();
    assert!(format!("{}", err).contains("author name/email required"));
}

#[test]
fn git_commit_message_trimming() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("g.txt"), b"g").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("g.txt")).unwrap();
    index.write().unwrap();
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    git_commit(&repo_dir, "  feat: trim  \n", None, false, &interrupt, |_p| {}).expect("trimmed commit ok");
    // verify last commit message
    let repo2 = git2::Repository::open(&repo_dir).unwrap();
    let head = repo2.head().unwrap();
    let commit = repo2.find_commit(head.target().unwrap()).unwrap();
    assert_eq!(commit.message().unwrap().trim(), "feat: trim");
}
