//! Workspace command integration tests (Direct Command Call)

use std::borrow::Cow;

use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::credential::SharedCredentialFactory;
use fireworks_collaboration_lib::app::commands::workspace::*;

use fireworks_collaboration_lib::app::types::{
    SharedConfig, SharedWorkspaceManager, TaskRegistryState,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::tasks::{TaskRegistry, TaskState};
use fireworks_collaboration_lib::core::workspace::model::Workspace;

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

// Include MockAssets definition (duplicated from direct_command for isolation)
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

fn create_mock_app() -> (
    tauri::App<tauri::test::MockRuntime>,
    SharedWorkspaceManager,
    TaskRegistryState,
    SharedConfig,
    SharedCredentialFactory,
) {
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());
    let workspace_manager: SharedWorkspaceManager = Arc::new(Mutex::new(None));
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));
    let credential_factory: SharedCredentialFactory = Arc::new(Mutex::new(None));

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .manage::<SharedWorkspaceManager>(workspace_manager.clone())
        .manage::<SharedConfig>(config.clone())
        .manage::<SharedCredentialFactory>(credential_factory.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, workspace_manager, registry, config, credential_factory)
}

#[tokio::test]
async fn test_create_workspace_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();

    let req = CreateWorkspaceRequest {
        name: "Test Workspace".to_string(),
        root_path: "/tmp/test_ws".to_string(),
        metadata: None,
    };

    let result = create_workspace(req, state).await;
    assert!(result.is_ok());

    let ws = manager.lock().unwrap();
    assert!(ws.is_some());
    assert_eq!(ws.as_ref().unwrap().name, "Test Workspace");
}

#[tokio::test]
async fn test_close_workspace_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();

    // Pre-load a workspace
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new("Test".to_string(), "/tmp".into()));
    }

    let result = close_workspace(state).await;
    assert!(result.is_ok());

    let ws = manager.lock().unwrap();
    assert!(ws.is_none());
}

#[tokio::test]
async fn test_add_repository_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();

    // Pre-load
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new("Test Add".to_string(), "/tmp".into()));
    }

    let req = AddRepositoryRequest {
        id: "repo-1".to_string(),
        name: "Repo 1".to_string(),
        path: "repo1".to_string(),
        remote_url: "https://git.example.com/repo1.git".to_string(),
        tags: Some(vec!["tag1".to_string()]),
        enabled: Some(true),
    };

    let result = add_repository(req, state).await;
    assert!(result.is_ok());

    let ws = manager.lock().unwrap();
    let repo = ws.as_ref().unwrap().get_repository("repo-1");
    assert!(repo.is_some());
    assert_eq!(
        repo.unwrap().remote_url,
        "https://git.example.com/repo1.git"
    );
}

#[tokio::test]
async fn test_list_repositories_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();

    // Pre-load
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test List".to_string(), "/tmp".into());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "r1".to_string(),
                "R1".to_string(),
                "r1".into(),
                "url".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let result = list_repositories(state).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_remove_repository_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();

    // Pre-load
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test Remove".to_string(), "/tmp".into());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "r1".to_string(),
                "R1".to_string(),
                "r1".into(),
                "url".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let result = remove_repository("r1".to_string(), state).await;
    assert!(result.is_ok());

    let ws = manager.lock().unwrap();
    assert!(ws.as_ref().unwrap().get_repository("r1").is_none());
}

