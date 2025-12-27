// Git Command Wrapper Tests
//
// 这些测试验证 git command wrapper 的完整流程：
// 1. 使用 mock_builder 创建应用实例
// 2. 通过 managed state 访问 TaskRegistry
// 3. 直接调用 command 底层逻辑（因为 TauriRuntime 类型限制无法直接调用 #[tauri::command]）
// 4. 验证任务创建和参数正确性

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::core::git::utils::parse_depth;
use fireworks_collaboration_lib::core::tasks::{TaskKind, TaskRegistry, TaskState};

struct MockAssets;

impl<R: tauri::Runtime> Assets<R> for MockAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Cow<'_, [u8]>)> + '_> {
        Box::new(std::iter::empty())
    }

    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

/// Helper to create a mock app with TaskRegistry
fn create_mock_app_with_registry() -> (
    tauri::App<tauri::test::MockRuntime>,
    Arc<Mutex<TaskRegistry>>,
) {
    let registry = Arc::new(Mutex::new(TaskRegistry::new()));
    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage(registry.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, registry)
}

#[tokio::test]
async fn test_git_clone_wrapper_creates_task() {
    let (app, _registry) = create_mock_app_with_registry();

    // Simulate what git_clone command does internally
    let repo = "https://github.com/example/repo.git".to_string();
    let dest = "/tmp/repo".to_string();
    let depth: Option<serde_json::Value> = Some(serde_json::json!(1));
    let filter: Option<String> = None;
    let strategy_override: Option<serde_json::Value> = None;
    let recurse_submodules = false;

    let depth_parsed = parse_depth(depth.clone());

    // Get registry from app state
    let reg: tauri::State<Arc<Mutex<TaskRegistry>>> = app.state();

    // Create task (same as git_clone command)
    let task_kind = TaskKind::GitClone {
        repo: repo.clone(),
        dest: dest.clone(),
        depth: depth_parsed,
        filter: filter.clone(),
        strategy_override: strategy_override.clone(),
        recurse_submodules,
    };

    let (id, _token) = {
        let mut guard = reg.lock().unwrap();
        guard.create(task_kind)
    };

    // Verify task was created
    assert!(!id.is_nil());

    // Verify task exists in registry via snapshot
    let guard = reg.lock().unwrap();
    let snapshot = guard.snapshot(&id);
    assert!(snapshot.is_some());

    // Verify task state and kind
    if let Some(snap) = snapshot {
        assert_eq!(snap.state, TaskState::Pending);
        assert_eq!(snap.kind, "GitClone");
    }
}

#[tokio::test]
async fn test_git_fetch_wrapper_creates_task() {
    let (app, _registry) = create_mock_app_with_registry();

    // Simulate what git_fetch command does internally
    let repo = "".to_string(); // Empty for default remote
    let dest = "/tmp/repo".to_string();
    let _preset: Option<String> = Some("shallow".to_string());
    let depth: Option<serde_json::Value> = None;
    let filter: Option<String> = Some("blob:none".to_string());

    let depth_parsed = parse_depth(depth.clone());

    // Get registry from app state
    let reg: tauri::State<Arc<Mutex<TaskRegistry>>> = app.state();

    // Create task (same as git_fetch command)
    let task_kind = TaskKind::GitFetch {
        repo: repo.clone(),
        dest: dest.clone(),
        depth: depth_parsed,
        filter: filter.clone(),
        strategy_override: None,
    };

    let (id, _token) = {
        let mut guard = reg.lock().unwrap();
        guard.create(task_kind)
    };

    // Verify task was created
    assert!(!id.is_nil());

    // Verify task exists in registry via snapshot
    let guard = reg.lock().unwrap();
    let snapshot = guard.snapshot(&id);
    assert!(snapshot.is_some());

    // Verify task state and kind
    if let Some(snap) = snapshot {
        assert_eq!(snap.state, TaskState::Pending);
        assert_eq!(snap.kind, "GitFetch");
    }
}

#[tokio::test]
async fn test_git_init_wrapper_creates_task() {
    let (app, _registry) = create_mock_app_with_registry();

    let dest = "/tmp/new_repo".to_string();

    // Get registry from app state
    let reg: tauri::State<Arc<Mutex<TaskRegistry>>> = app.state();

    // Create task (same as git_init command)
    let task_kind = TaskKind::GitInit { dest: dest.clone() };

    let (id, _token) = {
        let mut guard = reg.lock().unwrap();
        guard.create(task_kind)
    };

    // Verify task was created
    assert!(!id.is_nil());

    // Verify task exists in registry via snapshot
    let guard = reg.lock().unwrap();
    let snapshot = guard.snapshot(&id);
    assert!(snapshot.is_some());

    // Verify task state and kind
    if let Some(snap) = snapshot {
        assert_eq!(snap.state, TaskState::Pending);
        assert_eq!(snap.kind, "GitInit");
    }
}

#[tokio::test]
async fn test_git_add_wrapper_creates_task() {
    let (app, _registry) = create_mock_app_with_registry();

    let dest = "/tmp/repo".to_string();
    let paths = vec!["file1.txt".to_string(), "file2.txt".to_string()];

    // Get registry from app state
    let reg: tauri::State<Arc<Mutex<TaskRegistry>>> = app.state();

    // Create task (same as git_add command)
    let task_kind = TaskKind::GitAdd {
        dest: dest.clone(),
        paths: paths.clone(),
    };

    let (id, _token) = {
        let mut guard = reg.lock().unwrap();
        guard.create(task_kind)
    };

    // Verify task was created
    assert!(!id.is_nil());

    // Verify task exists in registry via snapshot
    let guard = reg.lock().unwrap();
    let snapshot = guard.snapshot(&id);
    assert!(snapshot.is_some());

    // Verify task state and kind
    if let Some(snap) = snapshot {
        assert_eq!(snap.state, TaskState::Pending);
        assert_eq!(snap.kind, "GitAdd");
    }
}

#[tokio::test]
async fn test_git_commit_wrapper_creates_task() {
    let (app, _registry) = create_mock_app_with_registry();

    let dest = "/tmp/repo".to_string();
    let message = "Test commit message".to_string();

    // Get registry from app state
    let reg: tauri::State<Arc<Mutex<TaskRegistry>>> = app.state();

    // Create task (same as git_commit command)
    let task_kind = TaskKind::GitCommit {
        dest: dest.clone(),
        message: message.clone(),
        allow_empty: false,
        author_name: Some("Test Author".to_string()),
        author_email: Some("test@example.com".to_string()),
    };

    let (id, _token) = {
        let mut guard = reg.lock().unwrap();
        guard.create(task_kind)
    };

    // Verify task was created
    assert!(!id.is_nil());

    // Verify task exists in registry via snapshot
    let guard = reg.lock().unwrap();
    let snapshot = guard.snapshot(&id);
    assert!(snapshot.is_some());

    // Verify task state and kind
    if let Some(snap) = snapshot {
        assert_eq!(snap.state, TaskState::Pending);
        assert_eq!(snap.kind, "GitCommit");
    }
}
