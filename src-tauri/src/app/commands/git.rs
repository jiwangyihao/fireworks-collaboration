//! Git operation commands.

use tauri::State;

use crate::core::git::runner::GitRunner;
use crate::core::git::utils::{parse_depth, resolve_push_credentials};
use crate::core::tasks::TaskKind;

// Command functions use raw tauri::AppHandle for CommandArg trait compatibility,
// then convert to wrapper for spawn calls
use super::super::types::{AppHandle, SharedCredentialFactory, TaskRegistryState, TauriRuntime};

/// Clone a Git repository.
///
/// # Parameters
/// - `repo`: Repository URL
/// - `dest`: Destination path
/// - `depth`: Optional shallow clone depth
/// - `filter`: Optional object filter (e.g., "blob:none")
/// - `strategy_override`: Optional strategy configuration override
/// - `recurse_submodules`: Whether to recursively clone submodules (P7.1)
#[tauri::command(rename_all = "camelCase")]
pub async fn git_clone(
    repo: String,
    dest: String,
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
    recurse_submodules: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let depth_parsed = parse_depth(depth.clone());
    let recurse = recurse_submodules.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitClone {
        repo: repo.clone(),
        dest: dest.clone(),
        depth: depth_parsed,
        filter: filter.clone(),
        strategy_override: strategy_override.clone(),
        recurse_submodules: recurse,
    });

    reg.clone().spawn_git_clone_task_with_opts(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        repo,
        dest,
        depth,
        filter,
        strategy_override,
        recurse,
        None,
    );

    Ok(id.to_string())
}

/// Fetch updates from a Git repository.
///
/// # Parameters
/// - `repo`: Repository URL (empty for default remote)
/// - `dest`: Local repository path
/// - `preset`: Optional preset name
/// - `depth`: Optional fetch depth
/// - `filter`: Optional object filter
/// - `strategy_override`: Optional strategy configuration override
#[tauri::command(rename_all = "camelCase")]
pub async fn git_fetch(
    repo: String,
    dest: String,
    preset: Option<String>,
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let depth_parsed = parse_depth(depth.clone());

    let (id, token) = reg.create(TaskKind::GitFetch {
        repo: repo.clone(),
        dest: dest.clone(),
        depth: depth_parsed,
        filter: filter.clone(),
        strategy_override: strategy_override.clone(),
    });

    reg.clone().spawn_git_fetch_task_with_opts(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        repo,
        dest,
        preset,
        depth,
        filter,
        strategy_override,
        None,
    );

    Ok(id.to_string())
}

