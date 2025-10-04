use fireworks_collaboration_lib::core::workspace::{WorkspaceManager, WorkspaceConfig, RepositoryEntry};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_manager() -> (WorkspaceManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");
    
    let config = WorkspaceConfig {
        enabled: true,
        ..Default::default()
    };
    
    let manager = WorkspaceManager::new(config, storage_path);
    (manager, temp_dir)
}

#[test]
fn test_create_and_load_workspace() {
    let (mut manager, _temp_dir) = create_test_manager();

    // 创建工作区
    manager
        .create_workspace("test-ws".to_string(), PathBuf::from("/test"))
        .unwrap();

    assert!(manager.is_workspace_loaded());
    assert_eq!(manager.current_workspace().unwrap().name, "test-ws");

    // 关闭并重新加载
    manager.close_workspace();
    assert!(!manager.is_workspace_loaded());

    manager.load_workspace().unwrap();
    assert!(manager.is_workspace_loaded());
    assert_eq!(manager.current_workspace().unwrap().name, "test-ws");
}

#[test]
fn test_add_and_remove_repository() {
    let (mut manager, _temp_dir) = create_test_manager();

    manager
        .create_workspace("test-ws".to_string(), PathBuf::from("/test"))
        .unwrap();

    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    );

    // 添加仓库
    manager.add_repository(repo.clone()).unwrap();
    assert_eq!(manager.get_repositories().unwrap().len(), 1);

    // 获取仓库
    let retrieved = manager.get_repository("repo1").unwrap();
    assert_eq!(retrieved.name, "Repo 1");

    // 移除仓库
    let removed = manager.remove_repository("repo1").unwrap();
    assert_eq!(removed.id, "repo1");
    assert_eq!(manager.get_repositories().unwrap().len(), 0);
}

#[test]
fn test_disabled_workspace() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");
    
    let config = WorkspaceConfig::default(); // 默认禁用
    let mut manager = WorkspaceManager::new(config, storage_path);

    // 尝试创建工作区应该失败
    let result = manager.create_workspace("test".to_string(), PathBuf::from("/test"));
    assert!(result.is_err());

    // 尝试加载工作区应该失败
    let result = manager.load_workspace();
    assert!(result.is_err());
}

#[test]
fn test_get_enabled_repositories() {
    let (mut manager, _temp_dir) = create_test_manager();

    manager
        .create_workspace("test-ws".to_string(), PathBuf::from("/test"))
        .unwrap();

    let mut repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    );
    repo1.enabled = true;

    let mut repo2 = RepositoryEntry::new(
        "repo2".to_string(),
        "Repo 2".to_string(),
        PathBuf::from("repo2"),
        "https://github.com/test/repo2".to_string(),
    );
    repo2.enabled = false;

    manager.add_repository(repo1).unwrap();
    manager.add_repository(repo2).unwrap();

    let enabled = manager.get_enabled_repositories().unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "repo1");
}
