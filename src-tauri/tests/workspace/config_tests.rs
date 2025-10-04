use fireworks_collaboration_lib::core::workspace::config::{WorkspaceConfigManager, PartialWorkspaceConfig};
use fireworks_collaboration_lib::core::workspace::model::WorkspaceConfig;

#[test]
fn test_new_with_defaults() {
    let mgr = WorkspaceConfigManager::with_defaults();
    assert!(!mgr.is_enabled());
    assert_eq!(mgr.max_concurrent_repos(), 3);
    assert!(mgr.default_template().is_none());
    assert!(mgr.workspace_file().is_none());
}

#[test]
fn test_update_config() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    let new_config = WorkspaceConfig {
        enabled: true,
        max_concurrent_repos: 5,
        ..Default::default()
    };

    assert!(mgr.update_config(new_config).is_ok());
    assert!(mgr.is_enabled());
    assert_eq!(mgr.max_concurrent_repos(), 5);
}

#[test]
fn test_update_config_invalid() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    let invalid_config = WorkspaceConfig {
        max_concurrent_repos: 0, // 无效值
        ..Default::default()
    };

    // 通过update_config间接验证
    assert!(mgr.update_config(invalid_config).is_err());
}

#[test]
fn test_set_enabled() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    mgr.set_enabled(true);
    assert!(mgr.is_enabled());
}

#[test]
fn test_set_max_concurrent_repos() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    assert!(mgr.set_max_concurrent_repos(10).is_ok());
    assert_eq!(mgr.max_concurrent_repos(), 10);

    // 设置为 0 应该失败
    assert!(mgr.set_max_concurrent_repos(0).is_err());
}

#[test]
fn test_set_default_template() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    mgr.set_default_template(Some("my-template".to_string()));
    assert_eq!(mgr.default_template(), Some("my-template"));

    mgr.set_default_template(None);
    assert!(mgr.default_template().is_none());
}

#[test]
fn test_merge_config() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    let partial = PartialWorkspaceConfig {
        enabled: Some(true),
        max_concurrent_repos: Some(7),
        default_template: Some("new-template".to_string()),
        workspace_file: None,
    };

    assert!(mgr.merge_config(partial).is_ok());
    assert!(mgr.is_enabled());
    assert_eq!(mgr.max_concurrent_repos(), 7);
    assert_eq!(mgr.default_template(), Some("new-template"));
}

#[test]
fn test_merge_config_invalid() {
    let mut mgr = WorkspaceConfigManager::with_defaults();
    
    let partial = PartialWorkspaceConfig {
        enabled: None,
        max_concurrent_repos: Some(0), // 无效值
        default_template: None,
        workspace_file: None,
    };

    assert!(mgr.merge_config(partial).is_err());
}
