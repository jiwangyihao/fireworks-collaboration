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

    fn run_git(args: &[&str], cwd: &std::path::Path) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("failed to execute git command");
        if !output.status.success() {
            panic!(
                "git command failed: {:?}\nstdout: {}\nstderr: {}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[tokio::test]
    async fn test_submodule_real_integration() {
        let temp_dir = tempdir().unwrap();
        let parent_path = temp_dir.path().join("parent");
        let child_path = temp_dir.path().join("child");

        // 1. Setup child repo
        std::fs::create_dir_all(&child_path).unwrap();
        run_git(&["init"], &child_path);
        run_git(&["config", "user.email", "test@example.com"], &child_path);
        run_git(&["config", "user.name", "test"], &child_path);
        std::fs::write(child_path.join("README"), "child").unwrap();
        run_git(&["add", "."], &child_path);
        run_git(&["commit", "-m", "initial child"], &child_path);

        // 2. Setup parent repo
        std::fs::create_dir_all(&parent_path).unwrap();
        run_git(&["init"], &parent_path);
        run_git(&["config", "user.email", "test@example.com"], &parent_path);
        run_git(&["config", "user.name", "test"], &parent_path);
        // Allow file protocol just in case
        run_git(&["config", "protocol.file.allow", "always"], &parent_path);

        std::fs::write(parent_path.join("README"), "parent").unwrap();
        run_git(&["add", "."], &parent_path);
        run_git(&["commit", "-m", "initial parent"], &parent_path);

        // 3. Add child as submodule to parent
        // Use relative path and allow file protocol via -c to be extra safe
        run_git(
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "../child",
                "my-sub",
            ],
            &parent_path,
        );
        run_git(&["commit", "-m", "add submodule"], &parent_path);

        let app = create_mock_app();
        let repo_path_str = parent_path.to_string_lossy().to_string();

        // 4. Test list_submodules
        let submodules = list_submodules(repo_path_str.clone(), app.state())
            .await
            .unwrap();
        assert_eq!(submodules.len(), 1);
        assert_eq!(submodules[0].name, "my-sub");

        // 5. Test has_submodules
        let has = has_submodules(repo_path_str.clone(), app.state())
            .await
            .unwrap();
        assert!(has);

        // 6. Test init/update
        let init_res = init_submodule(repo_path_str.clone(), "my-sub".to_string(), app.state())
            .await
            .unwrap();
        assert!(init_res.success);

        let update_res = update_submodule(repo_path_str.clone(), "my-sub".to_string(), app.state())
            .await
            .unwrap();
        assert!(update_res.success);

        // 7. Test sync
        let sync_res = sync_submodule(repo_path_str.clone(), "my-sub".to_string(), app.state())
            .await
            .unwrap();
        assert!(sync_res.success);

        // 8. Test get_submodule_config
        let config = get_submodule_config(app.state()).await.unwrap();
        assert!(config.max_depth > 0);
    }
}
