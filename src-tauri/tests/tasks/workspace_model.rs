//! Workspace model and storage unit tests
//!
//! Tests for Workspace, RepositoryEntry, SubmoduleManager, and WorkspaceStorage.

use std::path::PathBuf;
use tempfile::tempdir;

use fireworks_collaboration_lib::core::workspace::model::{
    RepositoryEntry, Workspace, WorkspaceConfig,
};
use fireworks_collaboration_lib::core::workspace::storage::WorkspaceStorage;

// ============ Workspace Model Tests ============

#[test]
fn test_workspace_new() {
    let ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    assert_eq!(ws.name, "test-ws");
    assert_eq!(ws.root_path, PathBuf::from("/tmp/test"));
    assert!(ws.repositories.is_empty());
    assert!(ws.description.is_none());
}

#[test]
fn test_workspace_add_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repos/myrepo"),
        "https://github.com/user/repo.git".to_string(),
    );

    let result = ws.add_repository(repo);
    assert!(result.is_ok());
    assert_eq!(ws.repositories.len(), 1);
    assert_eq!(ws.repositories[0].id, "repo1");
}

#[test]
fn test_workspace_add_duplicate_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repos/myrepo"),
        "https://github.com/user/repo.git".to_string(),
    );
    let repo2 = RepositoryEntry::new(
        "repo1".to_string(), // Same ID
        "Another Repo".to_string(),
        PathBuf::from("repos/another"),
        "https://github.com/user/another.git".to_string(),
    );

    ws.add_repository(repo1).unwrap();
    let result = ws.add_repository(repo2);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("已存在"));
}

#[test]
fn test_workspace_remove_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repos/myrepo"),
        "https://github.com/user/repo.git".to_string(),
    );
    ws.add_repository(repo).unwrap();

    let removed = ws.remove_repository("repo1");
    assert!(removed.is_ok());
    assert_eq!(removed.unwrap().id, "repo1");
    assert!(ws.repositories.is_empty());
}

#[test]
fn test_workspace_remove_nonexistent_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let result = ws.remove_repository("nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("不存在"));
}

#[test]
fn test_workspace_get_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repos/myrepo"),
        "https://github.com/user/repo.git".to_string(),
    );
    ws.add_repository(repo).unwrap();

    let found = ws.get_repository("repo1");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "My Repo");

    let not_found = ws.get_repository("nonexistent");
    assert!(not_found.is_none());
}

#[test]
fn test_workspace_get_enabled_repositories() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let mut repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Enabled Repo".to_string(),
        PathBuf::from("repos/enabled"),
        "https://github.com/user/enabled.git".to_string(),
    );
    repo1.enabled = true;

    let mut repo2 = RepositoryEntry::new(
        "repo2".to_string(),
        "Disabled Repo".to_string(),
        PathBuf::from("repos/disabled"),
        "https://github.com/user/disabled.git".to_string(),
    );
    repo2.enabled = false;

    ws.add_repository(repo1).unwrap();
    ws.add_repository(repo2).unwrap();

    let enabled = ws.get_enabled_repositories();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "repo1");
}

#[test]
fn test_workspace_update_repository() {
    let mut ws = Workspace::new("test-ws".to_string(), PathBuf::from("/tmp/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repos/myrepo"),
        "https://github.com/user/repo.git".to_string(),
    );
    ws.add_repository(repo).unwrap();

    let result = ws.update_repository("repo1", |r| {
        r.name = "Updated Name".to_string();
    });
    assert!(result.is_ok());
    assert_eq!(ws.get_repository("repo1").unwrap().name, "Updated Name");
}

// ============ RepositoryEntry Tests ============

#[test]
fn test_repository_entry_new() {
    let entry = RepositoryEntry::new(
        "test-id".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("path/to/repo"),
        "https://github.com/test/repo.git".to_string(),
    );

    assert_eq!(entry.id, "test-id");
    assert_eq!(entry.name, "Test Repo");
    assert_eq!(entry.default_branch, "main");
    assert!(entry.enabled);
    assert!(!entry.has_submodules);
    assert!(entry.tags.is_empty());
}

#[test]
fn test_repository_entry_add_tag() {
    let mut entry = RepositoryEntry::new(
        "test-id".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("path/to/repo"),
        "https://github.com/test/repo.git".to_string(),
    );

    entry.add_tag("frontend".to_string());
    assert_eq!(entry.tags.len(), 1);
    assert!(entry.has_tag("frontend"));

    // Adding duplicate should not add again
    entry.add_tag("frontend".to_string());
    assert_eq!(entry.tags.len(), 1);
}