/// Push changes to a remote Git repository.
///
/// # Parameters
/// - `dest`: Local repository path
/// - `remote`: Remote name (defaults to "origin")
/// - `refspecs`: Optional list of refspecs to push
/// - `username`: Optional username for authentication
/// - `password`: Optional password/token for authentication
/// - `use_stored_credential`: Whether to attempt to use stored credentials
/// - `strategy_override`: Optional strategy configuration override
#[tauri::command(rename_all = "camelCase")]
pub async fn git_push(
    dest: String,
    remote: Option<String>,
    refspecs: Option<Vec<String>>,
    username: Option<String>,
    password: Option<String>,
    use_stored_credential: Option<bool>,
    strategy_override: Option<serde_json::Value>,
    reg: State<'_, TaskRegistryState>,
    credential_factory: State<'_, SharedCredentialFactory>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    // Determine final username and password
    let use_stored = use_stored_credential.unwrap_or(false);
    let should_fetch_stored = use_stored && username.is_none() && password.is_none();

    let (stored_username, stored_password) = if should_fetch_stored {
        match try_get_git_credentials(&dest, &credential_factory).await {
            Ok(Some((u, p))) => {
                tracing::info!(
                    target = "git",
                    "Using stored credentials for git push: username={}, host_in_repo=?",
                    u
                );
                (Some(u), Some(p))
            }
            Ok(None) => {
                tracing::warn!(target="git", "No stored credentials found for {}, using provided credentials (which are likely empty)", dest);
                (None, None)
            }
            Err(e) => {
                tracing::error!(
                    target = "git",
                    "Failed to retrieve stored credentials: {}, using provided credentials",
                    e
                );
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let (final_username, final_password) = resolve_push_credentials(
        use_stored,
        username,
        password,
        stored_username,
        stored_password,
    );

    let (id, token) = reg.create(TaskKind::GitPush {
        dest: dest.clone(),
        remote: remote.clone(),
        refspecs: refspecs.clone(),
        username: final_username.clone(),
        password: final_password.clone(),
        strategy_override: strategy_override.clone(),
    });

    reg.clone().spawn_git_push_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        remote,
        refspecs,
        final_username,
        final_password,
        strategy_override,
        None,
    );

    Ok(id.to_string())
}

/// Helper function to try to get Git credentials from storage.
async fn try_get_git_credentials(
    repo_path: &str,
    credential_factory: &State<'_, SharedCredentialFactory>,
) -> Result<Option<(String, String)>, String> {
    // Extract host
    let host = extract_git_host(repo_path)?;
    tracing::info!(target = "git", "Extracted host from repo: {}", host);

    // Get credential store
    let factory_guard = credential_factory
        .lock()
        .map_err(|e| format!("Failed to lock credential factory: {}", e))?;

    let store = factory_guard
        .as_ref()
        .ok_or("Credential store not initialized")?;

    // For GitHub, try with x-access-token username (used for token auth)
    // This is the standard username for GitHub personal access tokens
    let usernames_to_try = if host.contains("github.com") {
        vec![Some("x-access-token"), None]
    } else {
        vec![None]
    };

    for username in usernames_to_try {
        tracing::info!(
            target = "git",
            "Looking up credentials for host: {}, username: {:?}",
            host,
            username
        );
        match store.get(&host, username) {
            Ok(Some(cred)) => {
                tracing::info!(
                    target = "git",
                    "Found credential for host: {}, username: {}",
                    host,
                    cred.username
                );
                return Ok(Some((
                    cred.username.to_string(),
                    cred.password_or_token.to_string(),
                )));
            }
            Ok(None) => {
                tracing::info!(
                    target = "git",
                    "No credential found for host: {}, username: {:?}",
                    host,
                    username
                );
                continue;
            }
            Err(e) => {
                tracing::warn!(target = "git", "Error looking up credential: {}", e);
                continue;
            }
        }
    }

    Ok(None)
}

/// Extract Git host from a repository path by reading git config.
///
/// This function attempts to read the remote URL from the Git repository
/// and extract the host part.
pub(crate) fn extract_git_host(repo_path: &str) -> Result<String, String> {
    use std::path::Path;

    let path = Path::new(repo_path);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Use git2 to read the config
    let repo =
        git2::Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let config = repo
        .config()
        .map_err(|e| format!("Failed to get config: {}", e))?;

    let url = config
        .get_string("remote.origin.url")
        .map_err(|_| "Failed to get remote URL".to_string())?;

    // Parse host from URL
    parse_git_host(&url)
}

/// Parse host from various Git URL formats.
pub fn parse_git_host(url: &str) -> Result<String, String> {
    // HTTPS: https://github.com/user/repo.git
    // Also handle custom transport scheme: https+custom://github.com/user/repo.git
    if url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("https+custom://")
    {
        let without_scheme = url
            .trim_start_matches("https+custom://")
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        // Remove any userinfo (username:password@)
        let without_userinfo = if let Some(at_pos) = without_scheme.find('@') {
            &without_scheme[at_pos + 1..]
        } else {
            without_scheme
        };
        let host = without_userinfo
            .split('/')
            .next()
            .ok_or("Invalid HTTPS URL")?;
        // Remove port if present
        let host = host.split(':').next().unwrap_or(host);
        return Ok(host.to_string());
    }

    // SSH: git@github.com:user/repo.git or ssh://git@github.com/user/repo.git
    if url.starts_with("git@") {
        let without_user = url.trim_start_matches("git@");
        let host = without_user.split(':').next().ok_or("Invalid SSH URL")?;
        return Ok(host.to_string());
    }

    if url.starts_with("ssh://") {
        let without_scheme = url.trim_start_matches("ssh://");
        let without_user = without_scheme.trim_start_matches("git@");
        let host = without_user.split('/').next().ok_or("Invalid SSH URL")?;
        return Ok(host.to_string());
    }

    Err(format!("Unsupported Git URL format: {}", url))
}

/// Initialize a new Git repository.
#[tauri::command(rename_all = "camelCase")]
pub async fn git_init(
    dest: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitInit { dest: dest.clone() });
    reg.clone()
        .spawn_git_init_task(Some(AppHandle::from_tauri(app.clone())), id, token, dest);
    Ok(id.to_string())
}

/// Stage files for commit.
///
/// # Parameters
/// - `dest`: Repository path
/// - `paths`: List of file paths to stage (relative to repository root)
#[tauri::command(rename_all = "camelCase")]
pub async fn git_add(
    dest: String,
    paths: Vec<String>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitAdd {
        dest: dest.clone(),
        paths: paths.clone(),
    });

    reg.clone().spawn_git_add_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        paths,
    );

    Ok(id.to_string())
}

