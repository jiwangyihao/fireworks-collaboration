//! P7.0 工作区模块集成测试
//!
//! 测试覆盖：
//! - 工作区数据模型的创建、修改、序列化
//! - 工作区配置的加载、验证、热更新
//! - 工作区存储的读写、备份、恢复
//! - WorkspaceManager 的完整工作流

#[path = "../common/mod.rs"]
pub(crate) mod common;

use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

use common::{repo_factory::RepoBuilder, task_wait::wait_until_task_done};
use fireworks_collaboration_lib::core::submodule::{SubmoduleConfig, SubmoduleManager};
use fireworks_collaboration_lib::core::tasks::{
    model::{TaskKind, TaskState, WorkspaceBatchOperation},
    registry::TaskRegistry,
    workspace_batch::{
        CloneOptions, FetchOptions, PushOptions, WorkspaceBatchChildOperation,
        WorkspaceBatchChildSpec,
    },
};
use fireworks_collaboration_lib::core::workspace::{
    RepositoryEntry, StatusQuery, SyncState, WorkingTreeState, Workspace, WorkspaceConfig,
    WorkspaceConfigManager, WorkspaceManager, WorkspaceStatusService, WorkspaceStorage,
};
use git2::{Repository as GitRepository, Signature};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

// ============================================================================
// 工作区数据模型测试
// ============================================================================

#[test]
fn test_workspace_creation_and_serialization() {
    let ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/test"));

    assert_eq!(ws.name, "test-workspace");
    assert_eq!(ws.root_path, PathBuf::from("/test"));
    assert!(ws.repositories.is_empty());
    assert!(ws.created_at.len() > 0);
    assert!(ws.updated_at.len() > 0);

    // 测试序列化
    let json = serde_json::to_string(&ws).unwrap();
    let deserialized: Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(ws, deserialized);
}

#[test]
fn test_repository_management() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    // 添加仓库
    let repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Repository 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );

    assert!(ws.add_repository(repo1.clone()).is_ok());
    assert_eq!(ws.repositories.len(), 1);

    // 重复添加应失败
    assert!(ws.add_repository(repo1).is_err());

    // 获取仓库
    assert!(ws.get_repository("repo1").is_some());
    assert!(ws.get_repository("nonexistent").is_none());

    // 移除仓库
    let removed = ws.remove_repository("repo1").unwrap();
    assert_eq!(removed.id, "repo1");
    assert_eq!(ws.repositories.len(), 0);

    // 移除不存在的仓库应失败
    assert!(ws.remove_repository("repo1").is_err());
}

#[test]
fn test_repository_tags() {
    let mut repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );

    // 添加标签
    repo.add_tag("frontend".to_string());
    repo.add_tag("critical".to_string());
    assert_eq!(repo.tags.len(), 2);
    assert!(repo.has_tag("frontend"));
    assert!(repo.has_tag("critical"));

    // 重复添加不应增加
    repo.add_tag("frontend".to_string());
    assert_eq!(repo.tags.len(), 2);

    // 移除标签
    repo.remove_tag("frontend");
    assert_eq!(repo.tags.len(), 1);
    assert!(!repo.has_tag("frontend"));
}

#[test]
fn test_enabled_repository_filtering() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    let mut repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    repo1.enabled = true;

    let mut repo2 = RepositoryEntry::new(
        "repo2".to_string(),
        "Repo 2".to_string(),
        PathBuf::from("repo2"),
        "https://github.com/test/repo2.git".to_string(),
    );
    repo2.enabled = false;

    let mut repo3 = RepositoryEntry::new(
        "repo3".to_string(),
        "Repo 3".to_string(),
        PathBuf::from("repo3"),
        "https://github.com/test/repo3.git".to_string(),
    );
    repo3.enabled = true;

    ws.add_repository(repo1).unwrap();
    ws.add_repository(repo2).unwrap();
    ws.add_repository(repo3).unwrap();

    let enabled = ws.get_enabled_repositories();
    assert_eq!(enabled.len(), 2);
    assert!(enabled.iter().any(|r| r.id == "repo1"));
    assert!(enabled.iter().any(|r| r.id == "repo3"));
    assert!(!enabled.iter().any(|r| r.id == "repo2"));
}

// ============================================================================
// 工作区配置测试
// ============================================================================

#[test]
fn test_workspace_config_defaults() {
    let config = WorkspaceConfig::default();
    assert_eq!(config.enabled, false);
    assert_eq!(config.max_concurrent_repos, 3);
    assert!(config.default_template.is_none());
    assert!(config.workspace_file.is_none());
    assert_eq!(config.status_cache_ttl_secs, 15);
    assert_eq!(config.status_max_concurrency, 4);
    assert!(config.status_auto_refresh_secs.is_none());
}

#[test]
fn test_workspace_config_manager() {
    let mut mgr = WorkspaceConfigManager::with_defaults();

    assert_eq!(mgr.is_enabled(), false);
    assert_eq!(mgr.max_concurrent_repos(), 3);

    // 启用工作区
    mgr.set_enabled(true);
    assert_eq!(mgr.is_enabled(), true);

    // 设置并发数
    assert!(mgr.set_max_concurrent_repos(10).is_ok());
    assert_eq!(mgr.max_concurrent_repos(), 10);

    // 设置为 0 应失败
    assert!(mgr.set_max_concurrent_repos(0).is_err());

    // 设置模板
    mgr.set_default_template(Some("my-template".to_string()));
    assert_eq!(mgr.default_template(), Some("my-template"));
}

#[test]
fn test_workspace_config_validation() {
    let mut mgr = WorkspaceConfigManager::with_defaults();

    // 无效配置：max_concurrent_repos = 0
    let mut invalid_config = WorkspaceConfig::default();
    invalid_config.max_concurrent_repos = 0;

    // 直接验证配置
    assert!(mgr.update_config(invalid_config).is_err());
}

// ============================================================================
// 工作区存储测试
// ============================================================================

#[test]
fn test_workspace_storage_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/test"));

    // 保存
    storage.save(&ws).unwrap();
    assert!(storage.exists());

    // 加载
    let loaded = storage.load().unwrap();
    assert_eq!(loaded.name, "test-workspace");
    assert_eq!(loaded.root_path, PathBuf::from("/test"));
}

