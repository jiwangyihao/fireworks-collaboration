//! P7.0 工作区模块集成测试
//!
//! 测试覆盖：
//! - 工作区数据模型的创建、修改、序列化
//! - 工作区配置的加载、验证、热更新
//! - 工作区存储的读写、备份、恢复
//! - WorkspaceManager 的完整工作流

use fireworks_collaboration_lib::core::workspace::{
    RepositoryEntry, Workspace, WorkspaceConfig, WorkspaceConfigManager, WorkspaceManager,
    WorkspaceStorage,
};
use std::path::PathBuf;
use tempfile::TempDir;

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
    println!("Backup created at: {:?}", backup_path);
    
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
    assert_eq!(restored.name, "original", "恢复后的名称应该是 'original'");
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

#[test]
fn test_workspace_storage_duplicate_id_detection() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);
    
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    
    // 直接操作内部数据添加重复 ID
    ws.repositories.push(RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    ));
    ws.repositories.push(RepositoryEntry::new(
        "repo1".to_string(), // 重复 ID
        "Repo 2".to_string(),
        PathBuf::from("repo2"),
        "https://github.com/test/repo2.git".to_string(),
    ));
    
    storage.save(&ws).unwrap();
    
    // 验证应该失败
    assert!(storage.validate().is_err());
}

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
    }).unwrap();
    
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
    
    ws.metadata.insert("team".to_string(), "platform".to_string());
    ws.metadata.insert("project".to_string(), "main".to_string());
    
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
    repo.custom_config.insert(
        "testCommand".to_string(),
        serde_json::json!("npm test"),
    );
    
    assert_eq!(repo.custom_config.len(), 2);
    
    // 测试序列化
    let json = serde_json::to_string(&repo).unwrap();
    let deserialized: RepositoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.custom_config, repo.custom_config);
}

// ===== 边界条件和错误处理测试 =====

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
    repo1.tags = vec!["frontend".to_string(), "react".to_string(), "critical".to_string()];
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
    let frontend_repos: Vec<_> = ws.repositories
        .iter()
        .filter(|r| r.has_tag("frontend"))
        .collect();
    assert_eq!(frontend_repos.len(), 2);
    assert!(frontend_repos.iter().any(|r| r.id == "repo1"));
    assert!(frontend_repos.iter().any(|r| r.id == "repo3"));
}

#[test]
fn test_workspace_timestamp_update() {
    let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
    let original_updated_at = ws.updated_at.clone();
    
    // 等待以确保时间戳变化
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // 添加仓库应更新时间戳
    let repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    ws.add_repository(repo).unwrap();
    
    assert_ne!(ws.updated_at, original_updated_at);
}

#[test]
fn test_config_manager_partial_merge() {
    use fireworks_collaboration_lib::core::workspace::PartialWorkspaceConfig;
    
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    // 部分更新(只更新enabled)
    let partial = PartialWorkspaceConfig {
        enabled: Some(true),
        max_concurrent_repos: None,
        default_template: None,
        workspace_file: None,
    };
    
    assert!(mgr.merge_config(partial).is_ok());
    assert_eq!(mgr.is_enabled(), true);
    assert_eq!(mgr.max_concurrent_repos(), 3); // 保持默认值
}

#[test]
fn test_storage_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    
    // 写入无效JSON
    std::fs::write(&file_path, "{ invalid json }").unwrap();
    
    let storage = WorkspaceStorage::new(file_path);
    
    // 加载应失败
    assert!(storage.load().is_err());
    
    // 验证应失败
    assert!(storage.validate().is_err());
}

#[test]
fn test_workspace_concurrent_modification() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");
    
    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    
    let mut manager = WorkspaceManager::new(config, storage_path);
    
    // 创建工作区
    manager.create_workspace("test".to_string(), PathBuf::from("/test")).unwrap();
    
    // 添加多个仓库
    for i in 0..10 {
        let repo = RepositoryEntry::new(
            format!("repo{}", i),
            format!("Repo {}", i),
            PathBuf::from(format!("repo{}", i)),
            format!("https://github.com/test/repo{}.git", i),
        );
        manager.add_repository(repo).unwrap();
    }
    
    assert_eq!(manager.get_repositories().unwrap().len(), 10);
    
    // 保存后重新加载
    manager.save_workspace().unwrap();
    manager.close_workspace();
    manager.load_workspace().unwrap();
    
    assert_eq!(manager.get_repositories().unwrap().len(), 10);
}