#[tokio::test]
async fn test_workspace_batch_clone_command() {
    let (app, manager, registry, _config, _) = create_mock_app();

    // Pre-load workspace with repo
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Batch".to_string(), "/tmp".into());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "r1".to_string(),
                "R1".to_string(),
                "r1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let req = WorkspaceBatchCloneRequest {
        repo_ids: Some(vec!["r1".to_string()]),
        ..Default::default()
    };

    let result = workspace_batch_clone(
        req,
        app.state(),
        app.state(), // registry
        app.state(), // config
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}
#[tokio::test]
async fn test_workspace_batch_fetch_command() {
    let (app, manager, registry, _config, _) = create_mock_app();

    // Pre-load workspace with repo
    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path().to_path_buf();
    let repo_path = temp_dir.path().join("r1");
    std::fs::create_dir_all(&repo_path).unwrap();

    // Init git repo to satisfy pre-checks
    let _ = std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .output();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Batch".to_string(), root_path);
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "r1".to_string(),
                "R1".to_string(),
                "r1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let req = WorkspaceBatchFetchRequest {
        repo_ids: Some(vec!["r1".to_string()]),
        ..Default::default()
    };

    let result = workspace_batch_fetch(
        req,
        app.state(),
        app.state(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    if let Err(ref e) = result {
        println!("Fetch Error: {}", e);
    }
    assert!(
        result.is_ok(),
        "Fetch failed with error: {:?}",
        result.err()
    );
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

#[tokio::test]
async fn test_workspace_batch_push_command() {
    let (app, manager, registry, _config, _) = create_mock_app();

    // Pre-load workspace with repo
    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path().to_path_buf();
    let repo_path = temp_dir.path().join("r1");
    std::fs::create_dir_all(&repo_path).unwrap();

    // Init git repo to satisfy pre-checks
    let _ = std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .output();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Batch".to_string(), root_path);
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "r1".to_string(),
                "R1".to_string(),
                "r1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let req = WorkspaceBatchPushRequest {
        repo_ids: Some(vec!["r1".to_string()]),
        ..Default::default()
    };

    let result = workspace_batch_push(
        req,
        app.state(),
        app.state(), // registry
        app.state(), // credential_factory
        app.handle().clone(),
    )
    .await;

    if let Err(ref e) = result {
        println!("Push Error: {}", e);
    }
    assert!(result.is_ok(), "Push failed with error: {:?}", result.err());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

// ============================================================================
// Additional workspace command tests for comprehensive coverage
// ============================================================================

#[tokio::test]
async fn test_save_workspace_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    // Create workspace first
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new(
            "Test".to_string(),
            temp.path().to_path_buf(),
        ));
    }

    let save_path = temp.path().join("workspace.json");
    let result = save_workspace(save_path.to_string_lossy().to_string(), app.state()).await;
    assert!(result.is_ok());
    assert!(save_path.exists());
}

#[tokio::test]
async fn test_load_workspace_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    // Create and save workspace first
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new(
            "LoadTest".to_string(),
            temp.path().to_path_buf(),
        ));
    }
    let save_path = temp.path().join("workspace.json");
    let _ = save_workspace(save_path.to_string_lossy().to_string(), app.state()).await;

    // Close and reload
    let _ = close_workspace(app.state()).await;

    let result = load_workspace(save_path.to_string_lossy().to_string(), app.state()).await;
    assert!(result.is_ok());
    let info = result.unwrap();
    assert_eq!(info.name, "LoadTest");
}

#[tokio::test]
async fn test_get_repository_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    // Setup workspace with repo
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test".to_string(), temp.path().to_path_buf());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "repo1".to_string(),
                "Repo1".to_string(),
                "repo1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let result = get_repository("repo1".to_string(), app.state()).await;
    assert!(result.is_ok());
    let repo = result.unwrap();
    assert_eq!(repo.id, "repo1");
}

#[tokio::test]
async fn test_get_repository_not_found() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new(
            "Test".to_string(),
            temp.path().to_path_buf(),
        ));
    }

    let result = get_repository("nonexistent".to_string(), app.state()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_enabled_repositories_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test".to_string(), temp.path().to_path_buf());
        let mut repo = fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
            "enabled_repo".to_string(),
            "Enabled".to_string(),
            "enabled".into(),
            "https://url.git".to_string(),
        );
        repo.enabled = true;
        ws.add_repository(repo).unwrap();
        *guard = Some(ws);
    }

    let result = list_enabled_repositories(app.state()).await;
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert!(!repos.is_empty());
}

