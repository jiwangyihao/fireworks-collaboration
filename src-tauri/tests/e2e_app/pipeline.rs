use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::git::*;
use fireworks_collaboration_lib::app::types::{
    SharedConfig, SharedCredentialFactory, TaskRegistryState,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;

use fireworks_collaboration_lib::core::git::runner::{Git2Runner, GitRunner};
use fireworks_collaboration_lib::core::tasks::TaskRegistry;
use std::path::PathBuf;
use std::process::Command;

pub fn create_bare_remote_with_commits(n: usize) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("fwc-remote-src-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("mkdir tmp");
    run(Command::new("git").arg("init").arg(&tmp));

    // config user for commit
    run(Command::new("git")
        .current_dir(&tmp)
        .args(&["config", "user.name", "Test Setup"]));
    run(Command::new("git")
        .current_dir(&tmp)
        .args(&["config", "user.email", "setup@test.com"]));

    for i in 1..=n.max(1) {
        let f = tmp.join(format!("f{i}.txt"));
        std::fs::write(&f, format!("c{i}\n")).unwrap();
        run(Command::new("git").current_dir(&tmp).args(&["add", "."]));
        run(Command::new("git")
            .current_dir(&tmp)
            .args(&["commit", "-m", &format!("c{i}")]));
    }
    // 裸仓库
    let bare = std::env::temp_dir().join(format!("fwc-remote-bare-{}", uuid::Uuid::new_v4()));
    run(Command::new("git").args(&[
        "clone",
        "--bare",
        tmp.to_string_lossy().as_ref(),
        bare.to_string_lossy().as_ref(),
    ]));
    bare
}

fn run(cmd: &mut Command) {
    let status = cmd.status().expect("run command");
    assert!(status.success(), "command failed: {cmd:?}");
}

// Mock Assets for Tauri
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

pub fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, TaskRegistryState) {
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));
    let credential_factory: SharedCredentialFactory = Arc::new(Mutex::new(None));
    let runner = Box::new(Git2Runner::new()) as Box<dyn GitRunner + Send + Sync>;

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .manage::<SharedConfig>(config)
        .manage::<SharedCredentialFactory>(credential_factory)
        .manage::<Box<dyn GitRunner + Send + Sync>>(runner)
        .build(context)
        .expect("Failed to build mock app");

    (app, registry)
}

pub struct AppPipeline {
    app: tauri::App<tauri::test::MockRuntime>,
    #[allow(dead_code)]
    registry: TaskRegistryState,
}

impl AppPipeline {
    pub fn new() -> Self {
        let (app, registry) = create_mock_app();
        Self { app, registry }
    }

    pub async fn clone_repo(&self, repo_url: &str, dest: &str) -> Result<String, String> {
        git_clone(
            repo_url.to_string(),
            dest.to_string(),
            None::<serde_json::Value>,
            None::<String>,
            None::<serde_json::Value>,
            None::<bool>,
            self.app.state(),
            self.app.handle().clone(),
        )
        .await
    }

    pub async fn push(&self, repo_path: &str) -> Result<String, String> {
        git_push(
            repo_path.to_string(),
            None::<String>,
            None::<Vec<String>>,
            None::<String>,
            None::<String>,
            None::<bool>,
            None::<serde_json::Value>,
            self.app.state(),
            self.app.state(),
            self.app.handle().clone(),
        )
        .await
    }

    pub async fn commit(&self, repo_path: &str, msg: &str) -> Result<String, String> {
        // git_commit command might not exist in app commands yet, check if we need to use Git2Runner directly or file system.
        // For now, we simulate commit using git2 directly since we are testing commands invocation primarily for clone/push.
        use git2::{Repository, Signature};
        let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

        let mut index = repo.index().map_err(|e| e.to_string())?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;

        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let signature = Signature::now("test", "test@example.com").map_err(|e| e.to_string())?;
        let parent_commit = match repo.head() {
            Ok(head) => {
                let target = head.target().unwrap();
                Some(repo.find_commit(target).map_err(|e| e.to_string())?)
            }
            Err(_) => None,
        };

        let parents = match &parent_commit {
            Some(c) => vec![c],
            None => vec![],
        };

        let oid = repo
            .commit(Some("HEAD"), &signature, &signature, msg, &tree, &parents)
            .map_err(|e| e.to_string())?;

        Ok(oid.to_string())
    }

    pub async fn wait_for_task(&self, task_id: &str) {
        use std::str::FromStr;
        let uuid = uuid::Uuid::from_str(task_id).expect("task id is uuid");
        crate::common::task_wait::wait_until_task_done(&self.registry, uuid).await;
    }
}
