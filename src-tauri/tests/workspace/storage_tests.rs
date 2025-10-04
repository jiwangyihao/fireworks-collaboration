use fireworks_collaboration_lib::core::workspace::{Workspace, WorkspaceStorage};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_workspace() -> Workspace {
    Workspace::new("test-workspace".to_string(), PathBuf::from("/test"))
}

#[test]
fn test_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let workspace = create_test_workspace();

    // 保存
    assert!(storage.save(&workspace).is_ok());
    assert!(storage.exists());

    // 加载
    let loaded = storage.load().unwrap();
    assert_eq!(loaded.name, workspace.name);
    assert_eq!(loaded.root_path, workspace.root_path);
}

#[test]
fn test_load_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nonexistent.json");
    let storage = WorkspaceStorage::new(file_path);

    assert!(storage.load().is_err());
}

#[test]
fn test_delete() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let workspace = create_test_workspace();
    storage.save(&workspace).unwrap();
    assert!(storage.exists());

    storage.delete().unwrap();
    assert!(!storage.exists());
}

#[test]
fn test_backup_and_restore() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let workspace = create_test_workspace();
    storage.save(&workspace).unwrap();

    // 备份
    let backup_path = storage.backup().unwrap();
    assert!(backup_path.exists());

    // 等待1秒以确保时间戳不同
    std::thread::sleep(std::time::Duration::from_secs(1));

    // 修改原文件
    let mut modified = workspace.clone();
    modified.name = "modified".to_string();
    storage.save(&modified).unwrap();

    let loaded = storage.load().unwrap();
    assert_eq!(loaded.name, "modified");

    // 从备份恢复
    storage.restore_from_backup(&backup_path).unwrap();
    let restored = storage.load().unwrap();
    assert_eq!(restored.name, "test-workspace");
}

#[test]
fn test_validate() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let workspace = create_test_workspace();
    storage.save(&workspace).unwrap();

    assert!(storage.validate().is_ok());
}

#[test]
fn test_validate_duplicate_ids() {
    use fireworks_collaboration_lib::core::workspace::model::RepositoryEntry;
    
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workspace.json");
    let storage = WorkspaceStorage::new(file_path);

    let mut workspace = create_test_workspace();
    
    // 添加两个相同 ID 的仓库（直接操作内部数据）
    workspace.repositories.push(RepositoryEntry::new(
        "repo1".to_string(),
        "Repo 1".to_string(),
        PathBuf::from("repo1"),
        "https://github.com/test/repo1".to_string(),
    ));
    workspace.repositories.push(RepositoryEntry::new(
        "repo1".to_string(), // 重复的 ID
        "Repo 2".to_string(),
        PathBuf::from("repo2"),
        "https://github.com/test/repo2".to_string(),
    ));

    storage.save(&workspace).unwrap();

    // 验证应该失败
    assert!(storage.validate().is_err());
}

#[test]
fn test_from_app_data_dir() {
    let temp_dir = TempDir::new().unwrap();
    let storage = WorkspaceStorage::from_app_data_dir(temp_dir.path());
    
    assert_eq!(storage.file_path(), temp_dir.path().join("workspace.json"));
}
