#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use fireworks_collaboration_lib::core::git::default_impl as impls;
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};

fn tmp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    base.join(format!("fwc-local-skel-{}", uuid::Uuid::new_v4()))
}

fn assert_protocol(e: GitError) {
    match e {
        GitError::Categorized { category: ErrorCategory::Protocol, .. } => {}
        GitError::Categorized { category, .. } => panic!("expect Protocol, got: {:?}", category),
    }
}

#[test]
fn commit_branch_checkout_tag_remote_are_not_implemented_yet() {
    let dest = tmp_dir();
    std::fs::create_dir_all(&dest).unwrap();
    let flag = AtomicBool::new(false);

    // commit
    let r = impls::commit::git_commit(&dest, "msg", None, false, &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    // branch
    let r = impls::branch::git_branch(&dest, "dev", false, false, &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    // checkout
    let r = impls::checkout::git_checkout(&dest, "dev", false, &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    // tag
    let r = impls::tag::git_tag(&dest, "v0.1.0", None, false, false, &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    // remote set/add/remove
    let r = impls::remote::git_remote_set(&dest, "origin", "https://example.com/repo.git", &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    let r = impls::remote::git_remote_add(&dest, "origin", "https://example.com/repo.git", &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());

    let r = impls::remote::git_remote_remove(&dest, "origin", &flag, |_p| {});
    assert!(r.is_err());
    assert_protocol(r.err().unwrap());
}
