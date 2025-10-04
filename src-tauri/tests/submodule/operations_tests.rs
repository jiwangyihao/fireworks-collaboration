use fireworks_collaboration_lib::core::submodule::{SubmoduleManager, SubmoduleConfig, SubmoduleError};
use git2::Repository;
use tempfile::TempDir;

fn create_test_repo() -> (TempDir, Repository) {
    let temp_dir = TempDir::new().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();
    (temp_dir, repo)
}

#[test]
fn test_submodule_manager_creation() {
    let config = SubmoduleConfig::default();
    let manager = SubmoduleManager::new(config.clone());
    assert_eq!(manager.config(), &config);
}

#[test]
fn test_list_submodules_empty() {
    let (_temp, repo) = create_test_repo();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());
    
    let submodules = manager.list_submodules(repo.path().parent().unwrap()).unwrap();
    assert_eq!(submodules.len(), 0);
}

#[test]
fn test_has_submodules_empty() {
    let (_temp, repo) = create_test_repo();
    let manager = SubmoduleManager::new(SubmoduleConfig::default());
    
    let has_subs = manager.has_submodules(repo.path().parent().unwrap()).unwrap();
    assert!(!has_subs);
}

#[test]
fn test_repository_not_found() {
    let manager = SubmoduleManager::new(SubmoduleConfig::default());
    let result = manager.list_submodules("/nonexistent/path");
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SubmoduleError::RepositoryNotFound(_)));
}

#[test]
fn test_submodule_error_category() {
    assert_eq!(SubmoduleError::SubmoduleNotFound("test".into()).category(), "NotFound");
    assert_eq!(SubmoduleError::MaxDepthExceeded(5).category(), "Limit");
    assert_eq!(SubmoduleError::InvalidConfig("test".into()).category(), "Config");
}

#[test]
fn test_max_depth_check() {
    let config = SubmoduleConfig {
        max_depth: 2,
        ..Default::default()
    };
    let manager = SubmoduleManager::new(config);
    let (_temp, repo) = create_test_repo();
    
    let result = manager.update_all(repo.path().parent().unwrap(), 2);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SubmoduleError::MaxDepthExceeded(2)));
}