#[test]
fn test_backward_compatibility() {
    // 测试旧版配置(无workspace字段)反序列化
    let old_config_json = r#"{
        "http": { "fake_sni_enabled": true },
        "tls": { "san_whitelist": [] },
        "logging": { "log_level": "info" }
    }"#;
    
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    let config: AppConfig = serde_json::from_str(old_config_json).unwrap();
    
    // workspace应使用默认值
    assert_eq!(config.workspace.enabled, false);
    assert_eq!(config.workspace.max_concurrent_repos, 3);
}

// ===== 性能和压力测试 =====

#[test]
fn test_large_workspace_performance() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("workspace.json");
    
    let mut config = WorkspaceConfig::default();
    config.enabled = true;
    config.max_concurrent_repos = 10;
    
    let mut manager = WorkspaceManager::new(config, storage_path);
    
    // 创建工作区
    manager.create_workspace("large-workspace".to_string(), PathBuf::from("/test")).unwrap();
    
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
    println!("添加100个仓库耗时: {:?}", add_duration);
    
    // 保存
    let start = std::time::Instant::now();
    manager.save_workspace().unwrap();
    let save_duration = start.elapsed();
    println!("保存100个仓库耗时: {:?}", save_duration);
    
    // 检查文件大小
    let file_size = std::fs::metadata(manager.storage().file_path()).unwrap().len();
    println!("工作区文件大小: {} bytes ({:.2} KB)", file_size, file_size as f64 / 1024.0);
    
    // 重新加载
    manager.close_workspace();
    let start = std::time::Instant::now();
    manager.load_workspace().unwrap();
    let load_duration = start.elapsed();
    println!("加载100个仓库耗时: {:?}", load_duration);
    
    // 验证
    assert_eq!(manager.get_repositories().unwrap().len(), 100);
    
    // 性能断言(宽松的阈值)
    assert!(add_duration.as_millis() < 500, "添加仓库不应超过500ms");
    assert!(save_duration.as_millis() < 500, "保存不应超过500ms");
    assert!(load_duration.as_millis() < 500, "加载不应超过500ms");
    assert!(file_size < 100_000, "100个仓库的文件大小不应超过100KB");
}

#[test]
fn test_workspace_serialization_format() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path.clone());
    
    let mut ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/test"));
    
    let mut repo = RepositoryEntry::new(
        "repo1".to_string(),
        "Test Repo".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1.git".to_string(),
    );
    repo.tags = vec!["tag1".to_string(), "tag2".to_string()];
    ws.add_repository(repo).unwrap();
    
    storage.save(&ws).unwrap();
    
    // 读取并验证JSON格式
    let content = std::fs::read_to_string(file_path).unwrap();
    assert!(content.contains("\"name\": \"test-workspace\""));
    assert!(content.contains("\"rootPath\":") || content.contains("\"root_path\""));
    assert!(content.contains("\"repositories\""));
    assert!(content.contains("\"createdAt\":") || content.contains("\"created_at\""));
    
    // 验证可以手动解析
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["name"].is_string());
    assert!(parsed["repositories"].is_array());
}

#[test]
fn test_config_max_concurrent_repos_boundary() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    // 测试边界值
    assert!(mgr.set_max_concurrent_repos(1).is_ok());
    assert_eq!(mgr.max_concurrent_repos(), 1);
    
    assert!(mgr.set_max_concurrent_repos(100).is_ok());
    assert_eq!(mgr.max_concurrent_repos(), 100);
    
    // 高于推荐值应有警告但允许
    assert!(mgr.set_max_concurrent_repos(200).is_ok());
    assert_eq!(mgr.max_concurrent_repos(), 200);
    
    // 0应失败
    assert!(mgr.set_max_concurrent_repos(0).is_err());
}