#[test]
fn test_workspace_storage_backup_and_restore() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path.clone());

    let ws = Workspace::new("original".to_string(), PathBuf::from("/test"));
    storage.save(&ws).unwrap();

    // 备份
    let backup_path = storage.backup().unwrap();
    assert!(backup_path.exists());

    // 验证备份内容
    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    let backup_ws: Workspace = serde_json::from_str(&backup_content).unwrap();
    assert_eq!(backup_ws.name, "original");

    // 等待1秒以确保时间戳不同
    std::thread::sleep(std::time::Duration::from_secs(1));

    // 修改
    let mut modified = ws.clone();
    modified.name = "modified".to_string();
    storage.save(&modified).unwrap();

    let loaded = storage.load().unwrap();
    assert_eq!(loaded.name, "modified");

    // 从备份恢复
    storage.restore_from_backup(&backup_path).unwrap();

    // 验证恢复成功
    let restored = storage.load().unwrap();
    assert_eq!(restored.name, "original");
    assert_eq!(restored.root_path, PathBuf::from("/test"));
}

#[test]
fn test_workspace_storage_validation() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    // 添加一些仓库
    let repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    ws.add_repository(repo1).unwrap();

    storage.save(&ws).unwrap();

    // 验证应该通过
    assert!(storage.validate().is_ok());
}

// ============================================================================
// WorkspaceManager 工作流测试
// ============================================================================

#[test]
fn test_workspace_manager_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");

    let mut config = WorkspaceConfig::default();
    config.enabled = true;

    let mut manager = WorkspaceManager::new(config, storage_path);

    // 创建工作区
    manager
        .create_workspace("my-workspace".to_string(), PathBuf::from("/projects"))
        .unwrap();

    assert!(manager.is_workspace_loaded());
    assert_eq!(manager.current_workspace().unwrap().name, "my-workspace");

    // 添加仓库
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "My Repo".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    manager.add_repository(repo).unwrap();

    // 保存
    manager.save_workspace().unwrap();

    // 关闭并重新加载
    manager.close_workspace();
    assert!(!manager.is_workspace_loaded());

    manager.load_workspace().unwrap();
    assert!(manager.is_workspace_loaded());
    assert_eq!(manager.get_repositories().unwrap().len(), 1);
}

#[test]
fn test_workspace_manager_disabled() {
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
fn test_workspace_update_repository() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Original Name".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    ws.add_repository(repo).unwrap();

    // 更新仓库
    ws.update_repository("repo1", |r| {
        r.name = "Updated Name".to_string();
        r.add_tag("updated".to_string());
    })
    .unwrap();

    let updated = ws.get_repository("repo1").unwrap();
    assert_eq!(updated.name, "Updated Name");
    assert!(updated.has_tag("updated"));

    // 更新不存在的仓库应失败
    let result = ws.update_repository("nonexistent", |_| {});
    assert!(result.is_err());
}

#[test]
fn test_workspace_metadata() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    ws.metadata
        .insert("team".to_string(), "platform".to_string());
    ws.metadata
        .insert("project".to_string(), "main".to_string());

    assert_eq!(ws.metadata.get("team"), Some(&"platform".to_string()));
    assert_eq!(ws.metadata.get("project"), Some(&"main".to_string()));

    // 测试序列化包含元数据
    let json = serde_json::to_string(&ws).unwrap();
    let deserialized: Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.metadata, ws.metadata);
}

#[test]
fn test_repository_custom_config() {
    let mut repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );

    repo.custom_config.insert(
        "buildCommand".to_string(),
        serde_json::json!("npm run build"),
    );
    repo.custom_config
        .insert("testCommand".to_string(), serde_json::json!("npm test"));

    assert_eq!(repo.custom_config.len(), 2);

    // 测试序列化
    let json = serde_json::to_string(&repo).unwrap();
    let deserialized: RepositoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.custom_config, repo.custom_config);
}

// ============================================================================
// 边界条件和错误处理测试
// ============================================================================

#[test]
fn test_workspace_empty_name() {
    // 工作区允许空名称(由应用层验证)
    let ws = Workspace::new("".to_string(), PathBuf::from("/test"));
    assert_eq!(ws.name, "");
}

#[test]
fn test_repository_special_characters_in_id() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    // 测试特殊字符ID(允许但不推荐)
    let repo = RepositoryEntry::new(
        "repo-with-dashes_and_underscores.123".to_string(),
        "Test".to_string(),
        PathBuf::from("repo"),
        "https://github.com/test/repo.git".to_string(),
    );

    assert!(ws.add_repository(repo).is_ok());
}

#[test]
fn test_workspace_very_long_path() {
    // 测试长路径支持
    let long_path = "a/".repeat(100);
    let ws = Workspace::new("test".to_string(), PathBuf::from(long_path));
    assert!(ws.root_path.to_string_lossy().len() > 100);
}

#[test]
fn test_multiple_tags_and_filtering() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));

    // 创建带多个标签的仓库
    let mut repo1 = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    repo1.tags = vec![
        "frontend".to_string(),
        "react".to_string(),
        "critical".to_string(),
    ];
    repo1.enabled = true;

    let mut repo2 = RepositoryEntry::new(
        "repo2".to_string(),
        "Repo 2".to_string(),
        PathBuf::from("repo2"),
        "https://github.com/test/repo2.git".to_string(),
    );
    repo2.tags = vec!["backend".to_string(), "nodejs".to_string()];
    repo2.enabled = true;

    let mut repo3 = RepositoryEntry::new(
        "repo3".to_string(),
        "Repo 3".to_string(),
        PathBuf::from("repo3"),
        "https://github.com/test/repo3.git".to_string(),
    );
    repo3.tags = vec!["frontend".to_string(), "vue".to_string()];
    repo3.enabled = false; // 禁用

    ws.add_repository(repo1).unwrap();
    ws.add_repository(repo2).unwrap();
    ws.add_repository(repo3).unwrap();

    // 测试启用仓库过滤
    let enabled = ws.get_enabled_repositories();
    assert_eq!(enabled.len(), 2);

    // 测试标签包含
    let frontend_repos: Vec<_> = ws
        .repositories
        .iter()
        .filter(|r| r.has_tag("frontend"))
        .collect();
    assert_eq!(frontend_repos.len(), 2);
    assert!(frontend_repos.iter().any(|r| r.id == "repo1"));
    assert!(frontend_repos.iter().any(|r| r.id == "repo3"));
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_large_workspace_performance() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");

    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.max_concurrent_repos = 10;

    let mut manager = WorkspaceManager::new(config, storage_path);

    // 创建工作区
    manager
        .create_workspace("large-workspace".to_string(), PathBuf::from("/test"))
        .unwrap();

    // 添加100个仓库
    let start = std::time::Instant::now();
    for i in 0..100 {
        let repo = RepositoryEntry::new(
            format!("repo-{:03}", i),
            format!("Repository {}", i),
            PathBuf::from(format!("repos/repo-{:03}", i)),
            format!("https://github.com/test/repo-{:03}.git", i),
        );
        manager.add_repository(repo).unwrap();
    }
    let add_duration = start.elapsed();

    // 保存
    let start = std::time::Instant::now();
    manager.save_workspace().unwrap();
    let save_duration = start.elapsed();

    // 重新加载
    manager.close_workspace();
    let start = std::time::Instant::now();
    manager.load_workspace().unwrap();
    let load_duration = start.elapsed();

    // 验证
    assert_eq!(manager.get_repositories().unwrap().len(), 100);

    // 性能断言(宽松的阈值)
    assert!(add_duration.as_millis() < 500, "添加仓库不应超过500ms");
    assert!(save_duration.as_millis() < 500, "保存不应超过500ms");
    assert!(load_duration.as_millis() < 500, "加载不应超过500ms");
}

