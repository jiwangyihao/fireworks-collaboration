//! Git command integration tests.
//!
//! Tests the actual command functions using MockRuntime.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Assets, Manager, State};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::git::*;
use fireworks_collaboration_lib::app::types::{
    SharedConfig, SharedCredentialFactory, TaskRegistryState,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::tasks::TaskRegistry;

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

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .manage::<SharedConfig>(config)
        .manage::<SharedCredentialFactory>(credential_factory)
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
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_add(
        dest,
        vec![".".to_string()],
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_git_commit_command() {
    let (app, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().to_string_lossy().to_string();

    let result = git_commit(
        dest,
        "msg".to_string(),
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
        app.state(), // credential_factory
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

    // Test pure function
    let result = git_repo_status(dest).await;

    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(status.current_branch.is_some());
    assert!(status.is_clean);
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
