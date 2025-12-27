use fireworks_collaboration_lib::core::workspace::status::{
    RepositoryStatus, SyncState, WorkingTreeState,
};
use fireworks_collaboration_lib::core::workspace::status_utils::{
    build_summary, derive_sync_state, resolve_repository_path, working_state_rank,
};
use std::path::{Path, PathBuf};

#[test]
fn test_derive_sync_state() {
    assert_eq!(
        derive_sync_state(&SyncState::Clean, 0, 0, true),
        SyncState::Clean
    );
    assert_eq!(
        derive_sync_state(&SyncState::Clean, 0, 0, false),
        SyncState::Unknown
    );
    assert_eq!(
        derive_sync_state(&SyncState::Detached, 0, 0, false),
        SyncState::Detached
    );

    // Ahead
    assert_eq!(
        derive_sync_state(&SyncState::Clean, 1, 0, true),
        SyncState::Ahead
    );

    // Behind
    assert_eq!(
        derive_sync_state(&SyncState::Clean, 0, 1, true),
        SyncState::Behind
    );

    // Diverged
    assert_eq!(
        derive_sync_state(&SyncState::Clean, 1, 1, true),
        SyncState::Diverged
    );

    // Detached precedence
    assert_eq!(
        derive_sync_state(&SyncState::Detached, 0, 0, true),
        SyncState::Detached
    );
}

#[test]
fn test_resolve_repository_path() {
    let root = Path::new("/workspace");

    let abs = PathBuf::from("/absolute/path");
    assert_eq!(resolve_repository_path(root, &abs), abs);

    let rel = Path::new("relative/repo");
    assert_eq!(
        resolve_repository_path(root, rel),
        root.join("relative/repo")
    );
}

#[test]
fn test_working_state_rank() {
    assert_eq!(working_state_rank(&WorkingTreeState::Clean), 0);
    assert_eq!(working_state_rank(&WorkingTreeState::Dirty), 1);
    assert_eq!(working_state_rank(&WorkingTreeState::Missing), 2);
    assert_eq!(working_state_rank(&WorkingTreeState::Error), 3);
}

#[test]
fn test_build_summary() {
    let status1 = create_repo_status("1", WorkingTreeState::Clean, SyncState::Clean, None);
    let status2 = create_repo_status("2", WorkingTreeState::Dirty, SyncState::Ahead, None);
    let status3 = create_repo_status(
        "3",
        WorkingTreeState::Error,
        SyncState::Unknown,
        Some("err"),
    );

    let summary = build_summary(&[status1, status2, status3]);

    assert_eq!(summary.working_states.clean, 1);
    assert_eq!(summary.working_states.dirty, 1);
    assert_eq!(summary.working_states.error, 1);

    assert_eq!(summary.sync_states.clean, 1);
    assert_eq!(summary.sync_states.ahead, 1);
    assert_eq!(summary.sync_states.unknown, 1);

    assert_eq!(summary.error_count, 1);
    assert_eq!(summary.error_repositories, vec!["3"]);
}

fn create_repo_status(
    id: &str,
    working: WorkingTreeState,
    sync: SyncState,
    error: Option<&str>,
) -> RepositoryStatus {
    RepositoryStatus {
        repo_id: id.into(),
        name: format!("Repo {}", id),
        enabled: true,
        path: "path".into(),
        tags: vec![],
        remote_url: None,
        current_branch: None,
        upstream_branch: None,
        ahead: 0,
        behind: 0,
        sync_state: sync,
        working_state: working,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        conflicts: 0,
        last_commit_id: None,
        last_commit_message: None,
        last_commit_time: None,
        last_commit_unix_time: None,
        status_timestamp: "".into(),
        status_unix_time: 0,
        is_cached: false,
        error: error.map(|s| s.into()),
    }
}
