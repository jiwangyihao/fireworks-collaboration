//! Git command integration tests.
//!
//! Tests the actual command functions using MockRuntime.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::git::*;
use fireworks_collaboration_lib::app::types::{
    SharedConfig, SharedCredentialFactory, TaskRegistryState,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::runner::{Git2Runner, GitRunner};
use fireworks_collaboration_lib::core::tasks::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::TaskState;
use uuid::Uuid;

// Mock Assets for Tauri
struct MockAssets;

impl<R: tauri::Runtime> Assets<R> for MockAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Cow<'_, [u8]>)> + '_> {
        Box::new(std::iter::empty())
    }
    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, TaskRegistryState) {
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));
    let credential_factory: SharedCredentialFactory = Arc::new(Mutex::new(None));
    let runner = Box::new(Git2Runner::new()) as Box<dyn GitRunner + Send + Sync>;

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .manage::<SharedConfig>(config)
        .manage::<SharedCredentialFactory>(credential_factory)
        .manage::<Box<dyn GitRunner + Send + Sync>>(runner)
        .build(context)
        .expect("Failed to build mock app");

    (app, registry)
}

fn init_git_repo(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(path)
        .output()
        .ok();
    // Initial commit
    std::fs::write(path.join("README.md"), "# Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(path)
        .output()
        .ok();
}

#[tokio::test]
async fn test_git_clone_command() {
    let (app, registry) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("repo").to_string_lossy().to_string();
    let repo = "https://github.com/test/repo.git".to_string();

    let result = git_clone(
        repo,
        dest,
        None,
        None,
        None,
        None,
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    assert!(registry.snapshot(&uuid).is_some());
}

#[tokio::test]
async fn test_git_fetch_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_fetch(
        "origin".to_string(),
        dest,
        None,
        None,
        None,
        None,
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_init_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("new_repo").to_string_lossy().to_string();

    let result = git_init(dest.clone(), app.state(), app.handle().clone()).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_add_command() {
    let (app, registry) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    init_git_repo(&repo_path);
    let dest = repo_path.to_string_lossy().to_string();

    // Create a new untracked file
    let untracked_file = repo_path.join("untracked.txt");
    std::fs::write(&untracked_file, "new content").unwrap();

    let result = git_add(
        dest.clone(),
        vec!["untracked.txt".to_string()],
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    // Wait for task completion
    let mut completed = false;
    for _ in 0..50 {
        if let Some(snapshot) = registry.snapshot(&uuid) {
            if snapshot.state == TaskState::Completed {
                completed = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(completed, "Git add task did not complete in time");

    // Verify it is now staged using git2
    let repo = git2::Repository::open(&repo_path).unwrap();
    let status = repo
        .status_file(std::path::Path::new("untracked.txt"))
        .unwrap();
    assert!(status.contains(git2::Status::INDEX_NEW));
}

#[tokio::test]
async fn test_git_commit_command() {
    let (app, registry) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    init_git_repo(&repo_path);
    let dest = repo_path.to_string_lossy().to_string();

    // Stage a change first
    std::fs::write(repo_path.join("README.md"), "# Updated").unwrap();
    let repo = git2::Repository::open(&repo_path).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("README.md")).unwrap();
    index.write().unwrap();

    let result = git_commit(
        dest,
        "Verify Commit".to_string(),
        None,
        None,
        None,
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    // Wait for task completion
    let mut completed = false;
    for _ in 0..50 {
        if let Some(snapshot) = registry.snapshot(&uuid) {
            if snapshot.state == TaskState::Completed {
                completed = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(completed, "Git commit task did not complete in time");

    // Verify commit exists in log
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), "Verify Commit");
}

#[tokio::test]
async fn test_git_push_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    // git_push(dest, remote, refspecs, username, password, use_stored_credential, strategy_override, reg, credential_factory, app)
    let result = git_push(
        dest,
        Some("origin".to_string()),
        None, // refspecs
        None, // username
        None, // password
        None, // use_stored_credential
        None, // strategy_override
        app.state(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_branch_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_branch(
        dest,
        "new-branch".to_string(),
        Some(false),
        Some(false),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_checkout_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_checkout(
        dest,
        "main".to_string(),
        None,
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_remote_add_remove() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_remote_add(
        dest.clone(),
        "origin".to_string(),
        "https://github.com/test/repo.git".to_string(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());

    let result_remove = git_remote_remove(
        dest,
        "origin".to_string(),
        app.state(),
        app.handle().clone(),
    )
    .await;
    assert!(result_remove.is_ok());
}

#[tokio::test]
async fn test_git_list_branches_command() {
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    let (_app, _) = create_mock_app();
    // Test pure function without app state
    let result = git_list_branches(dest, None).await;

    assert!(result.is_ok());
    let branches = result.unwrap();
    assert!(!branches.is_empty());
    assert!(branches
        .iter()
        .any(|b| b.name == "master" || b.name == "main"));
}

#[tokio::test]
async fn test_git_repo_status_command() {
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    let (_app, _) = create_mock_app();
    // Test pure function
    let result = git_repo_status(dest).await;

    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(status.current_branch.is_some());
    assert!(status.is_clean);
}

#[tokio::test]
async fn test_git_repo_status_detailed_counts() {
    let temp = tempfile::tempdir().unwrap();
    let repo_path = temp.path();
    init_git_repo(repo_path);
    let dest = repo_path.to_string_lossy().to_string();

    // 1. Untracked file
    let untracked = repo_path.join("untracked.txt");
    std::fs::write(&untracked, "untracked").unwrap();

    // 2. Staged file
    let staged = repo_path.join("staged.txt");
    std::fs::write(&staged, "staged").unwrap();

    let repo = git2::Repository::open(repo_path).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("staged.txt")).unwrap();
    index.write().unwrap();

    // 3. Unstaged (Modified) file
    // Modify existing README.md which was committed by init_git_repo
    let readme = repo_path.join("README.md");
    std::fs::write(&readme, "# Modified").unwrap();

    // Check status
    let result = git_repo_status(dest).await;
    assert!(result.is_ok());
    let status = result.unwrap();

    assert_eq!(status.untracked, 1, "Should have 1 untracked file");
    assert_eq!(status.staged, 1, "Should have 1 staged file");
    assert_eq!(status.unstaged, 1, "Should have 1 unstaged file");
    assert!(!status.is_clean);
}

#[tokio::test]
async fn test_git_repo_status_ahead_behind() {
    let temp_dir = tempfile::tempdir().unwrap();
    let origin_path = temp_dir.path().join("origin");
    let local_path = temp_dir.path().join("local");

    // 1. Init bare origin
    let _ = git2::Repository::init_bare(&origin_path).unwrap();

    // 2. Clone to local
    let _ = git2::Repository::clone(origin_path.to_str().unwrap(), &local_path).unwrap();
    let local_repo = git2::Repository::open(&local_path).unwrap();

    // 3. Create initial commit and push to origin
    let file = local_path.join("file.txt");
    std::fs::write(&file, "initial").unwrap();
    let mut index = local_repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = local_repo.find_tree(tree_id).unwrap();
    let sig = local_repo.signature().unwrap();
    let commit_oid = local_repo
        .commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();

    // Push to origin
    let mut remote = local_repo.find_remote("origin").unwrap();
    remote.push(&["refs/heads/master"], None).unwrap();

    // 4. Create 2 commits locally (Ahead 2)
    for i in 1..=2 {
        std::fs::write(&file, format!("change {}", i)).unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = local_repo.find_tree(tree_id).unwrap();
        let parent = local_repo.head().unwrap().peel_to_commit().unwrap();
        local_repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit {}", i),
                &tree,
                &[&parent],
            )
            .unwrap();
    }

    // 5. Create 1 commit on origin (Behind 1)
    // We can simulate this by cloning another repo, committing and pushing,
    // OR just committing directly to origin if it wasn't bare.
    // Since it's bare, we use another clone "other"
    let other_path = temp_dir.path().join("other");
    let _ = git2::Repository::clone(origin_path.to_str().unwrap(), &other_path).unwrap();
    let other_repo = git2::Repository::open(&other_path).unwrap();
    let other_file = other_path.join("other.txt");
    std::fs::write(&other_file, "other").unwrap();
    let mut other_index = other_repo.index().unwrap();
    other_index
        .add_path(std::path::Path::new("other.txt"))
        .unwrap();
    other_index.write().unwrap();
    let other_tree_id = other_index.write_tree().unwrap();
    let other_tree = other_repo.find_tree(other_tree_id).unwrap();
    let other_parent_commit = other_repo.find_commit(commit_oid).unwrap(); // Initial commit
    other_repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Remote change",
            &other_tree,
            &[&other_parent_commit],
        )
        .unwrap();
    let mut other_remote = other_repo.find_remote("origin").unwrap();
    other_remote.push(&["refs/heads/master"], None).unwrap();

    // 6. Fetch safely in local (without merging) to update origin/master ref
    // We need to fetch to see "behind"
    let mut remote = local_repo.find_remote("origin").unwrap();
    remote.fetch(&["master"], None, None).unwrap();

    // 7. Verify Status
    // Note: Ahead 2 (local commits), Behind 1 (remote change)
    // But since local diverged from remote (both added commits from Initial),
    // it should report Ahead 2, Behind 1 correctly if the graph logic handles divergence.

    let dest = local_path.to_string_lossy().to_string();
    let result = git_repo_status(dest).await;
    assert!(result.is_ok());
    let status = result.unwrap();

    // assert_eq!(status.ahead, 2, "Should be ahead by 2");
    // assert_eq!(status.behind, 1, "Should be behind by 1");
    // Note: Due to potential race or specific graph strictness, let's verify exact counts after debug.
    // Standard git behavior:
    // Local: Init -> C1 -> C2 -> C3 (Head)
    // Remote: Init -> C1 -> C4 (Origin/Head)
    // Ideally Ahead 2 (C2, C3), Behind 1 (C4).

    // Let's print for debug if it fails, but assertions are what we want.
    if status.ahead != 2 || status.behind != 1 {
        println!(
            "Status mismatch: Ahead={}, Behind={}",
            status.ahead, status.behind
        );
    }
    assert_eq!(status.ahead, 2);
    assert_eq!(status.behind, 1);
}

#[tokio::test]
async fn test_git_worktree_ops() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    // Use a subdirectory for the repo to keep temp root clean for worktrees
    let repo_path = temp.path().join("repo");
    init_git_repo(&repo_path);
    let dest = repo_path.to_string_lossy().to_string();

    // 1. List worktrees (should be just one main one)
    let result = git_worktree_list(dest.clone()).await;
    assert!(result.is_ok());
    let wts = result.unwrap();
    assert_eq!(wts.len(), 1);

    // 2. Add worktree
    let wt_path = temp.path().join("wt1").to_string_lossy().to_string();
    let add_result = git_worktree_add(
        dest.clone(),
        wt_path.clone(),
        "new-wt-branch".to_string(),
        Some(true),
        None,
        app.state(),
    )
    .await;
    assert!(add_result.is_ok());

    // 3. List again (should be 2)
    let result2 = git_worktree_list(dest.clone()).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().len(), 2);

    // 4. Remove worktree
    let remove_result = git_worktree_remove(
        dest,
        wt_path,
        Some(true), // force
        None,
        None,
        None,
        app.state(), // credential_factory
    )
    .await;
    assert!(remove_result.is_ok());
}

#[tokio::test]
async fn test_git_remote_branches() {
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    let (_app, _) = create_mock_app();
    // No actual remote, so it might fail or return empty.
    // Testing the logic path execution.
    let result = git_remote_branches(dest, None, None).await;
    // It might return Ok(vec![]) or Err if no remote configured?
    // git branch -r on a repo with no remote returns empty output (success).
    assert!(result.is_ok());
}

// ============================================================================
// parse_git_host tests
// ============================================================================

#[test]
fn test_parse_git_host_https() {
    let result = parse_git_host("https://github.com/owner/repo.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "github.com");
}

#[test]
fn test_parse_git_host_ssh() {
    let result = parse_git_host("git@github.com:owner/repo.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "github.com");
}

#[test]
fn test_parse_git_host_gitlab() {
    let result = parse_git_host("https://gitlab.com/group/project.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "gitlab.com");
}

#[test]
fn test_parse_git_host_invalid() {
    let result = parse_git_host("not-a-valid-url");
    assert!(result.is_err());
}

#[test]
fn test_parse_git_host_with_port() {
    let result = parse_git_host("https://git.example.com:8443/repo.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "git.example.com");
}

#[test]
fn test_parse_git_host_bitbucket() {
    let result = parse_git_host("https://bitbucket.org/team/repo.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "bitbucket.org");
}

#[test]
fn test_parse_git_host_ssh_no_git_prefix() {
    let result = parse_git_host("user@example.com:path/to/repo.git");
    let _ = result;
}

#[test]
fn test_parse_git_host_http_no_s() {
    let result = parse_git_host("http://github.com/owner/repo.git");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "github.com");
}

#[tokio::test]
async fn test_git_tag_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    // 1. Create lightweight tag
    let uuid_lw = git_tag(
        dest.clone(),
        "v1.0.0".to_string(),
        None,
        None,
        None,
        app.state(),
        app.handle().clone(),
    )
    .await
    .unwrap();

    // Poll for completion
    let registry = app.state::<TaskRegistryState>();
    let id_lw = uuid_lw.parse::<Uuid>().unwrap();
    loop {
        let snapshot = registry.snapshot(&id_lw).unwrap();
        if snapshot.state == TaskState::Completed {
            break;
        }
        if snapshot.state == TaskState::Failed {
            panic!(
                "Tag LW task failed: {}",
                registry.fail_reason(&id_lw).unwrap_or_default()
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // 2. Create annotated tag
    let uuid_ann = git_tag(
        dest.clone(),
        "v1.1.0".to_string(),
        Some("Release 1.1.0".to_string()),
        Some(true),
        None,
        app.state(),
        app.handle().clone(),
    )
    .await
    .unwrap();

    let id_ann = uuid_ann.parse::<Uuid>().unwrap();
    loop {
        let snapshot = registry.snapshot(&id_ann).unwrap();
        if snapshot.state == TaskState::Completed {
            break;
        }
        if snapshot.state == TaskState::Failed {
            panic!(
                "Tag ANN task failed: {}",
                registry.fail_reason(&id_ann).unwrap_or_default()
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // 3. Verify disk state
    let repo = git2::Repository::open(temp.path()).unwrap();

    // Check lightweight tag
    let (obj_lw, _ref_lw) = repo.revparse_ext("v1.0.0").unwrap();
    assert!(obj_lw.as_tag().is_none()); // revparse_ext on lightweight tag returns the commit

    // Check annotated tag
    let (obj_ann, _ref_ann) = repo.revparse_ext("v1.1.0").unwrap();
    let tag = obj_ann.as_tag().expect("Should be an annotated tag object");
    assert_eq!(tag.message(), Some("Release 1.1.0\n"));
}

#[tokio::test]
async fn test_git_remote_set_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    // Add remote first
    let uuid_add = git_remote_add(
        dest.clone(),
        "upstream".to_string(),
        "https://github.com/old/repo.git".to_string(),
        app.state(),
        app.handle().clone(),
    )
    .await
    .unwrap();

    let registry = app.state::<TaskRegistryState>();
    let id_add = uuid_add.parse::<Uuid>().unwrap();
    loop {
        let snapshot = registry.snapshot(&id_add).unwrap();
        if snapshot.state == TaskState::Completed {
            break;
        }
        if snapshot.state == TaskState::Failed {
            panic!(
                "Remote Add task failed: {}",
                registry.fail_reason(&id_add).unwrap_or_default()
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Change remote URL
    let uuid_set = git_remote_set(
        dest.clone(),
        "upstream".to_string(),
        "https://github.com/new/repo.git".to_string(),
        app.state(),
        app.handle().clone(),
    )
    .await
    .unwrap();

    let id_set = uuid_set.parse::<Uuid>().unwrap();
    loop {
        let snapshot = registry.snapshot(&id_set).unwrap();
        if snapshot.state == TaskState::Completed {
            break;
        }
        if snapshot.state == TaskState::Failed {
            panic!(
                "Remote Set task failed: {}",
                registry.fail_reason(&id_set).unwrap_or_default()
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Verify disk state
    let repo = git2::Repository::open(temp.path()).unwrap();
    let remote = repo.find_remote("upstream").unwrap();
    assert_eq!(remote.url(), Some("https://github.com/new/repo.git"));
}

#[tokio::test]
async fn test_git_delete_branch_command() {
    let (_app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    // Create a branch to delete via git2 directly for synchronous setup in test
    {
        let repo = git2::Repository::open(temp.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("to-delete", &head, false).unwrap();
    }

    // Delete it
    let result = git_delete_branch(
        dest,
        "to-delete".to_string(),
        Some(true), // force
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_worktree_details() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    init_git_repo(&repo_path);
    let dest = repo_path.to_string_lossy().to_string();

    // 1. Add worktree
    let wt_path = temp.path().join("wt_details").to_string_lossy().to_string();
    let add_result = git_worktree_add(
        dest.clone(),
        wt_path.clone(),
        "wt-branch".to_string(),
        Some(true),
        None,
        app.state(),
    )
    .await;
    assert!(add_result.is_ok());

    // 2. Verify details
    let result = git_worktree_list(dest.clone()).await;
    assert!(result.is_ok());
    let wts = result.unwrap();
    assert_eq!(wts.len(), 2);

    let main_wt = wts
        .iter()
        .find(|w| w.is_main)
        .expect("Should have a main worktree");
    let linked_wt = wts
        .iter()
        .find(|w| !w.is_main)
        .expect("Should have a linked worktree");

    assert!(main_wt.path.to_lowercase().contains("repo"));
    assert!(linked_wt.path.to_lowercase().contains("wt_details"));
    assert_eq!(linked_wt.branch, Some("wt-branch".to_string()));
    assert!(!linked_wt.is_bare);
}

#[tokio::test]
async fn test_git_remote_error_handling() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    init_git_repo(temp.path());
    let dest = temp.path().to_string_lossy().to_string();

    // 1. Remove non-existent remote
    let result = git_remote_remove(
        dest,
        "non-existent".to_string(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    let registry = app.state::<TaskRegistryState>();
    // Wait for task failure
    let mut failed = false;
    for _ in 0..50 {
        if let Some(snapshot) = registry.snapshot(&uuid) {
            if snapshot.state == TaskState::Failed {
                failed = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(
        failed,
        "Git remote remove should have failed for non-existent remote"
    );
}
