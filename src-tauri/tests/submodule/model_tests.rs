use fireworks_collaboration_lib::core::submodule::{SubmoduleInfo, SubmoduleConfig, SubmoduleOperation, SubmoduleProgressEvent};
use std::path::PathBuf;

#[test]
fn test_submodule_info_creation() {
    let info = SubmoduleInfo {
        name: "test-submodule".to_string(),
        path: PathBuf::from("libs/test"),
        url: "https://github.com/test/repo.git".to_string(),
        head_id: Some("abc123".to_string()),
        branch: Some("main".to_string()),
        initialized: true,
        cloned: true,
    };
    
    assert_eq!(info.name, "test-submodule");
    assert_eq!(info.path, PathBuf::from("libs/test"));
    assert_eq!(info.url, "https://github.com/test/repo.git");
    assert!(info.initialized);
    assert!(info.cloned);
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
fn test_submodule_config_serde() {
    let config = SubmoduleConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: SubmoduleConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(config, deserialized);
}

#[test]
fn test_submodule_operation_as_str() {
    assert_eq!(SubmoduleOperation::Init.as_str(), "init");
    assert_eq!(SubmoduleOperation::Update.as_str(), "update");
    assert_eq!(SubmoduleOperation::Sync.as_str(), "sync");
    assert_eq!(SubmoduleOperation::RecursiveClone.as_str(), "recursive_clone");
}

#[test]
fn test_submodule_progress_event_serde() {
    let event = SubmoduleProgressEvent {
        parent_task_id: Some(uuid::Uuid::new_v4()),
        submodule_name: "test".to_string(),
        operation: SubmoduleOperation::Update,
        percent: 50,
        depth: Some(1),
        total_submodules: Some(5),
        processed_submodules: Some(2),
    };
    
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: SubmoduleProgressEvent = serde_json::from_str(&json).unwrap();
    
    assert_eq!(event.submodule_name, deserialized.submodule_name);
    assert_eq!(event.percent, deserialized.percent);
}
