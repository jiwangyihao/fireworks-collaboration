//! Git operations coverage tests for checkout and remote commands
//! Phase 5-6: Cover 0% coverage modules

use std::process::Command;
use std::sync::atomic::AtomicBool;
use tempfile::tempdir;

use fireworks_collaboration_lib::core::git::{
    default_impl::{
        checkout::git_checkout,
        remote::{git_remote_add, git_remote_remove, git_remote_set},
        tag::git_tag,
    },
    service::ProgressPayload,
};

/// Helper to create a test repo with initial commit
fn setup_test_repo() -> tempfile::TempDir {
    let temp = tempdir().unwrap();
    let repo_path = temp.path();

    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init failed");

    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email failed");

    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .expect("git config name failed");

    std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();

    Command::new("git")
        .args(&["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit failed");

    temp
}

fn noop_progress(_: ProgressPayload) {}

// ============ Checkout Tests ============

#[test]
fn test_checkout_existing_branch() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create a branch first
    Command::new("git")
        .args(&["branch", "feature-branch"])
        .current_dir(repo_path)
        .output()
        .expect("git branch failed");

    // Checkout existing branch
    let result = git_checkout(
        repo_path,
        "feature-branch",
        false,
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok(), "Checkout existing branch should succeed");

    // Verify HEAD is on the branch
    let output = Command::new("git")
        .args(&["branch", "--show-current"])
        .current_dir(repo_path)
        .output()
        .expect("git branch show failed");
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(current_branch, "feature-branch");
}

#[test]
fn test_checkout_create_new_branch() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Checkout with create=true for non-existing branch
    let result = git_checkout(repo_path, "new-feature", true, &interrupt, noop_progress);
    assert!(result.is_ok(), "Checkout with create should succeed");

    // Verify HEAD is on the new branch
    let output = Command::new("git")
        .args(&["branch", "--show-current"])
        .current_dir(repo_path)
        .output()
        .expect("git branch show failed");
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(current_branch, "new-feature");
}

#[test]
fn test_checkout_nonexistent_branch_without_create() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Checkout non-existing branch without create
    let result = git_checkout(repo_path, "nonexistent", false, &interrupt, noop_progress);
    assert!(
        result.is_err(),
        "Checkout nonexistent without create should fail"
    );
    let err = result.unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn test_checkout_empty_reference() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_checkout(repo_path, "", false, &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_checkout_reference_with_space() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_checkout(
        repo_path,
        "invalid branch",
        false,
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("space"));
}

#[test]
fn test_checkout_not_a_repo() {
    let temp = tempdir().unwrap();
    let interrupt = AtomicBool::new(false);

    let result = git_checkout(temp.path(), "main", false, &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not a git repository"));
}

#[test]
fn test_checkout_cancel_interrupts() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(true);

    let result = git_checkout(repo_path, "main", false, &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("canceled"));
}

// ============ Remote Tests ============

#[test]
fn test_remote_add_success() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_remote_add(
        repo_path,
        "upstream",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(
        result.is_ok(),
        "Remote add should succeed: {:?}",
        result.err()
    );

    // Verify remote exists
    let output = Command::new("git")
        .args(&["remote", "-v"])
        .current_dir(repo_path)
        .output()
        .expect("git remote failed");
    let remotes = String::from_utf8_lossy(&output.stdout);
    assert!(remotes.contains("upstream"));
}

