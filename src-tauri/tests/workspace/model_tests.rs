use fireworks_collaboration_lib::core::workspace::{Workspace, RepositoryEntry, WorkspaceConfig};
use std::path::PathBuf;

#[test]
fn test_workspace_new() {
    let ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/test"));
    assert_eq!(ws.name, "test-workspace");
    assert_eq!(ws.root_path, PathBuf::from("/test"));
    assert!(ws.repositories.is_empty());
    assert!(!ws.created_at.is_empty());
    assert_eq!(ws.created_at, ws.updated_at);
}

#[test]
fn test_add_repository() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    );

    assert!(ws.add_repository(repo.clone()).is_ok());
    assert_eq!(ws.repositories.len(), 1);
    assert_eq!(ws.get_repository("repo1").unwrap().name, "Repo 1");

    // 重复添加应该失败
    assert!(ws.add_repository(repo).is_err());
}

#[test]
fn test_remove_repository() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    );

    ws.add_repository(repo).unwrap();
    assert_eq!(ws.repositories.len(), 1);

    let removed = ws.remove_repository("repo1").unwrap();
    assert_eq!(removed.id, "repo1");
    assert_eq!(ws.repositories.len(), 0);

    // 移除不存在的仓库应该失败
    assert!(ws.remove_repository("repo1").is_err());
}

#[test]
fn test_get_enabled_repositories() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    
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

    ws.add_repository(repo1).unwrap();
    ws.add_repository(repo2).unwrap();

    let enabled = ws.get_enabled_repositories();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "repo1");
}

#[test]
fn test_repository_entry_tags() {
    let mut repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    );

    repo.add_tag("frontend".to_string());
    repo.add_tag("critical".to_string());
    assert_eq!(repo.tags.len(), 2);
    assert!(repo.has_tag("frontend"));
    assert!(repo.has_tag("critical"));
    assert!(!repo.has_tag("backend"));

    // 重复添加不应增加标签
    repo.add_tag("frontend".to_string());
    assert_eq!(repo.tags.len(), 2);

    repo.remove_tag("frontend");
    assert_eq!(repo.tags.len(), 1);
    assert!(!repo.has_tag("frontend"));
}

#[test]
fn test_workspace_config_default() {
    let config = WorkspaceConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.max_concurrent_repos, 3);
    assert!(config.default_template.is_none());
    assert!(config.workspace_file.is_none());
}

#[test]
fn test_serialization() {
    let ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    let json = serde_json::to_string(&ws).unwrap();
    let deserialized: Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(ws, deserialized);
}
