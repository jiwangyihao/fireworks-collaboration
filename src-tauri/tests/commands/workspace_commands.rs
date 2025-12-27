//! Workspace 命令函数测试
//!
//! 测试 `app::commands::workspace` 模块中的辅助函数

use fireworks_collaboration_lib::app::commands::workspace::apply_repository_reorder;
use fireworks_collaboration_lib::core::workspace::{RepositoryEntry, Workspace};
use std::path::PathBuf;

/// Helper to build a test workspace with 3 repositories
fn build_workspace() -> Workspace {
    let mut ws = Workspace::new("demo".to_string(), PathBuf::from("/demo"));
    ws.add_repository(RepositoryEntry::new(
        "repo1".to_string(),
        "Repo One".to_string(),
        PathBuf::from("repo-1"),
        "https://example.com/r1.git".to_string(),
    ))
    .unwrap();
    ws.add_repository(RepositoryEntry::new(
        "repo2".to_string(),
        "Repo Two".to_string(),
        PathBuf::from("repo-2"),
        "https://example.com/r2.git".to_string(),
    ))
    .unwrap();
    ws.add_repository(RepositoryEntry::new(
        "repo3".to_string(),
        "Repo Three".to_string(),
        PathBuf::from("repo-3"),
        "https://example.com/r3.git".to_string(),
    ))
    .unwrap();
    ws
}

#[test]
fn test_apply_repository_reorder_moves_matching_ids() {
    let mut ws = build_workspace();
    let ids = vec!["repo3".to_string(), "repo1".to_string()];
    apply_repository_reorder(&mut ws, &ids).unwrap();

    let ordered: Vec<_> = ws.repositories.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ordered, vec!["repo3", "repo1", "repo2"]);
}

#[test]
fn test_apply_repository_reorder_errors_on_unknown_id() {
    let mut ws = build_workspace();
    let ids = vec!["repoX".to_string()];
    let result = apply_repository_reorder(&mut ws, &ids);
    assert!(result.is_err());
    let ordered: Vec<_> = ws.repositories.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ordered, vec!["repo1", "repo2", "repo3"]);
}

#[test]
fn test_apply_repository_reorder_errors_on_duplicates() {
    let mut ws = build_workspace();
    let ids = vec!["repo1".to_string(), "repo1".to_string()];
    let result = apply_repository_reorder(&mut ws, &ids);
    assert!(result.is_err());
}

#[test]
fn test_apply_repository_reorder_errors_on_empty_input() {
    let mut ws = build_workspace();
    let before = ws.updated_at.clone();
    let ids: Vec<String> = vec![];
    let result = apply_repository_reorder(&mut ws, &ids);
    assert!(result.is_err());
    assert_eq!(ws.updated_at, before);
}

#[test]
fn test_apply_repository_reorder_updates_timestamp_on_success() {
    let mut ws = build_workspace();
    let before = ws.updated_at.clone();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let ids = vec!["repo2".to_string(), "repo1".to_string()];
    apply_repository_reorder(&mut ws, &ids).unwrap();
    assert_ne!(ws.updated_at, before);
}

// ============================================================================
// WorkspaceInfo 和 RepositoryInfo 类型转换测试
// ============================================================================

use fireworks_collaboration_lib::app::commands::workspace::{RepositoryInfo, WorkspaceInfo};

#[test]
fn test_workspace_info_from_basic() {
    let ws = build_workspace();
    let info = WorkspaceInfo::from(&ws);

    assert_eq!(info.name, "demo");
    assert!(info.root_path.contains("demo"));
    assert_eq!(info.repositories.len(), 3);
}

#[test]
fn test_workspace_info_from_preserves_metadata() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    ws.metadata.insert("key".to_string(), "value".to_string());

    let info = WorkspaceInfo::from(&ws);

    assert_eq!(info.metadata.get("key").unwrap(), "value");
}

#[test]
fn test_repository_info_from_basic() {
    let repo = RepositoryEntry::new(
        "test-id".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("test-path"),
        "https://github.com/test/repo.git".to_string(),
    );

    let info = RepositoryInfo::from(&repo);

    assert_eq!(info.id, "test-id");
    assert_eq!(info.name, "Test Repo");
    assert!(info.path.contains("test-path"));
    assert_eq!(info.remote_url, "https://github.com/test/repo.git");
    assert!(info.enabled); // Default is true
}

#[test]
fn test_repository_info_from_with_tags() {
    let mut repo = RepositoryEntry::new(
        "id".to_string(),
        "name".to_string(),
        PathBuf::from("path"),
        "url".to_string(),
    );
    repo.tags = vec!["tag1".to_string(), "tag2".to_string()];

    let info = RepositoryInfo::from(&repo);

    assert_eq!(info.tags.len(), 2);
    assert!(info.tags.contains(&"tag1".to_string()));
    assert!(info.tags.contains(&"tag2".to_string()));
}

#[test]
fn test_workspace_info_repositories_count_matches() {
    let ws = build_workspace();
    let info = WorkspaceInfo::from(&ws);

    assert_eq!(info.repositories.len(), ws.repositories.len());

    // Verify first repository mapping
    assert_eq!(info.repositories[0].id, ws.repositories[0].id);
    assert_eq!(info.repositories[0].name, ws.repositories[0].name);
}
