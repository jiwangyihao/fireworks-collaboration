//! Git operation commands.

use tauri::State;

use crate::core::credential::storage::CredentialStore;
use crate::core::tasks::TaskKind;

use super::super::types::{SharedCredentialFactory, TaskRegistryState};

/// Helper to parse optional depth parameter from JSON value.
fn parse_depth(depth: Option<serde_json::Value>) -> Option<u32> {
    depth.and_then(|v| v.as_u64().map(|x| x as u32))
}

/// Clone a Git repository.
///
/// # Parameters
/// - `repo`: Repository URL
/// - `dest`: Destination path
/// - `depth`: Optional shallow clone depth
/// - `filter`: Optional object filter (e.g., "blob:none")
/// - `strategy_override`: Optional strategy configuration override
/// - `recurse_submodules`: Whether to recursively clone submodules (P7.1)
#[tauri::command]
pub async fn git_clone(
    repo: String,
    dest: String,
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
    recurse_submodules: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
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
        Some(app),
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
#[tauri::command]
pub async fn git_fetch(
    repo: String,
    dest: String,
    preset: Option<String>,
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
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
        Some(app),
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
#[tauri::command]
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
    app: tauri::AppHandle,
) -> Result<String, String> {
    // Determine final username and password
    let (final_username, final_password) =
        if use_stored_credential.unwrap_or(false) && username.is_none() && password.is_none() {
            // Try to get credentials from storage
            match try_get_git_credentials(&dest, &credential_factory).await {
                Ok(Some((u, p))) => {
                    tracing::info!("Using stored credentials for git push");
                    (Some(u), Some(p))
                }
                Ok(None) => {
                    tracing::debug!("No stored credentials found, using provided credentials");
                    (username.clone(), password.clone())
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to retrieve stored credentials: {}, using provided credentials",
                        e
                    );
                    (username.clone(), password.clone())
                }
            }
        } else {
            (username.clone(), password.clone())
        };

    let (id, token) = reg.create(TaskKind::GitPush {
        dest: dest.clone(),
        remote: remote.clone(),
        refspecs: refspecs.clone(),
        username: final_username.clone(),
        password: final_password.clone(),
        strategy_override: strategy_override.clone(),
    });

    reg.clone().spawn_git_push_task(
        Some(app),
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
///
/// Extracts the host from the repository path (if it's a Git remote URL)
/// and looks up credentials in the credential store.
async fn try_get_git_credentials(
    repo_path: &str,
    credential_factory: &State<'_, SharedCredentialFactory>,
) -> Result<Option<(String, String)>, String> {
    // Try to extract host from common Git URL formats:
    // - https://github.com/user/repo.git
    // - git@github.com:user/repo.git
    // - ssh://git@github.com/user/repo.git

    // For now, we'll try to read the remote URL from the Git config
    // and extract the host from there
    let host = extract_git_host(repo_path)?;

    // Get credential store
    let factory_guard = credential_factory
        .lock()
        .map_err(|e| format!("Failed to lock credential factory: {}", e))?;

    let store = factory_guard
        .as_ref()
        .ok_or("Credential store not initialized")?;

    // Try to get credential for this host (any username)
    match store.get(&host, None).map_err(|e| e.to_string())? {
        Some(cred) => Ok(Some((
            cred.username().to_string(),
            cred.password_or_token().to_string(),
        ))),
        None => Ok(None),
    }
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
pub(crate) fn parse_git_host(url: &str) -> Result<String, String> {
    // HTTPS: https://github.com/user/repo.git
    if url.starts_with("https://") || url.starts_with("http://") {
        let without_scheme = url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let host = without_scheme
            .split('/')
            .next()
            .ok_or("Invalid HTTPS URL")?;
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
#[tauri::command]
pub async fn git_init(
    dest: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitInit { dest: dest.clone() });
    reg.clone().spawn_git_init_task(Some(app), id, token, dest);
    Ok(id.to_string())
}

/// Stage files for commit.
///
/// # Parameters
/// - `dest`: Repository path
/// - `paths`: List of file paths to stage (relative to repository root)
#[tauri::command]
pub async fn git_add(
    dest: String,
    paths: Vec<String>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitAdd {
        dest: dest.clone(),
        paths: paths.clone(),
    });

    reg.clone()
        .spawn_git_add_task(Some(app), id, token, dest, paths);

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
#[tauri::command]
pub async fn git_commit(
    dest: String,
    message: String,
    allow_empty: Option<bool>,
    author_name: Option<String>,
    author_email: Option<String>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
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
        Some(app),
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
#[tauri::command]
pub async fn git_branch(
    dest: String,
    name: String,
    checkout: Option<bool>,
    force: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let checkout_flag = checkout.unwrap_or(false);
    let force_flag = force.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitBranch {
        dest: dest.clone(),
        name: name.clone(),
        checkout: checkout_flag,
        force: force_flag,
    });

    reg.clone()
        .spawn_git_branch_task(Some(app), id, token, dest, name, checkout_flag, force_flag);

    Ok(id.to_string())
}

/// Checkout a branch or commit.
///
/// # Parameters
/// - `dest`: Repository path
/// - `reference`: Branch name or commit reference
/// - `create`: Whether to create the branch if it doesn't exist
#[tauri::command]
pub async fn git_checkout(
    dest: String,
    reference: String,
    create: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let create_flag = create.unwrap_or(false);

    let (id, token) = reg.create(TaskKind::GitCheckout {
        dest: dest.clone(),
        reference: reference.clone(),
        create: create_flag,
    });

    reg.clone()
        .spawn_git_checkout_task(Some(app), id, token, dest, reference, create_flag);

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
#[tauri::command]
pub async fn git_tag(
    dest: String,
    name: String,
    message: Option<String>,
    annotated: Option<bool>,
    force: Option<bool>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
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
        Some(app),
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
#[tauri::command]
pub async fn git_remote_set(
    dest: String,
    name: String,
    url: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteSet {
        dest: dest.clone(),
        name: name.clone(),
        url: url.clone(),
    });

    reg.clone()
        .spawn_git_remote_set_task(Some(app), id, token, dest, name, url);

    Ok(id.to_string())
}

/// Add a new remote.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Remote name
/// - `url`: Remote URL
#[tauri::command]
pub async fn git_remote_add(
    dest: String,
    name: String,
    url: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteAdd {
        dest: dest.clone(),
        name: name.clone(),
        url: url.clone(),
    });

    reg.clone()
        .spawn_git_remote_add_task(Some(app), id, token, dest, name, url);

    Ok(id.to_string())
}

/// Remove a remote.
///
/// # Parameters
/// - `dest`: Repository path
/// - `name`: Remote name to remove
#[tauri::command]
pub async fn git_remote_remove(
    dest: String,
    name: String,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitRemoteRemove {
        dest: dest.clone(),
        name: name.clone(),
    });

    reg.clone()
        .spawn_git_remote_remove_task(Some(app), id, token, dest, name);

    Ok(id.to_string())
}
