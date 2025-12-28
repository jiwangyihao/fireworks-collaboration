//! 子模块集成测试
//!
//! 测试子模块操作的端到端流程

#[path = "../common/mod.rs"]
mod common;

use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

use fireworks_collaboration_lib::core::submodule::{SubmoduleConfig, SubmoduleManager};
use git2::Repository;
use tempfile::TempDir;

#[test]
fn test_submodule_manager_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // 创建一个空仓库
    Repository::init(repo_path).unwrap();

    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let submodules = manager.list_submodules(repo_path).unwrap();
    assert_eq!(submodules.len(), 0);
}

#[test]
fn test_submodule_manager_has_submodules_false() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // 创建一个空仓库
    Repository::init(repo_path).unwrap();

    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let has_subs = manager.has_submodules(repo_path).unwrap();
    assert!(!has_subs);
}

#[test]
fn test_submodule_config_defaults() {
    let config = SubmoduleConfig::default();

    assert!(config.auto_recurse);
    assert_eq!(config.max_depth, 5);
    assert!(config.auto_init_on_clone);
    assert!(config.recursive_update);
    assert!(!config.parallel);
    assert_eq!(config.max_parallel, 3);
}

#[test]
fn test_submodule_manager_with_custom_config() {
    let mut config = SubmoduleConfig::default();
    config.max_depth = 3;
    config.parallel = true;
    config.max_parallel = 5;

    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config.clone(), runner);
    assert_eq!(manager.config(), &config);
}

#[test]
fn test_init_nonexistent_repository() {
    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let result = manager.init_all("/nonexistent/path");
    assert!(result.is_err());
}

#[test]
fn test_update_nonexistent_repository() {
    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let result = manager.update_all("/nonexistent/path", 0);
    assert!(result.is_err());
}

#[test]
fn test_sync_nonexistent_repository() {
    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let result = manager.sync_all("/nonexistent/path");
    assert!(result.is_err());
}

#[test]
fn test_max_depth_enforcement() {
    let mut config = SubmoduleConfig::default();
    config.max_depth = 2;

    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    Repository::init(repo_path).unwrap();

    // 尝试在深度 2 进行更新应该失败
    let result = manager.update_all(repo_path, 2);
    assert!(result.is_err());
}

#[test]
fn test_submodule_manager_list_handles_empty_repo() {
    let temp_dir = TempDir::new().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();
    if let Ok(mut cfg) = repo.config() {
        if cfg.get_entry("user.name").is_err() {
            let _ = cfg.set_str("user.name", "Test User");
        }
        if cfg.get_entry("user.email").is_err() {
            let _ = cfg.set_str("user.email", "test@example.com");
        }
    }

    // 创建一个初始提交
    let sig = repo.signature().unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .unwrap();

    let config = SubmoduleConfig::default();
    let runner = Box::new(fireworks_collaboration_lib::core::git::CliGitRunner::new());
    let manager = SubmoduleManager::new(config, runner);

    let submodules = manager.list_submodules(temp_dir.path()).unwrap();
    assert_eq!(submodules.len(), 0);
}