/// Create a commit with staged changes.
///
/// # Parameters
/// - `dest`: Repository path
/// - `message`: Commit message
/// - `allow_empty`: Whether to allow empty commits
/// - `author_name`: Optional author name override
/// - `author_email`: Optional author email override
#[tauri::command(rename_all = "camelCase")]
pub async fn git_commit(
    dest: String,
    message: String,
    allow_empty: Option<bool>,
    author_name: Option<String>,
    author_email: Option<String>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let allow_empty_flag = allow_empty.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitCommit {
        dest: dest.clone(),
        message: message.clone(),
        allow_empty: allow_empty_flag,
        author_name: author_name.clone(),
        author_email: author_email.clone(),
    });

    reg.clone().spawn_git_commit_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        message,
        allow_empty_flag,
        author_name,
        author_email,
    );

    Ok(id.to_string())
}

/// Create or update a branch.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Branch name
/// - `checkout`: Whether to immediately checkout the branch
/// - `force`: Whether to force update if branch exists
#[tauri::command(rename_all = "camelCase")]
pub async fn git_branch(
    dest: String,
    name: String,
    checkout: Option<bool>,
    force: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let checkout_flag = checkout.unwrap_or(false);
    let force_flag = force.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitBranch {
        dest: dest.clone(),
        name: name.clone(),
        checkout: checkout_flag,
        force: force_flag,
    });

    reg.clone().spawn_git_branch_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        name,
        checkout_flag,
        force_flag,
    );

    Ok(id.to_string())
}

/// Checkout a branch or commit.
///
/// # Parameters
/// - `dest`: Repository path
/// - `reference`: Branch name or commit reference
/// - `create`: Whether to create the branch if it doesn't exist
#[tauri::command(rename_all = "camelCase")]
pub async fn git_checkout(
    dest: String,
    reference: String,
    create: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let create_flag = create.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitCheckout {
        dest: dest.clone(),
        reference: reference.clone(),
        create: create_flag,
    });

    reg.clone().spawn_git_checkout_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        reference,
        create_flag,
    );

    Ok(id.to_string())
}

/// Create or update a tag.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Tag name
/// - `message`: Optional tag message (for annotated tags)
/// - `annotated`: Whether to create an annotated tag
/// - `force`: Whether to force update if tag exists
#[tauri::command(rename_all = "camelCase")]
pub async fn git_tag(
    dest: String,
    name: String,
    message: Option<String>,
    annotated: Option<bool>,
    force: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let annotated_flag = annotated.unwrap_or(false);
    let force_flag = force.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitTag {
        dest: dest.clone(),
        name: name.clone(),
        message: message.clone(),
        annotated: annotated_flag,
        force: force_flag,
    });

    reg.clone().spawn_git_tag_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        name,
        message,
        annotated_flag,
        force_flag,
    );

    Ok(id.to_string())
}

/// Set the URL of an existing remote.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Remote name
/// - `url`: New remote URL
#[tauri::command(rename_all = "camelCase")]
pub async fn git_remote_set(
    dest: String,
    name: String,
    url: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteSet {
        dest: dest.clone(),
        name: name.clone(),
        url: url.clone(),
    });

    reg.clone().spawn_git_remote_set_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        name,
        url,
    );

    Ok(id.to_string())
}

/// Add a new remote.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Remote name
/// - `url`: Remote URL
#[tauri::command(rename_all = "camelCase")]
pub async fn git_remote_add(
    dest: String,
    name: String,
    url: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteAdd {
        dest: dest.clone(),
        name: name.clone(),
        url: url.clone(),
    });

    reg.clone().spawn_git_remote_add_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        name,
        url,
    );

    Ok(id.to_string())
}

