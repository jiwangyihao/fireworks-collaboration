#![cfg(not(feature = "tauri-app"))]
//! 递归克隆子模块集成测试
//!
//! 测试 P7.1 `TaskKind::GitClone` 的 `recurse_submodules` 参数

use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use std::sync::Arc;
use tempfile::TempDir;

use crate::common::task_wait::wait_until_task_done;
use crate::common::test_env;

/// 测试基础克隆不启用递归子模块(默认行为)
#[tokio::test(flavor = "current_thread")]
async fn test_clone_without_recurse_submodules() {
    test_env::init_test_env();

    let registry = Arc::new(TaskRegistry::new());
    let temp_dest = TempDir::new().unwrap();

    // 使用一个简单的远程仓库(无子模块)
    let origin = crate::common::repo_factory::RepoBuilder::new()
        .with_base_commit("readme.txt", "test content", "Initial commit")
        .build();

    // 创建任务,不启用递归子模块
    let (id, token) = registry.create(TaskKind::GitClone {
        repo: origin.path.to_str().unwrap().to_string(),
        dest: temp_dest.path().to_str().unwrap().to_string(),
        depth: None,
        filter: None,
        strategy_override: None,
        recurse_submodules: false,
    });

    let handle = registry.clone().spawn_git_clone_task_with_opts(
        None,
        id,
        token,
        origin.path.to_str().unwrap().to_string(),
        temp_dest.path().to_str().unwrap().to_string(),
        None,
        None,
        None,
        false, // recurse_submodules = false
        None,
    );

    // 等待任务完成
    wait_until_task_done(&registry, id).await;
    handle.await.unwrap();

    // 验证任务完成
    let snapshot = registry.snapshot(&id).unwrap();
    assert_eq!(snapshot.state, TaskState::Completed);

    // 验证仓库存在
    assert!(temp_dest.path().join(".git").exists());
}

/// 测试克隆时启用递归子模块参数传递
#[tokio::test(flavor = "current_thread")]
async fn test_clone_with_recurse_submodules_parameter() {
    test_env::init_test_env();

    let registry = Arc::new(TaskRegistry::new());
    let temp_dest = TempDir::new().unwrap();

    // 使用一个简单的仓库测试参数传递
    let origin = crate::common::repo_factory::RepoBuilder::new()
        .with_base_commit("file.txt", "content", "Commit")
        .build();

    // 创建任务,启用递归子模块
    let (id, token) = registry.create(TaskKind::GitClone {
        repo: origin.path.to_str().unwrap().to_string(),
        dest: temp_dest.path().to_str().unwrap().to_string(),
        depth: None,
        filter: None,
        strategy_override: None,
        recurse_submodules: true, // 启用递归
    });

    let handle = registry.clone().spawn_git_clone_task_with_opts(
        None,
        id,
        token,
        origin.path.to_str().unwrap().to_string(),
        temp_dest.path().to_str().unwrap().to_string(),
        None,
        None,
        None,
        true, // recurse_submodules = true
        None,
    );

    // 等待任务完成
    wait_until_task_done(&registry, id).await;
    handle.await.unwrap();

    // 验证任务完成(即使没有子模块也应该正常完成)
    let snapshot = registry.snapshot(&id).unwrap();
    assert_eq!(snapshot.state, TaskState::Completed);
}

/// 测试 `TaskKind::GitClone` 的序列化包含 `recurse_submodules` 字段
#[test]
fn test_git_clone_task_kind_serde_with_recurse_submodules() {
    // 测试序列化
    let task = TaskKind::GitClone {
        repo: "https://github.com/test/repo.git".to_string(),
        dest: "/path/to/dest".to_string(),
        depth: Some(1),
        filter: None,
        strategy_override: None,
        recurse_submodules: true,
    };

    let json = serde_json::to_string(&task).unwrap();
    eprintln!("Serialized JSON: {json}"); // Debug output
    assert!(
        json.contains("recurseSubmodules") || json.contains("recurse_submodules"),
        "JSON should contain recurseSubmodules field: {json}"
    );
    assert!(json.contains("true"));

    // 测试反序列化
    let deserialized: TaskKind = serde_json::from_str(&json).unwrap();
    if let TaskKind::GitClone {
        recurse_submodules, ..
    } = deserialized
    {
        assert!(recurse_submodules);
    } else {
        panic!("Expected GitClone task kind");
    }
}

/// 测试 `TaskKind::GitClone` 默认值向后兼容(缺少字段时默认为 false)
#[test]
fn test_git_clone_backward_compatible_default() {
    // 旧版 JSON 格式(不含 recurseSubmodules 字段)
    let old_json = r#"{
        "kind": "gitClone",
        "repo": "https://github.com/test/repo.git",
        "dest": "/path/to/dest",
        "depth": null,
        "filter": null,
        "strategyOverride": null
    }"#;

    // 应该能够反序列化,且 recurse_submodules 默认为 false
    let deserialized: TaskKind = serde_json::from_str(old_json).unwrap();
    if let TaskKind::GitClone {
        recurse_submodules, ..
    } = deserialized
    {
        assert!(!recurse_submodules, "默认值应该是 false");
    } else {
        panic!("Expected GitClone task kind");
    }
}
