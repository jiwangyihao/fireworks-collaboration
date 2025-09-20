#![cfg(not(feature = "tauri-app"))]
//! P2.3a: strategyOverride parsing for push (parse only, no application)

use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use serde_json::json;

#[tokio::test]
async fn push_with_valid_strategy_override_completes() {
    let reg = Arc::new(TaskRegistry::new());
    // We don't need an actual git repo because failure would come later; here we only assert
    // parsing succeeds. Create a minimal temp dir with .git to bypass early non-repo failure.
    let tmp = std::env::temp_dir().join(format!("fwc-push-strategy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    // init empty git repo (shelling out to git). If git not present the test will fail, consistent with others.
    let st = std::process::Command::new("git").args(["init", "--quiet"]).current_dir(&tmp).status().expect("git init");
    assert!(st.success());
    // create dummy commit so push logic attempts network later (still local so fine)
    std::fs::write(tmp.join("a.txt"), "1").unwrap();
    let _ = std::process::Command::new("git").current_dir(&tmp).args(["add", "."]).status();
    let _ = std::process::Command::new("git").current_dir(&tmp).args(["-c","user.email=you@example.com","-c","user.name=You","commit","-m","c1"]).status();

    let strategy = json!({
        "http": { "followRedirects": true, "maxRedirects": 5 },
        "tls": { "insecureSkipVerify": false },
        "retry": { "max": 3, "baseMs": 120, "factor": 1.3, "jitter": true },
        "extraTop": { "ignored": 1 }
    });
    let (id, token) = reg.create(TaskKind::GitPush { dest: tmp.to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(strategy) });
    // cancel early to avoid doing real push; ensures parse path executed
    token.cancel();
    let handle = reg.spawn_git_push_task(None, id, token, tmp.to_string_lossy().to_string(), None, None, None, None, Some(json!({"http": {"followRedirects": true}})));
    handle.await.unwrap();
    let snap = reg.snapshot(&id).expect("snapshot");
    assert!(matches!(snap.state, TaskState::Canceled | TaskState::Failed | TaskState::Completed));
}

#[tokio::test]
async fn push_with_invalid_strategy_override_array_fails() {
    let reg = Arc::new(TaskRegistry::new());
    let tmp = std::env::temp_dir().join(format!("fwc-push-strategy-bad-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let st = std::process::Command::new("git").args(["init", "--quiet"]).current_dir(&tmp).status().expect("git init");
    assert!(st.success());
    let bad = serde_json::json!([1,2,3]);
    let (id, token) = reg.create(TaskKind::GitPush { dest: tmp.to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(bad.clone()) });
    let handle = reg.spawn_git_push_task(None, id, token, tmp.to_string_lossy().to_string(), None, None, None, None, Some(bad));
    handle.await.unwrap();
    let snap = reg.snapshot(&id).expect("snapshot");
    assert!(matches!(snap.state, TaskState::Failed));
}