#[test]
fn test_repository_entry_remove_tag() {
    let mut entry = RepositoryEntry::new(
        "test-id".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("path/to/repo"),
        "https://github.com/test/repo.git".to_string(),
    );

    entry.add_tag("frontend".to_string());
    entry.add_tag("backend".to_string());
    assert_eq!(entry.tags.len(), 2);

    entry.remove_tag("frontend");
    assert_eq!(entry.tags.len(), 1);
    assert!(!entry.has_tag("frontend"));
    assert!(entry.has_tag("backend"));
}

// ============ WorkspaceConfig Tests ============

#[test]
fn test_workspace_config_default() {
    let config = WorkspaceConfig::default();
    assert!(!config.enabled); // 默认不启用
    assert_eq!(config.max_concurrent_repos, 3);
    assert_eq!(config.status_cache_ttl_secs, 15);
    assert_eq!(config.status_max_concurrency, 4);
    assert!(config.default_template.is_none());
}

// ============ WorkspaceStorage Tests ============

#[test]
fn test_storage_new() {
    let storage = WorkspaceStorage::new(PathBuf::from("/tmp/workspace.json"));
    assert_eq!(storage.file_path(), PathBuf::from("/tmp/workspace.json"));
}

#[test]
fn test_storage_from_app_data_dir() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::from_app_data_dir(temp.path());
    assert!(storage.file_path().ends_with("workspace.json"));
}

#[test]
fn test_storage_exists_false() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("nonexistent.json"));
    assert!(!storage.exists());
}

#[test]
fn test_storage_save_and_load() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("workspace.json"));

    let mut ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/tmp/test"));
    ws.add_repository(RepositoryEntry::new(
        "repo1".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("repos/test"),
        "https://github.com/test/repo.git".to_string(),
    ))
    .unwrap();

    // Save
    let save_result = storage.save(&ws);
    assert!(save_result.is_ok());
    assert!(storage.exists());

    // Load
    let loaded = storage.load();
    assert!(loaded.is_ok());
    let loaded_ws = loaded.unwrap();
    assert_eq!(loaded_ws.name, "test-workspace");
    assert_eq!(loaded_ws.repositories.len(), 1);
}

#[test]
fn test_storage_load_nonexistent() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("nonexistent.json"));
    let result = storage.load();
    assert!(result.is_err());
}

#[test]
fn test_storage_delete() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("workspace.json"));
    let ws = Workspace::new("test".to_string(), PathBuf::from("/tmp"));
    storage.save(&ws).unwrap();
    assert!(storage.exists());

    let result = storage.delete();
    assert!(result.is_ok());
    assert!(!storage.exists());
}

#[test]
fn test_storage_delete_nonexistent_ok() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("nonexistent.json"));
    // Deleting non-existent should be OK (no-op)
    let result = storage.delete();
    assert!(result.is_ok());
}

#[test]
fn test_storage_backup_and_restore() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("workspace.json"));

    let ws = Workspace::new("original".to_string(), PathBuf::from("/tmp/original"));
    storage.save(&ws).unwrap();

    // Backup
    let backup_result = storage.backup();
    assert!(backup_result.is_ok());
    let backup_path = backup_result.unwrap();
    assert!(backup_path.exists());

    // Verify backup file contains correct data
    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    assert!(backup_content.contains("original"));
}

#[test]
fn test_storage_validate_success() {
    let temp = tempdir().unwrap();
    let storage = WorkspaceStorage::new(temp.path().join("workspace.json"));

    let ws = Workspace::new("valid-workspace".to_string(), PathBuf::from("/tmp"));
    storage.save(&ws).unwrap();

    let result = storage.validate();
    assert!(result.is_ok());
}

#[test]
fn test_storage_validate_empty_name() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("workspace.json");

    // Write invalid JSON with empty name
    let invalid_json = r#"{
        "name": "",
        "root_path": "/tmp",
        "repositories": [],
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z",
        "metadata": {}
    }"#;
    std::fs::write(&file_path, invalid_json).unwrap();

    let storage = WorkspaceStorage::new(file_path);
    let result = storage.validate();
    assert!(result.is_err());
}

#[test]
fn test_storage_validate_duplicate_ids() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("workspace.json");

    // Write JSON with duplicate repo IDs
    let invalid_json = r#"{
        "name": "test",
        "root_path": "/tmp",
        "repositories": [
            {"id": "dup", "name": "Repo 1", "path": "repo1", "remote_url": "https://a.git"},
            {"id": "dup", "name": "Repo 2", "path": "repo2", "remote_url": "https://b.git"}
        ],
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z",
        "metadata": {}
    }"#;
    std::fs::write(&file_path, invalid_json).unwrap();

    let storage = WorkspaceStorage::new(file_path);
    let result = storage.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("重复"));
}