#[test]
fn test_remote_add_duplicate() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Add first
    git_remote_add(
        repo_path,
        "origin",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    )
    .unwrap();

    // Try to add again
    let result = git_remote_add(
        repo_path,
        "origin",
        "https://github.com/example/other.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn test_remote_set_url() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Add remote first
    git_remote_add(
        repo_path,
        "origin",
        "https://github.com/old/repo.git",
        &interrupt,
        noop_progress,
    )
    .unwrap();

    // Update URL
    let result = git_remote_set(
        repo_path,
        "origin",
        "https://github.com/new/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok());

    // Verify URL updated
    let output = Command::new("git")
        .args(&["remote", "get-url", "origin"])
        .current_dir(repo_path)
        .output()
        .expect("git remote get-url failed");
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(url.contains("new/repo"));
}

#[test]
fn test_remote_set_nonexistent() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_remote_set(
        repo_path,
        "nonexistent",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn test_remote_remove() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Add remote first
    git_remote_add(
        repo_path,
        "upstream",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    )
    .unwrap();

    // Remove it
    let result = git_remote_remove(repo_path, "upstream", &interrupt, noop_progress);
    assert!(result.is_ok());

    // Verify removed
    let output = Command::new("git")
        .args(&["remote"])
        .current_dir(repo_path)
        .output()
        .expect("git remote failed");
    let remotes = String::from_utf8_lossy(&output.stdout);
    assert!(!remotes.contains("upstream"));
}

#[test]
fn test_remote_remove_nonexistent() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_remote_remove(repo_path, "nonexistent", &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn test_remote_add_invalid_url() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // URL with space should fail
    let result = git_remote_add(
        repo_path,
        "origin",
        "https://github.com/example/repo with space.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("whitespace"));
}

#[test]
fn test_remote_add_empty_name() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_remote_add(
        repo_path,
        "",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
}

#[test]
fn test_remote_add_scp_style_url() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // SCP-style URL should be allowed
    let result = git_remote_add(
        repo_path,
        "origin",
        "git@github.com:example/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok(), "SCP-style URL should be allowed");
}

#[test]
fn test_remote_not_a_repo() {
    let temp = tempdir().unwrap();
    let interrupt = AtomicBool::new(false);

    let result = git_remote_add(
        temp.path(),
        "origin",
        "https://github.com/example/repo.git",
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not a git repository"));
}

// ============ Tag Tests ============

#[test]
fn test_tag_lightweight_create() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_tag(
        repo_path,
        "v1.0.0",
        None,
        false, // lightweight
        false, // no force
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok(), "Create lightweight tag should succeed");

    // Verify tag exists
    let output = Command::new("git")
        .args(&["tag", "-l"])
        .current_dir(repo_path)
        .output()
        .expect("git tag list failed");
    let tags = String::from_utf8_lossy(&output.stdout);
    assert!(tags.contains("v1.0.0"));
}

#[test]
fn test_tag_annotated_create() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_tag(
        repo_path,
        "v2.0.0",
        Some("Release version 2.0"),
        true,  // annotated
        false, // no force
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok(), "Create annotated tag should succeed");

    // Verify tag is annotated
    let output = Command::new("git")
        .args(&["tag", "-n1", "v2.0.0"])
        .current_dir(repo_path)
        .output()
        .expect("git tag show failed");
    let tag_info = String::from_utf8_lossy(&output.stdout);
    assert!(tag_info.contains("Release version 2.0"));
}

#[test]
fn test_tag_annotated_requires_message() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Annotated without message should fail
    let result = git_tag(
        repo_path,
        "v3.0.0",
        None, // no message
        true, // annotated
        false,
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("non-empty message"));
}

#[test]
fn test_tag_already_exists_without_force() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create first tag
    git_tag(
        repo_path,
        "existing",
        None,
        false,
        false,
        &interrupt,
        noop_progress,
    )
    .unwrap();

    // Try to create again without force
    let result = git_tag(
        repo_path,
        "existing",
        None,
        false,
        false, // no force
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn test_tag_force_overwrite() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create first tag
    git_tag(
        repo_path,
        "force-test",
        None,
        false,
        false,
        &interrupt,
        noop_progress,
    )
    .unwrap();

    // Make a new commit
    std::fs::write(temp.path().join("NEW.md"), "new content").unwrap();
    Command::new("git")
        .args(&["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["commit", "-m", "Second commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Force overwrite the tag
    let result = git_tag(
        repo_path,
        "force-test",
        None,
        false,
        true, // force
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok(), "Force tag should succeed");
}

#[test]
fn test_tag_empty_name() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_tag(repo_path, "", None, false, false, &interrupt, noop_progress);
    assert!(result.is_err());
}

#[test]
fn test_tag_not_a_repo() {
    let temp = tempdir().unwrap();
    let interrupt = AtomicBool::new(false);

    let result = git_tag(
        temp.path(),
        "v1.0.0",
        None,
        false,
        false,
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not a git repository"));
}

#[test]
fn test_tag_cancel_interrupts() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(true);

    let result = git_tag(
        repo_path,
        "v1.0.0",
        None,
        false,
        false,
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("canceled"));
}

// ============ Git Add Tests ============

use fireworks_collaboration_lib::core::git::default_impl::add::git_add;

#[test]
fn test_add_single_file() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create a new file
    std::fs::write(repo_path.join("newfile.txt"), "new content").unwrap();

    // Add the file
    let result = git_add(repo_path, &["newfile.txt"], &interrupt, noop_progress);
    assert!(result.is_ok(), "Add single file should succeed");

    // Verify file is staged
    let output = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .expect("git status failed");
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("A  newfile.txt") || status.contains("A newfile.txt"));
}

#[test]
fn test_add_multiple_files() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create multiple files
    std::fs::write(repo_path.join("file1.txt"), "content 1").unwrap();
    std::fs::write(repo_path.join("file2.txt"), "content 2").unwrap();

    let result = git_add(
        repo_path,
        &["file1.txt", "file2.txt"],
        &interrupt,
        noop_progress,
    );
    assert!(result.is_ok());
}

