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
use fireworks_collaboration_lib::core::tasks::{
    model::{TaskKind, TaskState, WorkspaceBatchOperation},
    registry::TaskRegistry,
    workspace_batch::{
        CloneOptions, FetchOptions, PushOptions, WorkspaceBatchChildOperation,
        WorkspaceBatchChildSpec,
    },
};
use fireworks_collaboration_lib::core::workspace::{
    RepositoryEntry, Workspace, WorkspaceConfig, WorkspaceConfigManager, WorkspaceManager,
    WorkspaceStorage,
};
use git2::{Repository as GitRepository, Signature};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