// ============================================================================
// 工作区状态服务测试
// ============================================================================

#[tokio::test]
async fn test_workspace_status_basic_and_cache() {
    let temp_dir = TempDir::new().unwrap();
    let repo1_path = temp_dir.path().join("repo-one");
    GitRepository::init(&repo1_path).expect("init repo1");
    create_commit(&repo1_path, "README.md", "hello", "feat: init repo1");
    std::fs::write(repo1_path.join("untracked.txt"), "pending change")
        .expect("create untracked file");

    let repo2_path = temp_dir.path().join("repo-two");
    GitRepository::init(&repo2_path).expect("init repo2");
    create_commit(&repo2_path, "lib.rs", "pub fn hi() {}", "feat: init repo2");

    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.status_cache_ttl_secs = 60;
    config.status_max_concurrency = 4;
    config.status_auto_refresh_secs = None;

    let status_service = WorkspaceStatusService::new(&config);

    let mut workspace = Workspace::new("ws".into(), temp_dir.path().to_path_buf());

    let mut repo1 = RepositoryEntry::new(
        "repo1".into(),
        "Repo One".into(),
        PathBuf::from("repo-one"),
        "https://example.com/repo1.git".into(),
    );
    repo1.tags = vec!["frontend".into()];
    repo1.enabled = true;

    let mut repo2 = RepositoryEntry::new(
        "repo2".into(),
        "Repo Two".into(),
        PathBuf::from("repo-two"),
        "https://example.com/repo2.git".into(),
    );
    repo2.tags = vec!["backend".into()];
    repo2.enabled = false;

    workspace.add_repository(repo1).unwrap();
    workspace.add_repository(repo2).unwrap();

    let response = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("query status");

    assert_eq!(response.total, 1);
    assert_eq!(response.refreshed, 1);
    assert_eq!(response.cached, 0);
    assert_eq!(response.statuses[0].repo_id, "repo1");
    assert_eq!(response.statuses[0].working_state, WorkingTreeState::Dirty);
    assert!(response.statuses[0].untracked >= 1);
    assert!(!response.statuses[0].is_cached);
    assert_eq!(response.summary.working_states.dirty, 1);
    assert_eq!(response.summary.working_states.clean, 0);
    assert_eq!(response.summary.sync_states.clean, 1);
    assert_eq!(response.summary.error_count, 0);

    let cached = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("query cached status");
    assert_eq!(cached.total, 1);
    assert_eq!(cached.cached, 1);
    assert_eq!(cached.refreshed, 0);
    assert!(cached.statuses[0].is_cached);
    assert_eq!(cached.summary.working_states.dirty, 1);
    assert_eq!(cached.summary.sync_states.clean, 1);

    let mut clean_filter = StatusQuery::default();
    clean_filter.filter.has_local_changes = Some(false);
    let clean = status_service
        .query_statuses(&workspace, clean_filter)
        .await
        .expect("query clean filter");
    assert_eq!(clean.total, 0);

    let mut include_disabled = StatusQuery::default();
    include_disabled.include_disabled = true;
    include_disabled.filter.tags = Some(vec!["backend".into()]);
    let with_disabled = status_service
        .query_statuses(&workspace, include_disabled)
        .await
        .expect("query disabled repo");
    assert_eq!(with_disabled.total, 1);
    assert_eq!(with_disabled.statuses[0].repo_id, "repo2");

    let mut refresh_query = StatusQuery::default();
    refresh_query.include_disabled = true;
    refresh_query.force_refresh = true;
    let refreshed = status_service
        .query_statuses(&workspace, refresh_query)
        .await
        .expect("force refresh statuses");
    assert!(refreshed.refreshed >= 1);
    assert_eq!(refreshed.summary.working_states.dirty, 1);
    assert_eq!(refreshed.summary.working_states.clean, 1);

    let mut missing = StatusQuery::default();
    missing.include_disabled = true;
    missing.repo_ids = Some(vec!["missing".into()]);
    let missing_resp = status_service
        .query_statuses(&workspace, missing)
        .await
        .expect("query missing repo");
    assert_eq!(missing_resp.total, 0);
    assert_eq!(missing_resp.missing_repo_ids, vec!["missing".to_string()]);
    assert_eq!(missing_resp.summary.working_states.clean, 0);
    assert_eq!(missing_resp.summary.error_count, 0);
}

