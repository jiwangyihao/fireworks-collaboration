use super::pipeline::{create_bare_remote_with_commits, AppPipeline};
use crate::common::test_env;
use std::fs;

#[tokio::test]
async fn clone_modify_push_via_app_commands() {
    test_env::init_test_env();

    // 1. Prepare Remote
    let remote_path = create_bare_remote_with_commits(1);
    let remote_url = remote_path.to_string_lossy().replace('\\', "/"); // standard git url format

    // 2. Setup App Pipeline
    let pipeline = AppPipeline::new();

    // 3. Clone
    let temp_dir = tempfile::tempdir().unwrap();
    let dest_path = temp_dir.path().join("my-repo");
    let dest_str = dest_path.to_string_lossy().to_string();

    let task_id = pipeline
        .clone_repo(&remote_url, &dest_str)
        .await
        .expect("clone command success");
    println!("Clone task id: {}", task_id);

    pipeline.wait_for_task(&task_id).await;

    // Validate Clone
    assert!(dest_path.join(".git").exists());
    assert!(dest_path.join("f1.txt").exists());

    // 4. Modify
    let new_file = dest_path.join("new_feature.txt");
    fs::write(&new_file, "amazing feature").expect("write file");

    // 5. Commit (Simulated via git2 as we focus on Command flow for Clone/Push)
    // In a full E2E, we would call `git_commit_command` if exposed.
    let commit_oid = pipeline
        .commit(&dest_str, "Add new feature")
        .await
        .expect("commit success");
    println!("Created commit: {}", commit_oid);

    // 6. Push
    let push_task = pipeline
        .push(&dest_str)
        .await
        .expect("push command success");
    println!("Push task id: {}", push_task);
    pipeline.wait_for_task(&push_task).await;

    // 7. Verify Remote State (using git CLI or git2 to inspect bare remote)
    let remote_repo = git2::Repository::open(&remote_path).expect("open remote");
    let head = remote_repo.head().expect("remote head");
    let head_commit = head.peel_to_commit().expect("head commit");

    assert_eq!(head_commit.message().unwrap(), "Add new feature");
    assert_eq!(head_commit.id().to_string(), commit_oid);
}
