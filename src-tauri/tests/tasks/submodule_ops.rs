//! Submodule operations unit tests
//!
//! Tests for SubmoduleManager, SubmoduleConfig, and SubmoduleInfo.

use std::process::Command;
use tempfile::tempdir;

use fireworks_collaboration_lib::core::submodule::model::SubmoduleConfig;
use fireworks_collaboration_lib::core::submodule::operations::SubmoduleManager;

// Helper to create a repo with initial commit
fn create_repo_with_commit() -> tempfile::TempDir {
    let temp = tempdir().unwrap();
    Command::new("git")
        .args(&["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init repo");
    Command::new("git")
        .args(&["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    std::fs::write(temp.path().join("README.md"), "# Test").unwrap();
    Command::new("git")
        .args(&["add", "."])
        .current_dir(temp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    temp
}

// ============ SubmoduleConfig Tests ============

#[test]
fn test_submodule_config_default() {
    let config = SubmoduleConfig::default();
    assert!(config.auto_recurse);
    assert_eq!(config.max_depth, 5);
    assert!(config.auto_init_on_clone);
    assert!(config.recursive_update);
    assert!(!config.parallel);
    assert_eq!(config.max_parallel, 3);
}

// ============ SubmoduleManager Tests ============

#[test]
fn test_submodule_manager_new() {
    let config = SubmoduleConfig::default();
    let manager = SubmoduleManager::new(config.clone());
    assert_eq!(manager.config(), &config);
}

#[test]
fn test_submodule_manager_has_submodules_no_submodules() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.has_submodules(temp.path());
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_submodule_manager_list_submodules_empty() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.list_submodules(temp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_submodule_manager_list_not_a_repo() {
    let temp = tempdir().unwrap();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.list_submodules(temp.path());
    assert!(result.is_err());
}

#[test]
fn test_submodule_manager_init_not_a_repo() {
    let temp = tempdir().unwrap();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.init(temp.path(), "nonexistent-submodule");
    assert!(result.is_err());
}

#[test]
fn test_submodule_manager_init_all_empty() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.init_all(temp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_submodule_manager_sync_all_empty() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.sync_all(temp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_submodule_manager_sync_nonexistent() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.sync(temp.path(), "nonexistent-submodule");
    assert!(result.is_err());
}

#[test]
fn test_submodule_manager_update_all_no_submodules() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.update_all(temp.path(), 1);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_submodule_manager_update_nonexistent() {
    let temp = create_repo_with_commit();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());

    let result = manager.update(temp.path(), "nonexistent-submodule");
    assert!(result.is_err());
}