#[tokio::test]
async fn test_workspace_status_cache_invalidation() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo-one");
    GitRepository::init(&repo_path).expect("init repo");
    create_commit(&repo_path, "main.rs", "fn main() {}", "feat: init repo");

    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.status_cache_ttl_secs = 120;
    config.status_max_concurrency = 2;

    let status_service = WorkspaceStatusService::new(&config);

    let mut workspace = Workspace::new("ws".into(), temp_dir.path().to_path_buf());
    let mut repo = RepositoryEntry::new(
        "repo1".into(),
        "Repo One".into(),
        PathBuf::from("repo-one"),
        "https://example.com/repo1.git".into(),
    );
    repo.enabled = true;
    workspace.add_repository(repo).unwrap();

    let initial = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("initial status");
    assert_eq!(initial.statuses[0].working_state, WorkingTreeState::Clean);
    assert!(!initial.statuses[0].is_cached);
    assert_eq!(initial.summary.working_states.clean, 1);

    std::fs::write(repo_path.join("dirty.txt"), "dirty").expect("create dirty file");

    let cached = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("cached status");
    assert_eq!(cached.statuses[0].working_state, WorkingTreeState::Clean);
    assert!(cached.statuses[0].is_cached);

    assert!(status_service.invalidate_repo("repo1"));

    let refreshed = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("refreshed status");
    assert_eq!(refreshed.statuses[0].working_state, WorkingTreeState::Dirty);
    assert!(!refreshed.statuses[0].is_cached);
    assert_eq!(refreshed.summary.working_states.dirty, 1);
}

#[tokio::test]
async fn test_workspace_status_summary_counts() {
    let temp_dir = TempDir::new().unwrap();
    let repo_clean_path = temp_dir.path().join("repo-clean");
    GitRepository::init(&repo_clean_path).expect("init clean repo");
    create_commit(
        &repo_clean_path,
        "main.rs",
        "fn main() {}",
        "feat: init clean",
    );

    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.status_cache_ttl_secs = 30;
    config.status_max_concurrency = 2;

    let status_service = WorkspaceStatusService::new(&config);

    let mut workspace = Workspace::new("ws".into(), temp_dir.path().to_path_buf());

    let mut clean_repo = RepositoryEntry::new(
        "repo_clean".into(),
        "Repo Clean".into(),
        PathBuf::from("repo-clean"),
        "https://example.com/clean.git".into(),
    );
    clean_repo.enabled = true;

    let mut missing_repo = RepositoryEntry::new(
        "repo_missing".into(),
        "Repo Missing".into(),
        PathBuf::from("repo-missing"),
        "https://example.com/missing.git".into(),
    );
    missing_repo.enabled = true;

    workspace.add_repository(clean_repo).unwrap();
    workspace.add_repository(missing_repo).unwrap();

    let response = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("query status summary");

    assert_eq!(response.total, 2);

    let mut clean_present = false;
    let mut missing_present = false;
    for status in &response.statuses {
        match status.repo_id.as_str() {
            "repo_clean" => {
                clean_present = true;
                assert_eq!(status.working_state, WorkingTreeState::Clean);
                assert!(status.error.is_none());
                assert_eq!(status.sync_state, SyncState::Clean);
            }
            "repo_missing" => {
                missing_present = true;
                assert_eq!(status.working_state, WorkingTreeState::Missing);
                assert_eq!(status.sync_state, SyncState::Unknown);
                let err = status
                    .error
                    .as_ref()
                    .expect("missing repo should have error");
                assert!(err.contains("not found"));
            }
            other => panic!("unexpected repo id {}", other),
        }
    }
    assert!(clean_present);
    assert!(missing_present);

    let summary = &response.summary;
    assert_eq!(summary.working_states.clean, 1);
    assert_eq!(summary.working_states.dirty, 0);
    assert_eq!(summary.working_states.missing, 1);
    assert_eq!(summary.working_states.error, 0);
    assert_eq!(summary.sync_states.clean, 1);
    assert_eq!(summary.sync_states.unknown, 1);
    assert_eq!(summary.sync_states.ahead, 0);
    assert_eq!(summary.sync_states.behind, 0);
    assert_eq!(summary.sync_states.diverged, 0);
    assert_eq!(summary.sync_states.detached, 0);
    assert_eq!(summary.error_count, 1);
    assert_eq!(summary.error_repositories, vec!["repo_missing".to_string()]);
}

fn create_commit(repo_path: &Path, file: &str, content: &str, message: &str) {
    let repo = GitRepository::open(repo_path).expect("open repo for commit");
    std::fs::write(repo_path.join(file), content).expect("write file for commit");
    let mut index = repo.index().expect("repo index");
    index.add_path(Path::new(file)).expect("add path to index");
    index.write().expect("write index");
    let tree_id = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_id).expect("find tree");
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("Workspace Tester", "tester@example.com"))
        .expect("signature");
    let parents: Vec<git2::Commit> = match repo.head() {
        Ok(head) => head
            .target()
            .map(|oid| repo.find_commit(oid).expect("find parent commit"))
            .into_iter()
            .collect(),
        Err(_) => Vec::new(),
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .expect("commit change");
}

// ============================================================================
// 工作区批量操作任务测试
// ============================================================================

#[tokio::test]
async fn test_workspace_batch_clone_success() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin1 = RepoBuilder::new()
        .with_base_commit("a.txt", "one", "c1")
        .build();
    let origin2 = RepoBuilder::new()
        .with_base_commit("b.txt", "two", "c1")
        .build();

    let dest1 = workspace_root.path().join("repo-one");
    let dest2 = workspace_root.path().join("repo-two");

    let specs = vec![
        WorkspaceBatchChildSpec {
            repo_id: "repo1".into(),
            repo_name: "Repo One".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: origin1.path.to_string_lossy().to_string(),
                dest: dest1.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
        WorkspaceBatchChildSpec {
            repo_id: "repo2".into(),
            repo_name: "Repo Two".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: origin2.path.to_string_lossy().to_string(),
                dest: dest2.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
    ];

    let operation = WorkspaceBatchOperation::Clone;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        2,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Completed);

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 2);

    for child in children {
        wait_until_task_done(registry.as_ref(), child).await;
        let snapshot = registry.snapshot(&child).unwrap();
        assert_eq!(snapshot.state, TaskState::Completed);
    }

    assert!(dest1.join(".git").exists());
    assert!(dest2.join(".git").exists());
}

