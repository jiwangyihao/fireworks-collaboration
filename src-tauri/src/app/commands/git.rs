//! Git operation commands.

use tauri::State;

use crate::core::tasks::TaskKind;

use super::super::types::TaskRegistryState;

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
#[tauri::command]
pub async fn git_clone(
    repo: String,
    dest: String,
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let depth_parsed = parse_depth(depth.clone());
    
    let (id, token) = reg.create(TaskKind::GitClone {
        repo: repo.clone(),
        dest: dest.clone(),
        depth: depth_parsed,
        filter: filter.clone(),
        strategy_override: strategy_override.clone(),
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
/// - `strategy_override`: Optional strategy configuration override
#[tauri::command]
pub async fn git_push(
    dest: String,
    remote: Option<String>,
    refspecs: Option<Vec<String>>,
    username: Option<String>,
    password: Option<String>,
    strategy_override: Option<serde_json::Value>,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitPush {
        dest: dest.clone(),
        remote: remote.clone(),
        refspecs: refspecs.clone(),
        username: username.clone(),
        password: password.clone(),
        strategy_override: strategy_override.clone(),
    });
    
    reg.clone().spawn_git_push_task(
        Some(app),
        id,
        token,
        dest,
        remote,
        refspecs,
        username,
        password,
        strategy_override,
    );
    
    Ok(id.to_string())
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
    
    reg.clone().spawn_git_branch_task(
        Some(app),
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
    
    reg.clone().spawn_git_checkout_task(
        Some(app),
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