/// Remove a remote.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Remote name to remove
#[tauri::command(rename_all = "camelCase")]
pub async fn git_remote_remove(
    dest: String,
    name: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteRemove {
        dest: dest.clone(),
        name: name.clone(),
    });

    reg.clone().spawn_git_remote_remove_task(
        Some(AppHandle::from_tauri(app.clone())),
        id,
        token,
        dest,
        name,
    );

    Ok(id.to_string())
}

// ============================================================================
// Synchronous query commands (no task creation)
// ============================================================================

use serde::Serialize;
use std::path::Path;
// use std::process::Command; // Replaced by GitRunner

/// Branch information returned by git_list_branches.
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub commit: Option<String>,
}

/// Repository status information.
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub current_branch: Option<String>,
    pub is_detached: bool,
    pub is_clean: bool,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub ahead: u32,
    pub behind: u32,
    pub branches: Vec<BranchInfo>,
    pub tracking_branch: Option<String>,
}

/// List all branches in a Git repository.
///
/// # Parameters
/// - `dest`: Repository path
/// - `include_remote`: Whether to include remote branches
#[tauri::command(rename_all = "camelCase")]
pub async fn git_list_branches(
    dest: String,
    include_remote: Option<bool>,
) -> Result<Vec<BranchInfo>, String> {
    list_branches_internal(&dest, include_remote.unwrap_or(false))
}