#[tokio::test]
async fn test_workspace_batch_clone_failure_summary() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("c.txt", "three", "c1")
        .build();

    let good_dest = workspace_root.path().join("repo-good");
    let bad_dest = workspace_root.path().join("repo-bad");
    let missing_remote = workspace_root.path().join("missing");

    let specs = vec![
        WorkspaceBatchChildSpec {
            repo_id: "good".into(),
            repo_name: "Good".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: origin.path.to_string_lossy().to_string(),
                dest: good_dest.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
        WorkspaceBatchChildSpec {
            repo_id: "bad".into(),
            repo_name: "Bad".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: missing_remote.to_string_lossy().to_string(),
                dest: bad_dest.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
    ];

    let operation = WorkspaceBatchOperation::Clone;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        2,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Failed);

    let summary = registry.fail_reason(&parent_id).unwrap();
    assert!(summary.contains("Bad"));
    assert!(summary.contains("bad"));

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 2);

    let mut states = Vec::new();
    for child in children {
        wait_until_task_done(registry.as_ref(), child).await;
        let snapshot = registry.snapshot(&child).unwrap();
        states.push(snapshot.state);
    }
    assert!(states.contains(&TaskState::Completed));
    assert!(states.contains(&TaskState::Failed));
}

// ============================================================================
// 工作区批量 Fetch 操作测试
// ============================================================================

#[tokio::test]
async fn test_workspace_batch_fetch_success() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("a.txt", "fetch", "init")
        .build();
    let remote_dir = tempfile::tempdir().unwrap();
    let remote_path = remote_dir.path().join("remote.git");
    GitRepository::init_bare(&remote_path).expect("init bare remote");
    let origin_url = remote_path.to_string_lossy().to_string();

    let origin_repo = GitRepository::open(&origin.path).unwrap();
    origin_repo
        .remote("bare", &origin_url)
        .expect("add bare remote")
        .push(&["refs/heads/master:refs/heads/master"], None)
        .expect("seed bare remote");

    let dest = workspace_root.path().join("repo-fetch");
    GitRepository::clone(&origin_url, &dest).expect("clone repo for fetch test");

    let specs = vec![WorkspaceBatchChildSpec {
        repo_id: "fetch".into(),
        repo_name: "Fetch Repo".into(),
        operation: WorkspaceBatchChildOperation::Fetch(FetchOptions {
            repo_url: origin_url,
            dest: dest.to_string_lossy().to_string(),
            preset: None,
            depth_u32: None,
            depth_value: None,
            filter: None,
            strategy_override: None,
        }),
    }];

    let operation = WorkspaceBatchOperation::Fetch;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        1,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Completed);
    assert!(registry.fail_reason(&parent_id).is_none());

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 1);
    let child_id = children[0];
    wait_until_task_done(registry.as_ref(), child_id).await;
    let child_state = registry.snapshot(&child_id).unwrap();
    assert_eq!(child_state.state, TaskState::Completed);
}

#[tokio::test]
async fn test_workspace_batch_fetch_missing_repository() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("b.txt", "fetch-miss", "init")
        .build();
    let origin_url = origin.path.to_string_lossy().to_string();

    let dest = workspace_root.path().join("missing-repo");
    fs::create_dir_all(&dest).unwrap();

    let specs = vec![WorkspaceBatchChildSpec {
        repo_id: "missing".into(),
        repo_name: "Missing Repo".into(),
        operation: WorkspaceBatchChildOperation::Fetch(FetchOptions {
            repo_url: origin_url,
            dest: dest.to_string_lossy().to_string(),
            preset: None,
            depth_u32: None,
            depth_value: None,
            filter: None,
            strategy_override: None,
        }),
    }];

    let operation = WorkspaceBatchOperation::Fetch;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        1,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Failed);
    let summary = registry.fail_reason(&parent_id).unwrap();
    assert!(summary.starts_with("batch fetch"));
    assert!(summary.contains("Missing Repo"));

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 1);
    let child_id = children[0];
    wait_until_task_done(registry.as_ref(), child_id).await;
    let child_state = registry.snapshot(&child_id).unwrap();
    assert_eq!(child_state.state, TaskState::Failed);
}

#[tokio::test]
async fn test_workspace_batch_fetch_failure_summary_truncation() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("c.txt", "fetch-trunc", "init")
        .build();
    let origin_url = origin.path.to_string_lossy().to_string();

    let mut specs = Vec::new();
    for idx in 0..4 {
        let dest = workspace_root.path().join(format!("missing-{}", idx));
        fs::create_dir_all(&dest).unwrap();
        specs.push(WorkspaceBatchChildSpec {
            repo_id: format!("missing-{idx}"),
            repo_name: format!("Repo {idx}"),
            operation: WorkspaceBatchChildOperation::Fetch(FetchOptions {
                repo_url: origin_url.clone(),
                dest: dest.to_string_lossy().to_string(),
                preset: None,
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
            }),
        });
    }

    let operation = WorkspaceBatchOperation::Fetch;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        2,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Failed);
    let summary = registry.fail_reason(&parent_id).unwrap();
    assert!(summary.starts_with("batch fetch: 4 repository failures"));
    assert!(summary.contains("Repo 0"));
    assert!(summary.contains("... +1 more"));

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 4);
    for child in children {
        wait_until_task_done(registry.as_ref(), child).await;
        let state = registry.snapshot(&child).unwrap();
        assert_eq!(state.state, TaskState::Failed);
    }
}

// ============================================================================
// 工作区批量 Push 操作测试
// ============================================================================

#[tokio::test]
async fn test_workspace_batch_push_success() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("init.txt", "push", "init")
        .build();
    let remote_dir = tempfile::tempdir().unwrap();
    let remote_path = remote_dir.path().join("remote.git");
    GitRepository::init_bare(&remote_path).expect("init bare remote");
    let origin_url = remote_path.to_string_lossy().to_string();

    let origin_repo = GitRepository::open(&origin.path).unwrap();
    origin_repo
        .remote("bare", &origin_url)
        .expect("add bare remote")
        .push(&["refs/heads/master:refs/heads/master"], None)
        .expect("seed bare remote");
    let remote_head_before = GitRepository::open(&remote_path)
        .unwrap()
        .refname_to_id("refs/heads/master")
        .expect("remote head before push");

    let dest = workspace_root.path().join("repo-push");
    GitRepository::clone(&origin_url, &dest).expect("clone repo for push test");
    create_commit(&dest, "local.txt", "local", "feat: local change");

    let specs = vec![WorkspaceBatchChildSpec {
        repo_id: "push".into(),
        repo_name: "Push Repo".into(),
        operation: WorkspaceBatchChildOperation::Push(PushOptions {
            dest: dest.to_string_lossy().to_string(),
            remote: None,
            refspecs: Some(vec!["refs/heads/master:refs/heads/master".into()]),
            username: None,
            password: None,
            strategy_override: None,
        }),
    }];

    let operation = WorkspaceBatchOperation::Push;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        1,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Completed);
    assert!(registry.fail_reason(&parent_id).is_none());

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 1);
    let child_id = children[0];
    wait_until_task_done(registry.as_ref(), child_id).await;
    let child_state = registry.snapshot(&child_id).unwrap();
    assert_eq!(child_state.state, TaskState::Completed);

    let remote_head_after = GitRepository::open(&remote_path)
        .unwrap()
        .refname_to_id("refs/heads/master")
        .expect("remote head after push");
    assert_ne!(remote_head_before, remote_head_after);
}

