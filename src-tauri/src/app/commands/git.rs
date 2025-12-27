//! Git operation commands.

use tauri::State;

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
    use std::process::Command;

    let path = Path::new(repo_path);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Try to get the origin remote URL
    let output = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url")
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git config: {}", e))?;

    if !output.status.success() {
        return Err("Failed to get remote URL".to_string());
    }

    let url = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in remote URL: {}", e))?
        .trim()
        .to_string();

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
use std::process::Command;

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
    let path = Path::new(&dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let include_remote_flag = include_remote.unwrap_or(false);

    // Get all branches with details
    let mut args = vec!["branch", "-v", "--no-color"];
    if include_remote_flag {
        args.push("-a");
    }

    let output = Command::new("git")
        .args(&args)
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to run git branch: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git branch failed: {}", stderr));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in git output: {}", e))?;

    let mut branches = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let is_current = line.starts_with('*');
        let line = line.trim_start_matches('*').trim();

        // Skip HEAD pointer for detached state
        if line.starts_with("(HEAD detached") {
            continue;
        }

        // Parse branch name and commit
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        let name = parts.first().unwrap_or(&"").to_string();

        if name.is_empty() {
            continue;
        }

        let is_remote = name.starts_with("remotes/");
        let display_name = if is_remote {
            name.trim_start_matches("remotes/").to_string()
        } else {
            name.clone()
        };

        // Extract commit hash if present
        let commit = if parts.len() > 1 {
            let rest = parts[1].trim();
            rest.split_whitespace().next().map(|s| s.to_string())
        } else {
            None
        };

        branches.push(BranchInfo {
            name: display_name,
            is_current,
            is_remote,
            upstream: None, // TODO: extract upstream tracking info
            commit,
        });
    }

    Ok(branches)
}