#[tokio::test]
async fn test_get_workspace_config_command() {
    let result = get_workspace_config().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_workspace_file_nonexistent() {
    let result = validate_workspace_file("/nonexistent/path.json".to_string()).await;
    // Should fail for non-existent file
    assert!(result.is_err() || result.unwrap() == false);
}

#[tokio::test]
async fn test_backup_workspace_nonexistent() {
    let result = backup_workspace("/nonexistent/workspace.json".to_string()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_restore_workspace_nonexistent() {
    let result = restore_workspace(
        "/nonexistent/backup.json".to_string(),
        "/some/path.json".to_string(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_workspace_no_workspace() {
    let (app, _, _, _, _) = create_mock_app();

    let result = get_workspace(app.state()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_repository_tags_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test".to_string(), temp.path().to_path_buf());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "repo1".to_string(),
                "Repo1".to_string(),
                "repo1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let result = update_repository_tags(
        "repo1".to_string(),
        vec!["tag1".to_string(), "tag2".to_string()],
        app.state(),
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_toggle_repository_enabled_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test".to_string(), temp.path().to_path_buf());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "repo1".to_string(),
                "Repo1".to_string(),
                "repo1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let result = toggle_repository_enabled("repo1".to_string(), app.state()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reorder_repositories_command() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("Test".to_string(), temp.path().to_path_buf());
        for i in 1..=3 {
            ws.add_repository(
                fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                    format!("repo{}", i),
                    format!("Repo{}", i),
                    format!("repo{}", i).into(),
                    format!("https://url{}.git", i),
                ),
            )
            .unwrap();
        }
        *guard = Some(ws);
    }

    // Reorder repos
    let result = reorder_repositories(
        vec![
            "repo3".to_string(),
            "repo1".to_string(),
            "repo2".to_string(),
        ],
        app.state(),
    )
    .await;
    assert!(result.is_ok());
    let repos = result.unwrap();
    assert_eq!(repos[0].id, "repo3");
    assert_eq!(repos[1].id, "repo1");
}

#[tokio::test]
async fn test_load_workspace_invalid_json() {
    let (app, _, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("invalid.json");
    std::fs::write(&file_path, "{ invalid_json: ").unwrap();

    let result = load_workspace(file_path.to_string_lossy().to_string(), app.state()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_workspace_duplicate_path() {
    let (app, _manager, _, _, _) = create_mock_app();
    let state = app.state::<SharedWorkspaceManager>();
    let temp = tempfile::tempdir().unwrap();

    // Create first time
    let req1 = CreateWorkspaceRequest {
        name: "WS1".to_string(),
        root_path: temp.path().to_string_lossy().to_string(),
        metadata: None,
    };
    let _ = create_workspace(req1, state.clone()).await;

    // Create again
    let req2 = CreateWorkspaceRequest {
        name: "WS1".to_string(),
        root_path: temp.path().to_string_lossy().to_string(),
        metadata: None,
    };
    let result = create_workspace(req2, state).await;
    assert!(result.is_ok());
}

// ============================================================================
// Workspace Status Service Tests
// ============================================================================

use fireworks_collaboration_lib::app::types::SharedWorkspaceStatusService;
use fireworks_collaboration_lib::core::workspace::model::WorkspaceConfig;
use fireworks_collaboration_lib::core::workspace::status::WorkspaceStatusService;

fn create_mock_app_with_status() -> (
    tauri::App<tauri::test::MockRuntime>,
    SharedWorkspaceManager,
    SharedWorkspaceStatusService,
) {
    let workspace_manager: SharedWorkspaceManager = Arc::new(Mutex::new(None));
    let ws_config = WorkspaceConfig::default();
    let status_service: SharedWorkspaceStatusService =
        Arc::new(WorkspaceStatusService::new(&ws_config));

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<SharedWorkspaceManager>(workspace_manager.clone())
        .manage::<SharedWorkspaceStatusService>(status_service.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, workspace_manager, status_service)
}

#[tokio::test]
async fn test_clear_workspace_status_cache_command() {
    let (app, _, _) = create_mock_app_with_status();

    let result = clear_workspace_status_cache(app.state()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_invalidate_workspace_status_entry_command() {
    let (app, _, _) = create_mock_app_with_status();

    // Invalidating a non-existent entry should return false
    let result = invalidate_workspace_status_entry("nonexistent".to_string(), app.state()).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_get_workspace_statuses_no_workspace() {
    let (app, _, _) = create_mock_app_with_status();

    // No workspace loaded should fail
    let result = get_workspace_statuses(None, app.state(), app.state()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No workspace"));
}

#[tokio::test]
async fn test_get_workspace_statuses_empty_workspace() {
    let (app, manager, _) = create_mock_app_with_status();
    let temp = tempfile::tempdir().unwrap();

    // Pre-load empty workspace
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new(
            "StatusTest".to_string(),
            temp.path().to_path_buf(),
        ));
    }

    // Get statuses for empty workspace should succeed
    let result = get_workspace_statuses(None, app.state(), app.state()).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.statuses.is_empty());
}

#[tokio::test]
async fn test_workspace_backup_restore_lifecycle() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let root_path = temp.path().to_path_buf();
    let ws_path = root_path.join("test_ws.json");

    // 1. Setup workspace and save it
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("BackupRestore".to_string(), root_path.clone());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "repo1".to_string(),
                "Repo1".to_string(),
                "repo1".into(),
                "https://url.git".to_string(),
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let save_path_str = ws_path.to_string_lossy().to_string();
    save_workspace(save_path_str.clone(), app.state())
        .await
        .expect("Initial save failed");
    assert!(ws_path.exists());

    // 2. Backup
    let backup_path_str = backup_workspace(save_path_str.clone())
        .await
        .expect("Backup failed");
    let backup_path = std::path::PathBuf::from(&backup_path_str);
    assert!(backup_path.exists());

    // 3. Delete original and Restore
    std::fs::remove_file(&ws_path).unwrap();
    assert!(!ws_path.exists());

    restore_workspace(backup_path_str, save_path_str.clone())
        .await
        .expect("Restore failed");
    assert!(ws_path.exists());

    // 4. Verify reload
    let _ = close_workspace(app.state()).await;
    let result = load_workspace(save_path_str, app.state()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "BackupRestore");
}

#[tokio::test]
async fn test_workspace_batch_fetch_multi_repo() {
    let (app, manager, registry, _config, _) = create_mock_app();
    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path().to_path_buf();

    // Setup 2 repos
    for id in ["r1", "r2"] {
        let repo_path = temp_dir.path().join(id);
        std::fs::create_dir_all(&repo_path).unwrap();

        // Minimal git init
        let _ = std::process::Command::new("git")
            .arg("init")
            .current_dir(&repo_path)
            .output();
    }

    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("MultiBatch".to_string(), root_path);
        for id in ["r1", "r2"] {
            ws.add_repository(
                fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                    id.to_string(),
                    id.to_uppercase(),
                    id.into(),
                    format!("https://url_{}.git", id),
                ),
            )
            .unwrap();
        }
        *guard = Some(ws);
    }

    let req = WorkspaceBatchFetchRequest {
        repo_ids: Some(vec!["r1".to_string(), "r2".to_string()]),
        ..Default::default()
    };

    let result = workspace_batch_fetch(
        req,
        app.state(),
        app.state(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    // Verify task exists in registry
    assert!(registry.snapshot(&uuid).is_some());
}

#[tokio::test]
async fn test_workspace_batch_clone_disk_verification() {
    let (app, manager, registry, _config, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();
    let ws_path = temp.path().to_path_buf();

    // 1. Setup a "remote" repo to clone from
    let remote_dir = temp.path().join("remote_repo");
    init_git_repo(&remote_dir);
    let remote_url = remote_dir.to_string_lossy().to_string();

    // 2. Pre-load workspace with repo
    {
        let mut guard = manager.lock().unwrap();
        let mut ws = Workspace::new("DiskVerify".to_string(), ws_path.clone());
        ws.add_repository(
            fireworks_collaboration_lib::core::workspace::model::RepositoryEntry::new(
                "repo1".to_string(),
                "Repo 1".to_string(),
                "repo1".into(),
                remote_url,
            ),
        )
        .unwrap();
        *guard = Some(ws);
    }

    let req = WorkspaceBatchCloneRequest {
        repo_ids: Some(vec!["repo1".to_string()]),
        ..Default::default()
    };

    let result = workspace_batch_clone(
        req,
        app.state(),
        app.state(),
        app.state(),
        app.handle().clone(),
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();

    // Wait for task completion
    let mut completed = false;
    for _ in 0..100 {
        if let Some(snapshot) = registry.snapshot(&uuid) {
            if snapshot.state == TaskState::Completed {
                completed = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(completed, "Workspace batch clone task did not complete");

    // 3. Verify disk state
    let target_path = ws_path.join("repo1");
    assert!(target_path.exists());
    assert!(target_path.join(".git").exists());
}

#[tokio::test]
async fn test_workspace_duplicate_repo_prevention() {
    let (app, manager, _, _, _) = create_mock_app();
    let temp = tempfile::tempdir().unwrap();

    // 1. Initialize workspace
    {
        let mut guard = manager.lock().unwrap();
        *guard = Some(Workspace::new("DupTest".into(), temp.path().into()));
    }

    // 2. Add first repo
    let req1 = AddRepositoryRequest {
        id: "r1".into(),
        name: "Repo 1".into(),
        path: "path1".into(),
        remote_url: "url1".into(),
        tags: None,
        enabled: Some(true),
    };
    add_repository(req1, app.state()).await.unwrap();

    // 3. Add same repo again (SAME PATH)
    let req2 = AddRepositoryRequest {
        id: "r2".into(), // Different ID
        name: "Repo 2".into(),
        path: "path1".into(), // SAME PATH
        remote_url: "url2".into(),
        tags: None,
        enabled: Some(true),
    };
    let result = add_repository(req2, app.state()).await;

    // Implementation should prevent duplicate paths within same workspace
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("已存在"));
}