#[tokio::test]
async fn test_workspace_batch_push_missing_remote() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("init.txt", "push", "init")
        .build();
    let remote_dir = tempfile::tempdir().unwrap();
    let remote_path = remote_dir.path().join("remote.git");
    GitRepository::init_bare(&remote_path).expect("init bare remote");
    let origin_url = remote_path.to_string_lossy().to_string();
    let origin_repo = GitRepository::open(&origin.path).unwrap();
    origin_repo
        .remote("bare", &origin_url)
        .expect("add bare remote")
        .push(&["refs/heads/master:refs/heads/master"], None)
        .expect("seed bare remote");

    let dest = workspace_root.path().join("repo-push-missing");
    GitRepository::clone(&origin_url, &dest).expect("clone repo for push failure test");
    create_commit(&dest, "local.txt", "local", "feat: local change");

    let repo = GitRepository::open(&dest).unwrap();
    repo.remote_delete("origin").expect("remove origin remote");

    let specs = vec![WorkspaceBatchChildSpec {
        repo_id: "push-missing".into(),
        repo_name: "Push Missing".into(),
        operation: WorkspaceBatchChildOperation::Push(PushOptions {
            dest: dest.to_string_lossy().to_string(),
            remote: None,
            refspecs: None,
            username: None,
            password: None,
            strategy_override: None,
        }),
    }];

    let operation = WorkspaceBatchOperation::Push;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        1,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Failed);
    let summary = registry.fail_reason(&parent_id).unwrap();
    assert!(summary.starts_with("batch push"));
    assert!(summary.contains("Push Missing"));

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 1);
    let child_id = children[0];
    wait_until_task_done(registry.as_ref(), child_id).await;
    let child_state = registry.snapshot(&child_id).unwrap();
    assert_eq!(child_state.state, TaskState::Failed);
}

#[tokio::test]
async fn test_workspace_batch_push_failure_summary_truncation() {
    let registry = Arc::new(TaskRegistry::new());
    let workspace_root = TempDir::new().unwrap();

    let origin = RepoBuilder::new()
        .with_base_commit("init.txt", "push", "init")
        .build();
    let remote_dir = tempfile::tempdir().unwrap();
    let remote_path = remote_dir.path().join("remote.git");
    GitRepository::init_bare(&remote_path).expect("init bare remote");
    let origin_url = remote_path.to_string_lossy().to_string();

    let origin_repo = GitRepository::open(&origin.path).unwrap();
    origin_repo
        .remote("bare", &origin_url)
        .expect("add bare remote")
        .push(&["refs/heads/master:refs/heads/master"], None)
        .expect("seed bare remote");

    let mut specs = Vec::new();
    for idx in 0..4 {
        let dest = workspace_root.path().join(format!("repo-push-fail-{idx}"));
        GitRepository::clone(&origin_url, &dest).expect("clone repo for push truncation test");
        let file_name = format!("local-{idx}.txt");
        let payload = format!("payload-{idx}");
        create_commit(&dest, &file_name, &payload, "feat: local change");

        let repo = GitRepository::open(&dest).unwrap();
        repo.remote_delete("origin").expect("remove origin remote");

        specs.push(WorkspaceBatchChildSpec {
            repo_id: format!("push-fail-{idx}"),
            repo_name: format!("Push Repo {idx}"),
            operation: WorkspaceBatchChildOperation::Push(PushOptions {
                dest: dest.to_string_lossy().to_string(),
                remote: None,
                refspecs: Some(vec!["refs/heads/master:refs/heads/master".into()]),
                username: None,
                password: None,
                strategy_override: None,
            }),
        });
    }

    let operation = WorkspaceBatchOperation::Push;
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        operation,
        specs,
        2,
    );

    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Failed);
    let summary = registry.fail_reason(&parent_id).unwrap();
    assert!(summary.starts_with("batch push: 4 repository failures"));
    assert!(summary.contains("Push Repo 0"));
    assert!(summary.contains("... +1 more"));

    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 4);
    for child in children {
        wait_until_task_done(registry.as_ref(), child).await;
        let child_state = registry.snapshot(&child).unwrap();
        assert_eq!(child_state.state, TaskState::Failed);
    }
}

// ============================================================================
// P7.6 稳定性验证测试
// ============================================================================

