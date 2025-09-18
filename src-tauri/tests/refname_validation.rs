use fireworks_collaboration_lib::core::git::default_impl::refname::{validate_ref_name, validate_branch_name, validate_tag_name, validate_remote_name};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};

fn expect_protocol(res: Result<(), GitError>) {
    match res {
        Ok(_) => panic!("expected Protocol error"),
        Err(GitError::Categorized { category, .. }) => assert_eq!(category, ErrorCategory::Protocol),
    }
}

#[test]
fn refname_invalid_set() {
    let cases = [
        "", " ", "/start", "double//slash", "has space", "ends/", "ends.", "trail.lock", "-leadingdash", "two..dots", "back\\slash", "colon:char", "quest?", "star*", "brack[et", "tilda~", "caret^", "at@{sym", "ctrl\u{0007}",
    ];
    for c in cases { expect_protocol(validate_ref_name(c)); }
}

#[test]
fn refname_valid_set() {
    let cases = [
        "feature/x", "hotfix_y", "release-1.0", "v1", "a", "multi/level/name", "feat/add-api", "fix/ISSUE-123", "RC_2025_09", "topic.with.dots",
    ];
    for c in cases { validate_ref_name(c).unwrap(); }
}

#[test]
fn wrappers_delegate() {
    // Currently wrappers identical; ensure they don't diverge unexpectedly.
    validate_branch_name("feature/x").unwrap();
    validate_tag_name("v1.2.3").unwrap();
    validate_remote_name("origin").unwrap();
    expect_protocol(validate_branch_name("has space"));
}
