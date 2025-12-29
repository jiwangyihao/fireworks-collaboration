//! Submodule command integration tests

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::submodule::*;
use fireworks_collaboration_lib::core::submodule::{SubmoduleConfig, SubmoduleManager};

// MockAssets definition
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

fn create_mock_app() -> tauri::App<tauri::test::MockRuntime> {
    let runner = Box::new(fireworks_collaboration_lib::core::git::Git2Runner::new());
    let manager: SharedSubmoduleManager = Arc::new(Mutex::new(SubmoduleManager::new(
        SubmoduleConfig::default(),
        runner,
    )));

    let context = tauri::test::mock_context(MockAssets);

    tauri::test::mock_builder()
        .manage::<SharedSubmoduleManager>(manager)
        .build(context)
        .expect("Failed to build mock app")
}

#[tokio::test]
async fn test_list_submodules_nonexistent_path() {
    let app = create_mock_app();

    let result = list_submodules("/nonexistent/path/to/repo".to_string(), app.state()).await;
    // Should fail for non-existent path
    assert!(result.is_err());
}

#[tokio::test]
async fn test_has_submodules_nonexistent_path() {
    let app = create_mock_app();

    let result = has_submodules("/nonexistent/path/to/repo".to_string(), app.state()).await;
    // Should fail for non-existent path
    assert!(result.is_err());
}

#[tokio::test]
async fn test_init_all_submodules_nonexistent_path() {
    let app = create_mock_app();

    let result = init_all_submodules("/nonexistent/path".to_string(), app.state()).await;
    // Commands that return SubmoduleCommandResult wrap errors in success=false
    assert!(result.is_ok());
    let cmd_result = result.unwrap();
    assert!(!cmd_result.success);
}

#[tokio::test]
async fn test_init_submodule_nonexistent_path() {
    let app = create_mock_app();

    let result = init_submodule(
        "/nonexistent/path".to_string(),
        "some-submodule".to_string(),
        app.state(),
    )
    .await;

    assert!(result.is_ok());
    let cmd_result = result.unwrap();
    assert!(!cmd_result.success);
}

#[tokio::test]
async fn test_update_all_submodules_nonexistent_path() {
    let app = create_mock_app();

    let result = update_all_submodules("/nonexistent/path".to_string(), app.state()).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().success);
}

#[tokio::test]
async fn test_update_submodule_nonexistent_path() {
    let app = create_mock_app();

    let result = update_submodule(
        "/nonexistent/path".to_string(),
        "some-submodule".to_string(),
        app.state(),
    )
    .await;

    assert!(result.is_ok());
    assert!(!result.unwrap().success);
}

#[tokio::test]
async fn test_sync_all_submodules_nonexistent_path() {
    let app = create_mock_app();

    let result = sync_all_submodules("/nonexistent/path".to_string(), app.state()).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().success);
}

#[tokio::test]
async fn test_sync_submodule_nonexistent_path() {
    let app = create_mock_app();

    let result = sync_submodule(
        "/nonexistent/path".to_string(),
        "some-submodule".to_string(),
        app.state(),
    )
    .await;

    assert!(result.is_ok());
    assert!(!result.unwrap().success);
}

#[tokio::test]
async fn test_get_submodule_config() {
    let app = create_mock_app();

    let result = get_submodule_config(app.state()).await;
    assert!(result.is_ok());

    let config = result.unwrap();
    // Default config should have some sensible defaults
    assert!(config.max_depth < 1000);
}

// Test with a temporary valid git repo
mod with_temp_repo {
    use super::*;
    use tempfile::tempdir;

    fn init_temp_git_repo() -> tempfile::TempDir {
        let temp = tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .expect("git init failed");
        temp
    }

    #[tokio::test]
    async fn test_list_submodules_empty_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();

        let result = list_submodules(temp.path().to_string_lossy().to_string(), app.state()).await;

        assert!(result.is_ok());
        let submodules = result.unwrap();
        assert!(submodules.is_empty());
    }

    #[tokio::test]
    async fn test_has_submodules_empty_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();

        let result = has_submodules(temp.path().to_string_lossy().to_string(), app.state()).await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_init_all_submodules_empty_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();

        let result =
            init_all_submodules(temp.path().to_string_lossy().to_string(), app.state()).await;

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        // For empty repo, init should succeed (no submodules to init)
        assert!(cmd_result.success);
    }

    #[tokio::test]
    async fn test_update_all_submodules_empty_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();

        let result =
            update_all_submodules(temp.path().to_string_lossy().to_string(), app.state()).await;

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        // For empty repo, update should succeed (no submodules to update)
        assert!(cmd_result.success);
    }

    #[tokio::test]
    async fn test_sync_all_submodules_empty_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();

        let result =
            sync_all_submodules(temp.path().to_string_lossy().to_string(), app.state()).await;

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        assert!(cmd_result.success);
    }

    #[tokio::test]
    async fn test_init_submodule_missing_in_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();
        // Try to init a submodule that doesn't exist
        let result = init_submodule(
            temp.path().to_string_lossy().to_string(),
            "nofound".to_string(),
            app.state(),
        )
        .await;

        assert!(result.is_ok());
        // Should fail gracefully
        assert!(!result.unwrap().success);
    }

    #[tokio::test]
    async fn test_update_submodule_missing_in_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();
        let result = update_submodule(
            temp.path().to_string_lossy().to_string(),
            "nofound".to_string(),
            app.state(),
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }

    #[tokio::test]
    async fn test_sync_submodule_missing_in_repo() {
        let temp = init_temp_git_repo();
        let app = create_mock_app();
        let result = sync_submodule(
            temp.path().to_string_lossy().to_string(),
            "nofound".to_string(),
            app.state(),
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }
}