#[tokio::test]
async fn test_workspace_clone_with_submodule_and_config_roundtrip() {
    let sandbox = TempDir::new().unwrap();
    let workspace_file = sandbox.path().join("workspace.json");
    let workspace_root = sandbox.path().join("workspace-root");
    fs::create_dir_all(&workspace_root).expect("create workspace root");

    let upstream_root = sandbox.path().join("upstream");
    fs::create_dir_all(&upstream_root).expect("create upstream root");
    let upstream_remote = create_remote_with_submodule(&upstream_root);

    let mut cfg = WorkspaceConfig::default();
    cfg.enabled = true;
    let mut manager = WorkspaceManager::new(cfg.clone(), workspace_file.clone());
    manager
        .create_workspace("p7.6-readiness".into(), workspace_root.clone())
        .expect("create workspace");

    let mut entry = RepositoryEntry::new(
        "primary".into(),
        "Primary Repo".into(),
        PathBuf::from("primary"),
        upstream_remote.to_string_lossy().to_string(),
    );
    entry.has_submodules = true;
    entry.tags = vec!["p7.6".into()];
    manager.add_repository(entry.clone()).expect("add repo");
    manager.save_workspace().expect("persist workspace");

    let storage = WorkspaceStorage::new(workspace_file.clone());
    storage.validate().expect("validate workspace");
    let backup_path = storage.backup().expect("backup workspace");
    storage
        .restore_from_backup(&backup_path)
        .expect("restore from backup");
    storage.validate().expect("validate after restore");

    manager.close_workspace();
    let mut manager = WorkspaceManager::new(cfg.clone(), workspace_file.clone());
    manager.load_workspace().expect("reload workspace");
    let workspace_snapshot = manager.current_workspace().unwrap().clone();
    assert_eq!(
        workspace_snapshot.repositories.len(),
        1,
        "reload should restore repository list"
    );
    let reloaded_repo = workspace_snapshot.repositories[0].clone();
    assert_eq!(
        reloaded_repo.tags, entry.tags,
        "repository tags should persist after roundtrip"
    );
    assert_eq!(
        reloaded_repo.path, entry.path,
        "repository path should persist after roundtrip"
    );
    entry = reloaded_repo;

    let registry = Arc::new(TaskRegistry::new());
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: WorkspaceBatchOperation::Clone,
        total: 1,
    });

    let dest = workspace_root.join("primary");
    let clone_spec = WorkspaceBatchChildSpec {
        repo_id: "primary".into(),
        repo_name: "Primary Repo".into(),
        operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
            repo_url: entry.remote_url.clone(),
            dest: dest.to_string_lossy().to_string(),
            depth_u32: None,
            depth_value: None,
            filter: None,
            strategy_override: None,
            recurse_submodules: true,
        }),
    };

    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token.clone(),
        WorkspaceBatchOperation::Clone,
        vec![clone_spec],
        4,
    );
    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_snapshot = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_snapshot.state, TaskState::Completed);
    assert!(dest.join(".git").exists(), "expected cloned repository");
    assert!(
        dest.join(".gitmodules").exists(),
        "expected .gitmodules in cloned repository"
    );
    assert!(
        dest.join("modules/child/.git").exists(),
        "expected submodule git directory to be populated"
    );

    let submodule_manager = SubmoduleManager::new(SubmoduleConfig::default());
    let submodules = submodule_manager
        .list_submodules(&dest)
        .expect("list submodules");
    assert_eq!(submodules.len(), 1, "expected cloned submodule");
    assert!(submodules[0].cloned, "submodule should be cloned");

    let fetch_registry = Arc::new(TaskRegistry::new());
    let (fetch_parent, fetch_token) = fetch_registry.create(TaskKind::WorkspaceBatch {
        operation: WorkspaceBatchOperation::Fetch,
        total: 1,
    });
    let fetch_spec = WorkspaceBatchChildSpec {
        repo_id: "primary".into(),
        repo_name: "Primary Repo".into(),
        operation: WorkspaceBatchChildOperation::Fetch(FetchOptions {
            repo_url: entry.remote_url.clone(),
            dest: dest.to_string_lossy().to_string(),
            preset: None,
            depth_u32: None,
            depth_value: None,
            filter: None,
            strategy_override: None,
        }),
    };
    let fetch_handle = fetch_registry.clone().spawn_workspace_batch_task(
        None,
        fetch_parent,
        fetch_token.clone(),
        WorkspaceBatchOperation::Fetch,
        vec![fetch_spec],
        1,
    );
    fetch_handle.await.unwrap();
    wait_until_task_done(fetch_registry.as_ref(), fetch_parent).await;
    let fetch_snapshot = fetch_registry.snapshot(&fetch_parent).unwrap();
    assert_eq!(fetch_snapshot.state, TaskState::Completed);

    let mut status_cfg = WorkspaceConfig::default();
    status_cfg.enabled = true;
    status_cfg.status_max_concurrency = 4;
    let status_service = WorkspaceStatusService::new(&status_cfg);
    let status_response = status_service
        .query_statuses(&workspace_snapshot, StatusQuery::default())
        .await
        .expect("query workspace status");
    assert_eq!(status_response.total, 1);
    assert_eq!(status_response.summary.error_count, 0);
    assert!(
        status_response.statuses[0]
            .tags
            .contains(&"p7.6".to_string()),
        "expected readiness tag"
    );
    assert!(
        !status_response.statuses[0].is_cached,
        "initial status query should be live"
    );
    assert_eq!(status_response.summary.sync_states.clean, 1);

    let cached_status = status_service
        .query_statuses(&workspace_snapshot, StatusQuery::default())
        .await
        .expect("query cached workspace status");
    assert_eq!(cached_status.cached, 1, "subsequent query should hit cache");
    assert!(
        cached_status.statuses[0].is_cached,
        "status entry should be cached on repeat query"
    );
}

#[tokio::test]
async fn test_workspace_batch_clone_performance_targets() {
    let baseline = measure_clone_batch(1, 1).await;
    eprintln!("workspace_batch_clone: count=1 per_repo={baseline:.2}ms");
    for &count in &[10usize, 50, 100] {
        let per_repo = measure_clone_batch(count, count.min(20)).await;
        eprintln!(
            "workspace_batch_clone: count={count} per_repo={per_repo:.2}ms baseline={baseline:.2}ms"
        );
        assert!(
            per_repo <= baseline * 2.0,
            "per-repo latency {per_repo:.2}ms exceeded baseline {baseline:.2}ms"
        );
    }
}

#[tokio::test]
async fn test_workspace_status_performance_targets() {
    let baseline = measure_status_query(1, 1).await;
    eprintln!("workspace_status: count=1 per_repo={baseline:.2}ms total={baseline:.2}ms");
    for &(count, concurrency) in &[(10usize, 4usize), (50, 10), (100, 16)] {
        let per_repo = measure_status_query(count, concurrency).await;
        let total = per_repo * count as f64;
        // 动态阈值策略：
        // - 基础：baseline * 2 + 10ms（原逻辑）
        // - Windows 下文件 IO/进程调度抖动更大，增加 5ms 固定缓冲，并确保至少 baseline * 2 + 15ms
        // - 同时对较大批次按 log2(count) 给予额外缓冲（最多 ~7ms），避免规模线性放大时偶发抖动
        let mut threshold = baseline * 2.0 + 10.0;
        #[cfg(windows)]
        {
            let scale_buf = (count as f64).log2();
            threshold = (baseline * 2.0 + 15.0).max(threshold + 5.0 + scale_buf);
        }
        eprintln!(
            "workspace_status: count={count} concurrency={concurrency} per_repo={per_repo:.2}ms total={total:.2}ms baseline={baseline:.2}ms threshold={threshold:.2}ms"
        );
        assert!(
            per_repo <= threshold,
            "status per-repo latency {per_repo:.2}ms exceeded threshold {threshold:.2}ms (baseline {baseline:.2}ms)"
        );
        assert!(
            total <= 3_000.0,
            "status query total latency {total:.2}ms exceeded 3000ms target"
        );
    }
}