#[test]
fn test_add_empty_paths() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_add(repo_path, &[], &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_add_nonexistent_file() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    let result = git_add(repo_path, &["nonexistent.txt"], &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn test_add_absolute_path_rejected() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    std::fs::write(repo_path.join("file.txt"), "content").unwrap();
    let abs_path = repo_path.join("file.txt");

    let result = git_add(
        repo_path,
        &[abs_path.to_str().unwrap()],
        &interrupt,
        noop_progress,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("absolute path"));
}

#[test]
fn test_add_empty_path_entry() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    std::fs::write(repo_path.join("file.txt"), "content").unwrap();

    let result = git_add(repo_path, &["file.txt", ""], &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("empty path"));
}

#[test]
fn test_add_directory() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(false);

    // Create a directory with files
    std::fs::create_dir(repo_path.join("subdir")).unwrap();
    std::fs::write(repo_path.join("subdir/inner.txt"), "inner content").unwrap();

    let result = git_add(repo_path, &["subdir"], &interrupt, noop_progress);
    assert!(result.is_ok(), "Add directory should succeed");
}

#[test]
fn test_add_cancel_interrupts() {
    let temp = setup_test_repo();
    let repo_path = temp.path();
    let interrupt = AtomicBool::new(true);

    std::fs::write(repo_path.join("file.txt"), "content").unwrap();

    let result = git_add(repo_path, &["file.txt"], &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("canceled"));
}

#[test]
fn test_add_not_a_repo() {
    let temp = tempdir().unwrap();
    let interrupt = AtomicBool::new(false);

    std::fs::write(temp.path().join("file.txt"), "content").unwrap();

    let result = git_add(temp.path(), &["file.txt"], &interrupt, noop_progress);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not a git repository"));
}

// ============ Opts Tests ============

use fireworks_collaboration_lib::core::git::default_impl::opts::{
    parse_depth_filter_opts, parse_strategy_override, PartialFilter,
};

#[test]
fn test_partial_filter_parse_blob_none() {
    let result = PartialFilter::parse("blob:none");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), PartialFilter::BlobNone);
}

#[test]
fn test_partial_filter_parse_tree_zero() {
    let result = PartialFilter::parse("tree:0");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), PartialFilter::TreeZero);
}

#[test]
fn test_partial_filter_parse_invalid() {
    let result = PartialFilter::parse("invalid");
    assert!(result.is_none());
}

#[test]
fn test_partial_filter_as_str() {
    assert_eq!(PartialFilter::BlobNone.as_str(), "blob:none");
    assert_eq!(PartialFilter::TreeZero.as_str(), "tree:0");
}

#[test]
fn test_parse_depth_filter_opts_empty() {
    let result = parse_depth_filter_opts(None, None, None);
    assert!(result.is_ok());
    let opts = result.unwrap();
    assert!(opts.depth.is_none());
    assert!(opts.filter.is_none());
}

#[test]
fn test_parse_depth_filter_opts_with_depth() {
    let result = parse_depth_filter_opts(Some(serde_json::json!(5)), None, None);
    assert!(result.is_ok());
    let opts = result.unwrap();
    assert_eq!(opts.depth, Some(5));
}

#[test]
fn test_parse_depth_filter_opts_with_filter() {
    let result = parse_depth_filter_opts(None, Some("blob:none".to_string()), None);
    assert!(result.is_ok());
    let opts = result.unwrap();
    assert_eq!(opts.filter, Some(PartialFilter::BlobNone));
}

#[test]
fn test_parse_depth_filter_opts_invalid_depth_negative() {
    let result = parse_depth_filter_opts(Some(serde_json::json!(-1)), None, None);
    assert!(result.is_err());
}

#[test]
fn test_parse_depth_filter_opts_invalid_depth_zero() {
    let result = parse_depth_filter_opts(Some(serde_json::json!(0)), None, None);
    assert!(result.is_err());
}

#[test]
fn test_parse_depth_filter_opts_invalid_filter() {
    let result = parse_depth_filter_opts(None, Some("invalid".to_string()), None);
    assert!(result.is_err());
}

#[test]
fn test_parse_strategy_override_empty() {
    let result = parse_strategy_override(None);
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn test_parse_strategy_override_with_http() {
    let json = serde_json::json!({
        "http": {
            "connectTimeoutMs": 5000
        }
    });
    let result = parse_strategy_override(Some(json));
    assert!(result.is_ok());
}

#[test]
fn test_parse_strategy_override_with_retry() {
    let json = serde_json::json!({
        "retry": {
            "maxAttempts": 3,
            "delayMs": 1000
        }
    });
    let result = parse_strategy_override(Some(json));
    assert!(result.is_ok());
}