pub(crate) fn list_branches_internal(
    dest: &str,
    include_remote: bool,
) -> Result<Vec<BranchInfo>, String> {
    let path = Path::new(dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Use git2 to list branches
    let repo =
        git2::Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let mut branches = Vec::new();

    // Get HEAD to determine current branch
    let head = repo.head().ok();
    let current_branch_name = head
        .as_ref()
        .and_then(|h| h.shorthand())
        .map(|s| s.to_string());

    // Determine branch types to iterate
    let branch_type = if include_remote {
        None // Both local and remote
    } else {
        Some(git2::BranchType::Local)
    };

    // Iterate branches
    for branch in repo
        .branches(branch_type)
        .map_err(|e| format!("Failed to list branches: {}", e))?
    {
        if let Ok((branch, branch_type)) = branch {
            if let Ok(name) = branch.name() {
                if let Some(branch_name) = name {
                    let is_current = current_branch_name
                        .as_ref()
                        .map_or(false, |cb| cb == branch_name);
                    let is_remote = branch_type == git2::BranchType::Remote;

                    // Get upstream info
                    let upstream = if !is_remote {
                        branch
                            .upstream()
                            .ok()
                            .and_then(|u| u.name().ok().flatten().map(|s| s.to_string()))
                    } else {
                        None
                    };

                    // Get commit
                    let commit = branch.get().target().map(|oid| oid.to_string());

                    branches.push(BranchInfo {
                        name: branch_name.to_string(),
                        is_current,
                        is_remote,
                        upstream,
                        commit,
                    });
                }
            }
        }
    }

    Ok(branches)
}

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
                if let (Some(local_oid), Some(upstream_oid)) =
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

    // Get branches using git2
    let mut branches = Vec::new();
    for branch in repo
        .branches(None)
        .map_err(|e| format!("Failed to list branches: {}", e))?
    {
        if let Ok((branch, _)) = branch {
            if let Ok(name) = branch.name() {
                if let Some(n) = name {
                    let is_current = current_branch.as_ref().map_or(false, |cb| cb == n);
                    let upstream = branch
                        .upstream()
                        .ok()
                        .and_then(|u| u.name().ok().flatten().map(|s| s.to_string()));
                    let commit = branch.get().target().map(|oid| oid.to_string());
                    branches.push(BranchInfo {
                        name: n.to_string(),
                        is_current,
                        is_remote: false,
                        upstream,
                        commit,
                    });
                }
            }
        }
    }

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

/// Delete a local branch.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Branch name to delete
/// - `force`: Whether to force delete (even if not merged)
pub async fn git_delete_branch(
    dest: String,
    name: String,
    force: Option<bool>,
) -> Result<(), String> {
    let path = Path::new(&dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Use git2 to delete the branch
    let repo =
        git2::Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    // Find the branch
    let mut branch = repo
        .find_branch(&name, git2::BranchType::Local)
        .map_err(|e| format!("Failed to find branch '{}': {}", name, e))?;

    // Check if we need to force delete
    let is_force = force.unwrap_or(false);

    // Try to delete
    if is_force {
        // Force delete
        branch
            .delete()
            .map_err(|e| format!("Failed to delete branch '{}': {}", name, e))?;
    } else {
        // Check if merged first
        let is_merged = branch.is_head();
        if is_merged {
            return Err(format!("Cannot delete current branch '{}'", name));
        }

        branch.delete().map_err(|e| {
            format!(
                "Failed to delete branch '{}'. Use force=true to delete unmerged branch: {}",
                name, e
            )
        })?;
    }

    Ok(())
}

/// List remote branches in a Git repository.
///
/// # Parameters
/// - `dest`: Repository path
/// - `remote`: Remote name (defaults to "origin")
/// - `fetch_first`: Whether to fetch before listing (defaults to false)
#[tauri::command(rename_all = "camelCase")]
pub async fn git_remote_branches(
    dest: String,
    remote: Option<String>,
    fetch_first: Option<bool>,
) -> Result<Vec<String>, String> {
    let path = std::path::Path::new(&dest);
    if !path.exists() || (!path.join(".git").exists() && !path.join(".git").is_file()) {
        return Err("Not a git repository".to_string());
    }

    let remote_name = remote.as_deref().unwrap_or("origin");

    let repo =
        git2::Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    // Optionally fetch first (skipped for now as it requires credentials)
    if fetch_first.unwrap_or(false) {
        // TODO: Implement fetch with credentials
    }

    // Get remote branches
    let mut branches = Vec::new();
    let filter = Some(git2::BranchType::Remote);

    for branch in repo
        .branches(filter)
        .map_err(|e| format!("Failed to list branches: {}", e))?
    {
        if let Ok((branch, _)) = branch {
            if let Ok(name) = branch.name() {
                if let Some(name) = name {
                    if name.starts_with(remote_name) {
                        branches.push(name.to_string());
                    }
                }
            }
        }
    }

    Ok(branches)
}

// ============================================================================
// Git Worktree commands
// ============================================================================

/// Worktree information returned by git_worktree_list.
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_main: bool,
    pub is_detached: bool,
    pub locked: bool,
    pub prunable: bool,
    pub tracking_branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

/// List all worktrees in a Git repository.
///
/// # Parameters
/// - `dest`: Repository path (main worktree or any linked worktree)
#[tauri::command(rename_all = "camelCase")]
pub async fn git_worktree_list(dest: String) -> Result<Vec<WorktreeInfo>, String> {
    let path = Path::new(&dest);
    if !path.exists() {
        return Err("Path does not exist".to_string());
    }

    let repo =
        git2::Repository::open(path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let mut worktrees = Vec::new();

    // Helper to extract details from a repository instance (for a worktree)
    let get_details = |wt_path: &std::path::Path| -> (
        Option<String>,
        Option<String>,
        bool,
        Option<String>,
        u32,
        u32,
    ) {
        if let Ok(wt_repo) = git2::Repository::open(wt_path) {
            let head = wt_repo.head().ok();
            let branch = head
                .as_ref()
                .and_then(|h| h.shorthand())
                .map(|s| s.to_string());
            let head_oid = head
                .as_ref()
                .and_then(|h| h.target())
                .map(|o| o.to_string());
            let detached = wt_repo.head_detached().unwrap_or(false);

            let mut tracking = None;
            let mut ahead = 0;
            let mut behind = 0;

            if let Some(branch_name) = &branch {
                if let Ok(local_branch) = wt_repo.find_branch(branch_name, git2::BranchType::Local)
                {
                    if let Ok(upstream) = local_branch.upstream() {
                        tracking = upstream.name().ok().flatten().map(|s| s.to_string());
                        if let (Some(local_oid), Some(upstream_oid)) =
                            (local_branch.get().target(), upstream.get().target())
                        {
                            if let Ok((a, b)) = wt_repo.graph_ahead_behind(local_oid, upstream_oid)
                            {
                                ahead = a as u32;
                                behind = b as u32;
                            }
                        }
                    } else {
                        // Fallback: check implicit upstream "origin/branchname"
                        let implicit = format!("origin/{}", branch_name);
                        if let Ok(remote_branch) =
                            wt_repo.find_branch(&implicit, git2::BranchType::Remote)
                        {
                            tracking = Some(implicit);
                            if let (Some(local_oid), Some(remote_oid)) =
                                (local_branch.get().target(), remote_branch.get().target())
                            {
                                if let Ok((a, b)) =
                                    wt_repo.graph_ahead_behind(local_oid, remote_oid)
                                {
                                    ahead = a as u32;
                                    behind = b as u32;
                                }
                            }
                        }
                    }
                }
            }
            (head_oid, branch, detached, tracking, ahead, behind)
        } else {
            (None, None, false, None, 0, 0)
        }
    };

    // 1. Handle Main Worktree
    if !repo.is_bare() {
        let common_dir = repo.path();
        let main_wt_path = common_dir.parent();

        if let Some(main_path) = main_wt_path {
            let (head, branch, detached, tracking, ahead, behind) = get_details(main_path);

            // Main worktree usually isn't locked/prunable in the same way (or we assume defaults)
            worktrees.push(WorktreeInfo {
                path: main_path.to_string_lossy().to_string(),
                head,
                branch,
                is_bare: repo.is_bare(),
                is_main: true,
                is_detached: detached,
                locked: false,
                prunable: false,
                tracking_branch: tracking,
                ahead,
                behind,
            });
        }
    }

    // 2. Handle Linked Worktrees
    if let Ok(wt_names) = repo.worktrees() {
        for name in wt_names.iter() {
            if let Some(name) = name {
                if let Ok(wt) = repo.find_worktree(name) {
                    let wt_path = wt.path();
                    let locked = match wt.is_locked() {
                        Ok(git2::WorktreeLockStatus::Locked(_)) => true,
                        _ => false,
                    };
                    let prunable = wt.is_prunable(None).unwrap_or(false);

                    let (head, branch, detached, tracking, ahead, behind) = get_details(wt_path);

                    worktrees.push(WorktreeInfo {
                        path: wt_path.to_string_lossy().to_string(),
                        head,
                        branch,
                        is_bare: false,
                        is_main: false,
                        is_detached: detached,
                        locked,
                        prunable,
                        tracking_branch: tracking,
                        ahead,
                        behind,
                    });
                }
            }
        }
    }

    Ok(worktrees)
}

/// Add a new worktree.
///
/// # Parameters
/// - `dest`: Main repository path
/// - `path`: Path for the new worktree
/// - `branch`: Branch to checkout (will be created if it doesn't exist)
/// - `create_branch`: Whether to create a new branch
/// - `from_remote`: If provided, create branch from this remote ref (e.g. "origin/feature-x")
#[tauri::command(rename_all = "camelCase")]
pub async fn git_worktree_add(
    dest: String,
    path: String,
    branch: String,
    create_branch: Option<bool>,
    from_remote: Option<String>,
    _credential_factory: State<'_, SharedCredentialFactory>,
) -> Result<(), String> {
    let repo_path = Path::new(&dest);
    if !repo_path.exists() || !repo_path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let wt_path = Path::new(&path);
    if wt_path.exists() {
        return Err(format!("Worktree path already exists: {}", path));
    }

    // If from_remote is provided, we try to fetch first
    if let Some(ref remote_ref) = from_remote {
        if let Some(remote_name) = remote_ref.split('/').next() {
            // Use Git2Runner for fetch
            let runner = crate::core::git::Git2Runner::new();
            let should_interrupt = std::sync::atomic::AtomicBool::new(false);

            // Try to get credentials
            // Note: fetch_repo implementation in default_impl/ops.rs handles AppConfig loading internally?
            // Checking do_fetch logic: it takes config.
            // Git2Runner::fetch_repo takes &config.
            // We'll load config locally.
            tracing::info!(target = "git", "Fetching from {}...", remote_name);
            let _ = runner.fetch_repo(repo_path, remote_name, None, &should_interrupt, &mut |_| {});
        }
    }

    let repo = git2::Repository::open(repo_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;

    // Determine target commitment/reference
    // If from_remote is set, we use that ref.
    // If create_branch is true, we create a new branch pointing to that ref (or HEAD).
    // If create_branch is false, we try to checkout the branch 'branch'.

    let should_create = from_remote.is_some() || create_branch.unwrap_or(true);

    if should_create {
        // Resolve target commit
        let target_commit = if let Some(ref remote_ref) = from_remote {
            // Resolve remote ref
            let obj = repo
                .revparse_single(remote_ref)
                .map_err(|e| format!("Failed to resolve remote ref {}: {}", remote_ref, e))?;
            obj.peel_to_commit()
                .map_err(|e| format!("target is not a commit: {}", e))?
        } else {
            // Use HEAD
            let head = repo
                .head()
                .map_err(|e| format!("Failed to get HEAD: {}", e))?;
            head.peel_to_commit()
                .map_err(|e| format!("HEAD is not a commit: {}", e))?
        };

        // Create the branch
        // Note: git worktree add -b creates the branch. git2::Repository::worktree does NOT create the branch automatically?
        // libgit2 worktree_add just creates the worktree and checks it out.
        // It expects the reference to exist if we pass it, or we create it.
        // If we want to create a NEW branch "branch" pointing to "target_commit", we do it now.

        let _ = repo
            .branch(&branch, &target_commit, false)
            .map_err(|e| format!("Failed to create branch {}: {}", branch, e))?;
    }

    // Verify reference exists or we just created it
    let reference = repo
        .find_branch(&branch, git2::BranchType::Local)
        .map_err(|e| format!("Branch {} not found: {}", branch, e))?
        .into_reference();

    let mut opts = git2::WorktreeAddOptions::new();
    opts.reference(Some(&reference));

    let _wt = repo
        .worktree(&branch, wt_path, Some(&opts))
        .map_err(|e| format!("Failed to add worktree: {}", e))?;

    tracing::info!(
        target = "git",
        "Added worktree at {} on branch {}",
        path,
        branch
    );

    Ok(())
}

/// Remove a worktree.
///
/// # Parameters
/// - `dest`: Main repository path
/// - `path`: Path of the worktree to remove
/// - `force`: Whether to force removal even if dirty
/// - `delete_remote_branch`: Whether to also delete the remote branch
/// - `remote`: Remote name for branch deletion (defaults to "origin")
/// - `use_stored_credential`: Whether to use stored credentials for remote operations
#[tauri::command(rename_all = "camelCase")]
pub async fn git_worktree_remove(
    dest: String,
    path: String,
    force: Option<bool>,
    delete_remote_branch: Option<bool>,
    remote: Option<String>,
    use_stored_credential: Option<bool>,
    credential_factory: State<'_, SharedCredentialFactory>,
) -> Result<(), String> {
    let repo_path = Path::new(&dest);
    if !repo_path.exists() {
        return Err("Repository path does not exist".to_string());
    }

    let wt_path = Path::new(&path);
    if !wt_path.exists() {
        return Err(format!("Worktree path does not exist: {}", path));
    }

    let repo = git2::Repository::open(repo_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;

    // 1. Find the worktree and branch name
    // We need to iterate worktrees to find the one matching 'path' to get its name/branch
    let mut wt_name: Option<String> = None;
    let mut branch_name: Option<String> = None;

    if let Ok(wts) = repo.worktrees() {
        for name in wts.iter() {
            if let Some(name) = name {
                if let Ok(wt) = repo.find_worktree(name) {
                    // Check if paths match
                    let current_wt_path = wt.path();
                    // Normalize for comparison
                    if current_wt_path.canonicalize().ok() == wt_path.canonicalize().ok()
                        || current_wt_path == wt_path
                    {
                        wt_name = Some(name.to_string());

                        // Get branch name
                        if let Ok(wt_repo) = git2::Repository::open(current_wt_path) {
                            if let Ok(head) = wt_repo.head() {
                                if let Some(s) = head.shorthand() {
                                    branch_name = Some(s.to_string());
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    let name = wt_name.ok_or_else(|| "Could not find registered worktree for path".to_string())?;

    // 2. Validate status and Remove Worktree
    let worktree = repo
        .find_worktree(&name)
        .map_err(|e| format!("Failed to find worktree: {}", e))?;

    // Check clean status if not forced
    if !force.unwrap_or(false) {
        if let Ok(wt_repo) = git2::Repository::open(wt_path) {
            let statuses = wt_repo
                .statuses(None)
                .map_err(|e| format!("Failed to check status: {}", e))?;
            if !statuses.is_empty() {
                return Err("Worktree is dirty. Use force to remove it.".to_string());
            }
        }
    }

    // Prune logic (remove from main repo)
    // Note: libgit2 prune does NOT delete the directory usually, it just removes metadata.
    // git worktree remove does both.
    let mut opts = git2::WorktreePruneOptions::new();
    opts.valid(true); // Don't prune if locked?

    worktree
        .prune(Some(&mut opts))
        .map_err(|e| format!("Failed to prune worktree: {}", e))?;

    // Delete directory
    // We try to remove the directory. If it fails (e.g. file lock), we warn but don't fail the operation
    // since the worktree is already pruned from git.
    if let Err(e) = std::fs::remove_dir_all(wt_path) {
        tracing::warn!("Failed to remove worktree directory {}: {}", path, e);
    }

    // 3. Delete Remote Branch
    if delete_remote_branch.unwrap_or(false) {
        if let Some(branch) = branch_name {
            let remote_name = remote.as_deref().unwrap_or("origin");

            // Get credentials if needed
            let use_stored = use_stored_credential.unwrap_or(false);
            let mut creds: Option<(String, String)> = None;
            if use_stored {
                if let Ok(Some((u, p))) = try_get_git_credentials(&dest, &credential_factory).await
                {
                    creds = Some((u, p));
                }
            }

            // Use Git2Runner to push deletion
            let runner = crate::core::git::Git2Runner::new();
            let should_interrupt = std::sync::atomic::AtomicBool::new(false);
            let refspec = format!(":refs/heads/{}", branch);

            let cred_refs = creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));

            // We need to implement GitRunner for Git2Runner, which we did.
            use crate::core::git::runner::GitRunner;

            let result = runner.push_repo(
                repo_path,
                Some(remote_name),
                Some(&[&refspec]),
                cred_refs,
                &should_interrupt,
                &mut |_| {}, // Ignore progress
            );

            match result {
                Ok(_) => {
                    tracing::info!(
                        target = "git",
                        "Successfully deleted remote branch: {}/{}",
                        remote_name,
                        branch
                    );
                }
                Err(e) => {
                    tracing::warn!(target = "git", "Failed to delete remote branch: {}", e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // parse_git_host tests
    // -------------------------------------------------------------------------
    #[test]
    fn test_parse_git_host_https() {
        assert_eq!(
            parse_git_host("https://github.com/user/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            parse_git_host("http://gitlab.com/user/repo").unwrap(),
            "gitlab.com"
        );
        assert_eq!(
            parse_git_host("https+custom://internal.git.com/repo").unwrap(),
            "internal.git.com"
        );
    }

    #[test]
    fn test_parse_git_host_https_with_user_and_port() {
        assert_eq!(
            parse_git_host("https://user:pass@github.com/user/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            parse_git_host("https://github.com:8443/user/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            parse_git_host("https://user@gitlab.com:8080/repo").unwrap(),
            "gitlab.com"
        );
    }

    #[test]
    fn test_parse_git_host_ssh() {
        assert_eq!(
            parse_git_host("git@github.com:user/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            parse_git_host("ssh://git@github.com/user/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            parse_git_host("ssh://github.com/user/repo.git").unwrap(),
            "github.com"
        );
    }

    #[test]
    fn test_parse_git_host_invalid() {
        let result = parse_git_host("not-a-git-url");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported Git URL format"));
    }

    // -------------------------------------------------------------------------
    // extract_git_host tests
    // -------------------------------------------------------------------------
    #[test]
    fn test_extract_git_host_not_repo() {
        let temp = tempfile::tempdir().unwrap();
        let result = extract_git_host(&temp.path().to_string_lossy());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a git repository"));
    }

    #[test]
    fn test_extract_git_host_valid() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();

        // Init repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();

        // Add remote
        std::process::Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/test/repo.git",
            ])
            .current_dir(path)
            .output()
            .unwrap();

        let result = extract_git_host(&path.to_string_lossy());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "github.com");
    }
}
