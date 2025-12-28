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
use fireworks_collaboration_lib::core::tasks::TaskRegistry;
use fireworks_collaboration_lib::core::workspace::model::Workspace;

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
