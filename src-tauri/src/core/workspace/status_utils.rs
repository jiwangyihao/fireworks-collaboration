use super::status::{RepositoryStatus, SyncState, WorkingTreeState, WorkspaceStatusSummary};
use std::path::{Path, PathBuf};

/// Determine sync state logic
pub fn derive_sync_state(
    current: &SyncState,
    ahead: u32,
    behind: u32,
    has_branch: bool,
) -> SyncState {
    if !has_branch {
        return match current {
            SyncState::Detached => SyncState::Detached,
            SyncState::Unknown => SyncState::Unknown,
            _ => SyncState::Unknown,
        };
    }
    if ahead > 0 && behind > 0 {
        SyncState::Diverged
    } else if ahead > 0 {
        SyncState::Ahead
    } else if behind > 0 {
        SyncState::Behind
    } else if matches!(current, SyncState::Detached) {
        SyncState::Detached
    } else {
        SyncState::Clean
    }
}

pub fn resolve_repository_path(root: &Path, repo_path: &Path) -> PathBuf {
    if repo_path.is_absolute() {
        repo_path.to_path_buf()
    } else {
        root.join(repo_path)
    }
}

pub fn append_error(status: &mut RepositoryStatus, msg: impl Into<String>) {
    let msg = msg.into();
    if let Some(existing) = &mut status.error {
        existing.push_str("; ");
        existing.push_str(&msg);
    } else {
        status.error = Some(msg);
    }
}

pub fn case_insensitive_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

pub fn compare_optional_strings(a: Option<&String>, b: Option<&String>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a_val), Some(b_val)) => case_insensitive_cmp(a_val, b_val),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

pub fn working_state_rank(state: &WorkingTreeState) -> u8 {
    match state {
        WorkingTreeState::Clean => 0,
        WorkingTreeState::Dirty => 1,
        WorkingTreeState::Missing => 2,
        WorkingTreeState::Error => 3,
    }
}

pub fn build_summary(statuses: &[RepositoryStatus]) -> WorkspaceStatusSummary {
    let mut summary = WorkspaceStatusSummary::default();

    for status in statuses {
        match status.working_state {
            WorkingTreeState::Clean => summary.working_states.clean += 1,
            WorkingTreeState::Dirty => summary.working_states.dirty += 1,
            WorkingTreeState::Missing => summary.working_states.missing += 1,
            WorkingTreeState::Error => summary.working_states.error += 1,
        }

        match status.sync_state {
            SyncState::Clean => summary.sync_states.clean += 1,
            SyncState::Ahead => summary.sync_states.ahead += 1,
            SyncState::Behind => summary.sync_states.behind += 1,
            SyncState::Diverged => summary.sync_states.diverged += 1,
            SyncState::Detached => summary.sync_states.detached += 1,
            SyncState::Unknown => summary.sync_states.unknown += 1,
        }

        if status.error.is_some() {
            summary.error_repositories.push(status.repo_id.clone());
        }
    }

    // Explicitly set error_count based on accumulated errors
    summary.error_count = summary.error_repositories.len();

    summary
}
