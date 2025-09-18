#![cfg(not(feature = "tauri-app"))]
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, commit::git_commit, branch::git_branch, checkout::git_checkout, add::git_add};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_repo() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-branch-{}", uuid::Uuid::new_v4())) }
fn cat(err: GitError) -> ErrorCategory { match err { GitError::Categorized { category, .. } => category } }

#[test]
fn branch_create_and_checkout_success() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // prepare initial commit so branch has HEAD
    std::fs::write(dest.join("a.txt"), "hi").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: init", None, false, &flag, |_p| {}).unwrap();
    // create branch without checkout
    let mut phases = Vec::<String>::new();
    git_branch(&dest, "feature/x", false, false, &flag, |p: ProgressPayload| phases.push(p.phase)).unwrap();
    assert!(phases.last().unwrap().contains("Branched"));
    // checkout branch
    git_checkout(&dest, "feature/x", false, &flag, |_p| {}).unwrap();
}

#[test]
fn branch_conflict_without_force() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("f.txt"), "hi").unwrap();
    git_add(&dest, &["f.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: base", None, false, &flag, |_p| {}).unwrap();
    git_branch(&dest, "dup", false, false, &flag, |_p| {}).unwrap();
    let e = git_branch(&dest, "dup", false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn branch_force_moves() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("1.txt"), "1").unwrap();
    git_add(&dest, &["1.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: first", None, false, &flag, |_p| {}).unwrap();
    git_branch(&dest, "force-test", false, false, &flag, |_p| {}).unwrap();
    // new commit
    std::fs::write(dest.join("2.txt"), "2").unwrap();
    git_add(&dest, &["2.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: second", None, false, &flag, |_p| {}).unwrap();
    // force move
    git_branch(&dest, "force-test", false, true, &flag, |_p| {}).unwrap();
}

#[test]
fn checkout_nonexistent_without_create_fails() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: base", None, false, &flag, |_p| {}).unwrap();
    let e = git_checkout(&dest, "no-such", false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Protocol));
}

#[test]
fn checkout_create_success() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: base", None, false, &flag, |_p| {}).unwrap();
    git_checkout(&dest, "new-branch", true, &flag, |_p| {}).unwrap();
}

#[test]
fn branch_and_checkout_cancelled() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(true); // already canceled
    let e = git_branch(&dest, "x", false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(e), ErrorCategory::Cancel));
}

#[test]
fn branch_invalid_names_rejected() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // prepare a commit so non-empty repo
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: base", None, false, &flag, |_p| {}).unwrap();
    for bad in [" ", "a b", "end/", "dot.", "-lead", "a..b", "a\\b"] {
        let err = git_branch(&dest, bad, false, false, &flag, |_p| {}).unwrap_err();
        assert!(matches!(cat(err), ErrorCategory::Protocol), "expect Protocol for bad name {bad}");
    }
}

#[test]
fn branch_creation_without_commit_rejected() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    let err = git_branch(&dest, "feature/a", false, false, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(err), ErrorCategory::Protocol));
}

#[test]
fn branch_force_without_commit_rejected() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    let err = git_branch(&dest, "main", false, true, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(err), ErrorCategory::Protocol));
}

#[test]
fn checkout_cancel_during_operation() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "feat: base", None, false, &flag, |_p| {}).unwrap();
    git_branch(&dest, "dev", false, false, &flag, |_p| {}).unwrap();
    // now cancel just before checkout
    let cancel_flag = AtomicBool::new(true);
    let err = git_checkout(&dest, "dev", false, &cancel_flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(err), ErrorCategory::Cancel));
}

#[test]
fn branch_force_move_updates_ref() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // first commit
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
    git_branch(&dest, "move", false, false, &flag, |_p| {}).unwrap();
    // second commit
    std::fs::write(dest.join("b.txt"), "b").unwrap();
    git_add(&dest, &["b.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "c2", None, false, &flag, |_p| {}).unwrap();
    // record head commit id
    let repo = git2::Repository::open(&dest).unwrap();
    let new_head = repo.head().unwrap().target().unwrap();
    git_branch(&dest, "move", false, true, &flag, |_p| {}).unwrap();
    // verify branch ref now points to new head
    let repo2 = git2::Repository::open(&dest).unwrap();
    let br = repo2.find_branch("move", git2::BranchType::Local).unwrap();
    let tgt = br.into_reference().target().unwrap();
    assert_eq!(tgt, new_head, "force move should update branch ref");
}

#[test]
fn branch_valid_names_succeed_and_phase_emitted() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // prepare one commit
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
    let valids = ["feature/one", "hotfix-123", "refs_ok/level", "abc", "long.name-seg"]; // (refs/ prefix avoided intentionally)
    for v in valids { let mut phases = Vec::new(); git_branch(&dest, v, false, false, &flag, |p:ProgressPayload| phases.push(p.phase)).unwrap(); assert!(phases.last().unwrap().starts_with("Branched"), "expected Branched phase for {v}"); }
}

#[test]
fn branch_new_invalid_additional_cases() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // one commit
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
    // new invalid set hitting extended rules
    let invalids = ["/start", "double//slash", "end.lock", "have:colon", "quest?", "star*", "brack[et", "tilda~", "caret^", "at@{sym", "ctrl\u{0007}bell"];
    for bad in invalids { let err = git_branch(&dest, bad, false, false, &flag, |_p| {}).unwrap_err(); assert!(matches!(cat(err), ErrorCategory::Protocol), "{bad} should be Protocol"); }
}

#[test]
fn checkout_create_on_existing_branch_noop_like() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("a.txt"), "a").unwrap();
    git_add(&dest, &["a.txt"], &flag, |_p| {}).unwrap();
    git_commit(&dest, "c1", None, false, &flag, |_p| {}).unwrap();
    git_branch(&dest, "dev", false, false, &flag, |_p| {}).unwrap();
    // create=true when already exists should behave like normal checkout (our impl path is existing branch case; create ignored)
    git_checkout(&dest, "dev", true, &flag, |_p| {}).unwrap();
}

#[test]
fn checkout_create_without_commit_rejected() {
    let dest = tmp_repo();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // repo has no commit
    let err = git_checkout(&dest, "newbranch", true, &flag, |_p| {}).unwrap_err();
    assert!(matches!(cat(err), ErrorCategory::Protocol));
}