async fn measure_clone_batch(total: usize, concurrency: usize) -> f64 {
    let origin = RepoBuilder::new()
        .with_base_commit("bench.txt", "benchmark", "init bench")
        .build();
    let bare_dir = tempfile::tempdir().expect("create bare remote dir");
    let bare_path = bare_dir.path().join("remote.git");
    GitRepository::init_bare(&bare_path).expect("init bare remote");
    let origin_repo = GitRepository::open(&origin.path).expect("open origin repo");
    let bare_url = bare_path.to_string_lossy().to_string();
    origin_repo
        .remote("bench", bare_url.as_str())
        .expect("add bench remote")
        .push(&["refs/heads/master:refs/heads/master"], None)
        .expect("seed bare remote");

    let workspace_root = TempDir::new().expect("create workspace root");
    let registry = Arc::new(TaskRegistry::new());
    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: WorkspaceBatchOperation::Clone,
        total: total as u32,
    });

    let specs: Vec<_> = (0..total)
        .map(|idx| {
            let dest = workspace_root.path().join(format!("bench-repo-{idx}"));
            WorkspaceBatchChildSpec {
                repo_id: format!("bench-{idx}"),
                repo_name: format!("Benchmark Repo {idx}"),
                operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                    repo_url: bare_url.clone(),
                    dest: dest.to_string_lossy().to_string(),
                    depth_u32: None,
                    depth_value: None,
                    filter: None,
                    strategy_override: None,
                    recurse_submodules: false,
                }),
            }
        })
        .collect();

    let started = Instant::now();
    let handle = registry.clone().spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        WorkspaceBatchOperation::Clone,
        specs,
        concurrency.max(1),
    );
    handle.await.unwrap();
    wait_until_task_done(registry.as_ref(), parent_id).await;

    let parent_state = registry.snapshot(&parent_id).unwrap();
    assert_eq!(parent_state.state, TaskState::Completed);

    let elapsed = started.elapsed();
    (elapsed.as_secs_f64() * 1_000.0) / total.max(1) as f64
}

async fn measure_status_query(total: usize, concurrency: usize) -> f64 {
    let workspace_root = TempDir::new().expect("create status workspace root");
    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.status_cache_ttl_secs = 1;
    config.status_max_concurrency = concurrency.max(1);
    config.status_auto_refresh_secs = None;

    let status_service = WorkspaceStatusService::new(&config);
    let mut workspace = Workspace::new(
        format!("status-bench-{total}"),
        workspace_root.path().to_path_buf(),
    );

    for idx in 0..total {
        let repo_dir = workspace_root.path().join(format!("status-repo-{idx}"));
        GitRepository::init(&repo_dir).expect("init status repo");
        let file_name = format!("file-{idx}.txt");
        let commit_message = format!("feat: init status repo {idx}");
        create_commit(&repo_dir, &file_name, "workspace bench", &commit_message);
        if idx % 3 == 0 {
            fs::write(repo_dir.join("untracked.txt"), "pending changes")
                .expect("write untracked file");
        }

        let mut entry = RepositoryEntry::new(
            format!("status-{idx}"),
            format!("Status Repo {idx}"),
            PathBuf::from(format!("status-repo-{idx}")),
            format!("https://example.com/status-{idx}.git"),
        );
        entry.enabled = true;
        if idx % 2 == 0 {
            entry.tags = vec!["bench".into()];
        }
        workspace.add_repository(entry).expect("add status repo");
    }

    let started = Instant::now();
    let response = status_service
        .query_statuses(&workspace, StatusQuery::default())
        .await
        .expect("collect status metrics");
    assert_eq!(response.total, total);
    let elapsed = started.elapsed();
    (elapsed.as_secs_f64() * 1_000.0) / total.max(1) as f64
}

fn create_remote_with_submodule(base: &Path) -> PathBuf {
    let submodule_path = base.join("submodule-src");
    fs::create_dir_all(&submodule_path).expect("create submodule dir");
    run_git(Some(&submodule_path), &["init"]);
    run_git(
        Some(&submodule_path),
        &["config", "user.email", "ci@example.com"],
    );
    run_git(Some(&submodule_path), &["config", "user.name", "CI Tester"]);
    fs::write(submodule_path.join("README.md"), "submodule\n").expect("write submodule file");
    run_git(Some(&submodule_path), &["add", "README.md"]);
    run_git(Some(&submodule_path), &["commit", "-m", "init submodule"]);

    let parent_path = base.join("workspace-src");
    fs::create_dir_all(&parent_path).expect("create parent dir");
    run_git(Some(&parent_path), &["init"]);
    run_git(
        Some(&parent_path),
        &["config", "user.email", "ci@example.com"],
    );
    run_git(Some(&parent_path), &["config", "user.name", "CI Tester"]);
    run_git(
        Some(&parent_path),
        &["config", "protocol.file.allow", "always"],
    );
    fs::write(parent_path.join("README.md"), "workspace\n").expect("write workspace file");
    run_git(Some(&parent_path), &["add", "README.md"]);
    run_git(Some(&parent_path), &["commit", "-m", "init workspace"]);

    let submodule_url = submodule_path.to_string_lossy().to_string();
    run_git(
        Some(&parent_path),
        &["submodule", "add", submodule_url.as_str(), "modules/child"],
    );
    run_git(Some(&parent_path), &["commit", "-am", "add submodule"]);

    let bare_path = base.join("workspace-src.git");
    let parent_url = parent_path.to_string_lossy().to_string();
    let bare_url = bare_path.to_string_lossy().to_string();
    run_git(
        None,
        &["clone", "--bare", parent_url.as_str(), bare_url.as_str()],
    );
    bare_path
}

fn run_git(dir: Option<&Path>, args: &[&str]) {
    let mut cmd = Command::new("git");
    if let Some(path) = dir {
        cmd.current_dir(path);
    }
    cmd.env("GIT_ALLOW_PROTOCOL", "file");
    cmd.args(args);
    let status = cmd.status().expect("run git command");
    assert!(status.success(), "git command failed: {:?} {:?}", dir, args);
}
