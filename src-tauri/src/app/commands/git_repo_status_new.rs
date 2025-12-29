/// Get the status of a Git repository.
///
/// # Parameters
/// - `dest`: Repository path
#[tauri::command(rename_all = "camelCase")]
pub async fn git_repo_status(dest: String) -> Result<RepoStatus, String> {
    use git2::Repository;

    let path = Path::new(&dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let repo = Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    // Get current branch
    let head = repo.head().ok();
    let current_branch = if let Some(ref h) = head {
        if h.is_branch() {
            h.shorthand().map(|s| s.to_string())
        } else {
            None // Detached HEAD
        }
    } else {
        None
    };

    let is_detached = current_branch.is_none();

    // Get file status counts using git2
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;

    let statuses = repo
        .statuses(None)
        .map_err(|e| format!("Failed to get status: {}", e))?;

    for entry in statuses.iter() {
        let status = entry.status();

        if status.contains(git2::Status::WT_NEW) {
            untracked += 1;
        } else {
            if status.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            ) {
                staged += 1;
            }
            if status.intersects(
                git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_TYPECHANGE
                    | git2::Status::WT_RENAMED,
            ) {
                unstaged += 1;
            }
        }
    }

    let is_clean = staged == 0 && unstaged == 0 && untracked == 0;

    // Get ahead/behind counts
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut tracking_branch = None;

    if let Some(ref branch_name) = current_branch {
        if let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
            if let Ok(upstream) = branch.upstream() {
                tracking_branch = upstream.name().ok().flatten().map(|s| s.to_string());

                // Get ahead/behind counts
                if let (Ok(local_oid), Ok(upstream_oid)) =
                    (branch.get().target(), upstream.get().target())
                {
                    if let Ok((a, b)) = repo.graph_ahead_behind(local_oid, upstream_oid) {
                        ahead = a as u32;
                        behind = b as u32;
                    }
                }
            }
        }
    }

    // Get branches using existing helper
    let branches = list_branches_internal(&dest, false).unwrap_or_default();

    Ok(RepoStatus {
        current_branch,
        is_detached,
        is_clean,
        staged,
        unstaged,
        untracked,
        ahead,
        behind,
        branches,
        tracking_branch,
    })
}