/// Get the status of a Git repository.
///
/// # Parameters
/// - `dest`: Repository path
#[tauri::command(rename_all = "camelCase")]
pub async fn git_repo_status(dest: String) -> Result<RepoStatus, String> {
    let path = Path::new(&dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    // Get current branch
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to get current branch: {}", e))?;

    let current_branch = if branch_output.status.success() {
        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();
        if branch == "HEAD" {
            None // Detached HEAD
        } else {
            Some(branch)
        }
    } else {
        None
    };

    let is_detached = current_branch.is_none();

    // Get status --porcelain for file counts
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to get status: {}", e))?;

    let status_text = String::from_utf8_lossy(&status_output.stdout);
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;

    for line in status_text.lines() {
        if line.len() < 2 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');

        if index_status == '?' {
            untracked += 1;
        } else {
            if index_status != ' ' && index_status != '?' {
                staged += 1;
            }
            if worktree_status != ' ' && worktree_status != '?' {
                unstaged += 1;
            }
        }
    }

    let is_clean = staged == 0 && unstaged == 0 && untracked == 0;

    // Get ahead/behind counts
    let mut ahead = 0u32;
    let mut behind = 0u32;

    if let Some(ref branch) = current_branch {
        let ab_output = Command::new("git")
            .args([
                "rev-list",
                "--left-right",
                "--count",
                &format!("{}...@{{u}}", branch),
            ])
            .current_dir(&dest)
            .output();

        if let Ok(output) = ab_output {
            if output.status.success() {
                let ab_text = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = ab_text.trim().split_whitespace().collect();
                if parts.len() == 2 {
                    ahead = parts[0].parse().unwrap_or(0);
                    behind = parts[1].parse().unwrap_or(0);
                }
            }
        }
    }

    // Get upstream tracking branch
    let tracking_branch = if let Some(ref _branch) = current_branch {
        let tb_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
            .current_dir(&dest)
            .output();

        if let Ok(output) = tb_output {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Get branches
    let branches = git_list_branches(dest.clone(), Some(false))
        .await
        .unwrap_or_default();

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
#[tauri::command(rename_all = "camelCase")]
pub async fn git_delete_branch(
    dest: String,
    name: String,
    force: Option<bool>,
) -> Result<(), String> {
    let path = Path::new(&dest);
    if !path.exists() || !path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let delete_flag = if force.unwrap_or(false) { "-D" } else { "-d" };

    let output = Command::new("git")
        .args(["branch", delete_flag, &name])
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to delete branch: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to delete branch: {}", stderr));
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

    // Optionally fetch first
    if fetch_first.unwrap_or(false) {
        let _ = Command::new("git")
            .args(["fetch", remote_name, "--prune"])
            .current_dir(&dest)
            .output();
    }

    // Get remote branches
    let output = Command::new("git")
        .args(["branch", "-r", "--list", &format!("{}/*", remote_name)])
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to list remote branches: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git branch -r failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<String> = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.contains("->")) // Filter out HEAD pointer
        .map(|line| line.to_string())
        .collect();

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

    // Check if it's a git repository (either .git dir or .git file for worktrees)
    let git_path = path.join(".git");
    if !git_path.exists() {
        return Err("Not a git repository".to_string());
    }

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to list worktrees: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree list failed: {}", stderr));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in git output: {}", e))?;

    let mut worktrees = Vec::new();
    let mut current_wt: Option<WorktreeInfo> = None;
    let mut is_first = true;

    for line in stdout.lines() {
        if line.is_empty() {
            // Empty line marks end of a worktree entry
            if let Some(wt) = current_wt.take() {
                worktrees.push(wt);
            }
            continue;
        }

        if line.starts_with("worktree ") {
            // Start of a new worktree entry
            if let Some(wt) = current_wt.take() {
                worktrees.push(wt);
            }
            let wt_path = line.trim_start_matches("worktree ").to_string();
            current_wt = Some(WorktreeInfo {
                path: wt_path,
                head: None,
                branch: None,
                is_bare: false,
                is_main: is_first,
                is_detached: false,
                locked: false,
                prunable: false,
                tracking_branch: None,
                ahead: 0,
                behind: 0,
            });
            is_first = false;
        } else if let Some(ref mut wt) = current_wt {
            if line.starts_with("HEAD ") {
                wt.head = Some(line.trim_start_matches("HEAD ").to_string());
            } else if line.starts_with("branch ") {
                let branch = line.trim_start_matches("branch refs/heads/").to_string();
                wt.branch = Some(branch);
            } else if line == "bare" {
                wt.is_bare = true;
            } else if line == "detached" {
                wt.is_detached = true;
            } else if line == "locked" {
                wt.locked = true;
            } else if line == "prunable" {
                wt.prunable = true;
            }
        }
    }

    // Don't forget the last entry
    if let Some(wt) = current_wt.take() {
        worktrees.push(wt);
    }

    // Enhance worktrees with tracking info
    // We do this after parsing all worktrees to minimize git commands during parsing
    for wt in &mut worktrees {
        if let Some(ref branch) = wt.branch {
            // Get upstream tracking branch
            // Note: We need to run this command within the worktree path or separate the logic
            // Using the worktree path is safer as it respects that worktree's config/HEAD
            let wt_path = Path::new(&wt.path);

            if wt_path.exists() {
                let mut tracking_branch_name: Option<String> = None;

                tracing::info!(
                    target = "git",
                    "Checking tracking info for worktree: {:?}, branch: {:?}",
                    wt_path,
                    branch
                );

                // 1. Try configured upstream
                let tb_output = Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
                    .current_dir(wt_path)
                    .output();

                if let Ok(output) = tb_output {
                    if output.status.success() {
                        let tb = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !tb.is_empty() {
                            tracing::info!(
                                target = "git",
                                "Found configured tracking branch for {:?}: {}",
                                wt_path,
                                tb
                            );
                            tracking_branch_name = Some(tb);
                        }
                    } else {
                        tracing::warn!(
                            target = "git",
                            "No configured upstream for {:?}: {}",
                            wt_path,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }

                // 2. Fallback to origin/<branch> if no configured upstream
                if tracking_branch_name.is_none() {
                    let implicit_tracking = format!("origin/{}", branch);
                    tracing::info!(
                        target = "git",
                        "Checking implicit tracking branch: {}",
                        implicit_tracking
                    );

                    let verify_output = Command::new("git")
                        .args(["rev-parse", "--verify", &implicit_tracking])
                        .current_dir(wt_path)
                        .output();

                    if let Ok(output) = verify_output {
                        if output.status.success() {
                            tracing::info!(
                                target = "git",
                                "Found implicit tracking branch: {}",
                                implicit_tracking
                            );
                            tracking_branch_name = Some(implicit_tracking);
                        } else {
                            tracing::warn!(
                                target = "git",
                                "Implicit tracking branch {} not found",
                                implicit_tracking
                            );
                        }
                    }
                }

                if let Some(tb) = tracking_branch_name {
                    wt.tracking_branch = Some(tb.clone());

                    // Get ahead/behind
                    // Use the resolved tracking branch name explicitly
                    let ab_output = Command::new("git")
                        .args([
                            "rev-list",
                            "--left-right",
                            "--count",
                            &format!("HEAD...{}", tb),
                        ])
                        .current_dir(wt_path)
                        .output();

                    if let Ok(ab_out) = ab_output {
                        if ab_out.status.success() {
                            let ab_text = String::from_utf8_lossy(&ab_out.stdout);
                            let parts: Vec<&str> = ab_text.trim().split_whitespace().collect();
                            if parts.len() == 2 {
                                wt.ahead = parts[0].parse().unwrap_or(0);
                                wt.behind = parts[1].parse().unwrap_or(0);
                                tracing::info!(
                                    target = "git",
                                    "Counts for {:?} against {}: +{}/-{}",
                                    wt_path,
                                    tb,
                                    wt.ahead,
                                    wt.behind
                                );
                            }
                        } else {
                            tracing::warn!(
                                target = "git",
                                "Failed to get counts for {:?}: {}",
                                wt_path,
                                String::from_utf8_lossy(&ab_out.stderr)
                            );
                        }
                    }
                }
            } else {
                tracing::warn!(
                    target = "git",
                    "Worktree path does not exist: {:?}",
                    wt_path
                );
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
) -> Result<(), String> {
    let repo_path = Path::new(&dest);
    if !repo_path.exists() || !repo_path.join(".git").exists() {
        return Err("Not a git repository".to_string());
    }

    let wt_path = Path::new(&path);
    if wt_path.exists() {
        return Err(format!("Worktree path already exists: {}", path));
    }

    // If from_remote is provided, fetch first to ensure we have the latest
    if let Some(ref remote_ref) = from_remote {
        // Extract remote name from ref (e.g., "origin" from "origin/feature-x")
        if let Some(remote_name) = remote_ref.split('/').next() {
            tracing::info!(
                target = "git",
                "Fetching from remote {} before creating worktree",
                remote_name
            );
            let fetch_output = Command::new("git")
                .args(["fetch", remote_name])
                .current_dir(&dest)
                .output();

            if let Ok(output) = fetch_output {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!(target = "git", "Fetch warning: {}", stderr);
                    // Don't fail on fetch error, the branch might already exist locally
                }
            }
        }
    }

    let mut args = vec!["worktree", "add"];

    // Build command based on parameters
    if let Some(ref remote_ref) = from_remote {
        // Create new branch from remote ref: git worktree add -b <branch> <path> <remote_ref>
        args.push("-b");
        args.push(&branch);
        args.push(&path);
        args.push(remote_ref);
    } else if create_branch.unwrap_or(true) {
        // Create new branch with -b flag
        args.push("-b");
        args.push(&branch);
        args.push(&path);
    } else {
        // Checkout existing branch
        args.push(&path);
        args.push(&branch);
    }

    tracing::info!(target = "git", "Running: git {}", args.join(" "));

    let output = Command::new("git")
        .args(&args)
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to add worktree: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", stderr));
    }

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

    // Get the branch name from the worktree before removing it
    let branch_name = if delete_remote_branch.unwrap_or(false) {
        // Get worktree info to find the branch
        let list_output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&dest)
            .output()
            .map_err(|e| format!("Failed to list worktrees: {}", e))?;

        let stdout = String::from_utf8_lossy(&list_output.stdout);
        let mut current_wt_path: Option<String> = None;
        let mut branch: Option<String> = None;

        for line in stdout.lines() {
            if line.starts_with("worktree ") {
                current_wt_path = Some(line.trim_start_matches("worktree ").to_string());
            } else if line.starts_with("branch ") {
                if let Some(ref wt) = current_wt_path {
                    // Normalize paths for comparison
                    let normalized_wt = wt.replace('\\', "/");
                    let normalized_path = path.replace('\\', "/");
                    if normalized_wt == normalized_path {
                        // Extract branch name from "refs/heads/<branch>"
                        let full_ref = line.trim_start_matches("branch ");
                        branch = Some(full_ref.trim_start_matches("refs/heads/").to_string());
                        break;
                    }
                }
            }
        }
        branch
    } else {
        None
    };

    // Remove the worktree
    let mut args = vec!["worktree", "remove"];
    if force.unwrap_or(false) {
        args.push("--force");
    }
    args.push(&path);

    tracing::info!(target = "git", "Running: git {}", args.join(" "));

    let output = Command::new("git")
        .args(&args)
        .current_dir(&dest)
        .output()
        .map_err(|e| format!("Failed to remove worktree: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree remove failed: {}", stderr));
    }

    // Delete remote branch if requested
    if delete_remote_branch.unwrap_or(false) {
        if let Some(ref branch) = branch_name {
            let remote_name = remote.as_deref().unwrap_or("origin");
            tracing::info!(
                target = "git",
                "Deleting remote branch: {} on {}",
                branch,
                remote_name
            );

            // Get credentials if needed
            let creds: Option<(String, String)> = if use_stored_credential.unwrap_or(false) {
                match try_get_git_credentials(&dest, &credential_factory).await {
                    Ok(Some((u, p))) => {
                        tracing::info!(
                            target = "git",
                            "Using stored credentials for remote branch deletion"
                        );
                        Some((u, p))
                    }
                    _ => None,
                }
            } else {
                None
            };

            // Build the push --delete command
            let delete_ref = format!(":refs/heads/{}", branch);

            // Use git push with credentials in URL if available
            let push_result = if let Some((user, pass)) = creds {
                // Get remote URL and embed credentials
                let url_output = Command::new("git")
                    .args(["remote", "get-url", remote_name])
                    .current_dir(&dest)
                    .output();

                if let Ok(url_out) = url_output {
                    if url_out.status.success() {
                        let url = String::from_utf8_lossy(&url_out.stdout).trim().to_string();
                        // Embed credentials in URL
                        if let Ok(mut parsed) = url::Url::parse(&url) {
                            let _ = parsed.set_username(&user);
                            let _ = parsed.set_password(Some(&pass));
                            let auth_url = parsed.to_string();

                            Command::new("git")
                                .args(["push", &auth_url, &delete_ref])
                                .current_dir(&dest)
                                .output()
                        } else {
                            // URL parsing failed, try without auth
                            Command::new("git")
                                .args(["push", remote_name, "--delete", branch])
                                .current_dir(&dest)
                                .output()
                        }
                    } else {
                        Command::new("git")
                            .args(["push", remote_name, "--delete", branch])
                            .current_dir(&dest)
                            .output()
                    }
                } else {
                    Command::new("git")
                        .args(["push", remote_name, "--delete", branch])
                        .current_dir(&dest)
                        .output()
                }
            } else {
                Command::new("git")
                    .args(["push", remote_name, "--delete", branch])
                    .current_dir(&dest)
                    .output()
            };

            match push_result {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        tracing::warn!(
                            target = "git",
                            "Failed to delete remote branch: {}",
                            stderr
                        );
                        // Don't fail the whole operation, just log the warning
                    } else {
                        tracing::info!(
                            target = "git",
                            "Successfully deleted remote branch: {}/{}",
                            remote_name,
                            branch
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(target = "git", "Failed to execute push --delete: {}", e);
                }
            }
        } else {
            tracing::warn!(
                target = "git",
                "Could not determine branch name for worktree, skipping remote deletion"
            );
        }
    }

    Ok(())
}
