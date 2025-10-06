//! Workspace management commands.
//!
//! This module provides Tauri command handlers for workspace operations,
//! including create, load, save, add/remove repositories, and status queries.

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{debug, error, info, warn};

use super::super::types::{SharedConfig, TaskRegistryState};
use crate::core::tasks::{
    model::WorkspaceBatchOperation,
    workspace_batch::{
        CloneOptions, FetchOptions, PushOptions, WorkspaceBatchChildOperation,
        WorkspaceBatchChildSpec,
    },
    TaskKind,
};
use crate::core::workspace::{
    model::{RepositoryEntry, Workspace, WorkspaceConfig},
    status::{StatusQuery, WorkspaceStatusResponse, WorkspaceStatusService},
    storage::WorkspaceStorage,
};

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Shared workspace state (just holds the current workspace).
pub type SharedWorkspaceManager = Arc<Mutex<Option<Workspace>>>;
/// Shared workspace status service.
pub type SharedWorkspaceStatusService = Arc<WorkspaceStatusService>;

/// Workspace information for frontend.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub name: String,
    pub root_path: String,
    pub repositories: Vec<RepositoryInfo>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: HashMap<String, String>,
}

impl From<&Workspace> for WorkspaceInfo {
    fn from(ws: &Workspace) -> Self {
        Self {
            name: ws.name.clone(),
            root_path: ws.root_path.to_string_lossy().to_string(),
            repositories: ws.repositories.iter().map(RepositoryInfo::from).collect(),
            created_at: ws.created_at.clone(),
            updated_at: ws.updated_at.clone(),
            metadata: ws.metadata.clone(),
        }
    }
}

/// Repository information for frontend.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub remote_url: String,
    pub tags: Vec<String>,
    pub enabled: bool,
}

impl From<&RepositoryEntry> for RepositoryInfo {
    fn from(repo: &RepositoryEntry) -> Self {
        Self {
            id: repo.id.clone(),
            name: repo.name.clone(),
            path: repo.path.to_string_lossy().to_string(),
            remote_url: repo.remote_url.clone(),
            tags: repo.tags.clone(),
            enabled: repo.enabled,
        }
    }
}

/// Create workspace request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub root_path: String,
    pub metadata: Option<HashMap<String, String>>,
}

/// Add repository request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRepositoryRequest {
    pub id: String,
    pub name: String,
    pub path: String,
    pub remote_url: String,
    pub tags: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// Update repository request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRepositoryRequest {
    pub id: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub remote_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

/// Batch clone request options.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBatchCloneRequest {
    pub repo_ids: Option<Vec<String>>,
    pub include_disabled: Option<bool>,
    pub max_concurrency: Option<usize>,
    pub depth: Option<serde_json::Value>,
    pub filter: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
    pub recurse_submodules: Option<bool>,
}

/// Batch fetch request options.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBatchFetchRequest {
    pub repo_ids: Option<Vec<String>>,
    pub include_disabled: Option<bool>,
    pub max_concurrency: Option<usize>,
    pub preset: Option<String>,
    pub depth: Option<serde_json::Value>,
    pub filter: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
}

/// Batch push request options.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBatchPushRequest {
    pub repo_ids: Option<Vec<String>>,
    pub include_disabled: Option<bool>,
    pub max_concurrency: Option<usize>,
    pub remote: Option<String>,
    pub refspecs: Option<Vec<String>>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub strategy_override: Option<serde_json::Value>,
}

/// Create a new workspace.
#[tauri::command]
pub async fn create_workspace(
    request: CreateWorkspaceRequest,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<WorkspaceInfo, String> {
    info!("Creating workspace: {}", request.name);

    let mut ws = Workspace::new(request.name.clone(), PathBuf::from(&request.root_path));

    if let Some(metadata) = request.metadata {
        ws.metadata = metadata;
    }

    let info = WorkspaceInfo::from(&ws);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    *manager_guard = Some(ws);

    info!("Workspace '{}' created successfully", request.name);
    Ok(info)
}

/// Load workspace from file.
#[tauri::command]
pub async fn load_workspace(
    path: String,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<WorkspaceInfo, String> {
    info!("Loading workspace from: {}", path);

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new(workspace_path.clone());

    let ws = storage.load().map_err(|e| {
        error!("Failed to load workspace from {}: {}", path, e);
        format!("Failed to load workspace: {}", e)
    })?;

    let info = WorkspaceInfo::from(&ws);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    *manager_guard = Some(ws);

    info!("Workspace loaded successfully from {}", path);
    Ok(info)
}

/// Save current workspace to file.
#[tauri::command]
pub async fn save_workspace(
    path: String,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<(), String> {
    info!("Saving workspace to: {}", path);

    let manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace = manager_guard.as_ref().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new(workspace_path.clone());

    storage.save(workspace).map_err(|e| {
        error!("Failed to save workspace to {}: {}", path, e);
        format!("Failed to save workspace: {}", e)
    })?;

    info!("Workspace saved successfully to {}", path);
    Ok(())
}

/// Get current workspace information.
#[tauri::command]
pub async fn get_workspace(
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<WorkspaceInfo, String> {
    let manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace = manager_guard.as_ref().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    Ok(WorkspaceInfo::from(workspace))
}

/// Close current workspace.
#[tauri::command]
pub async fn close_workspace(manager: State<'_, SharedWorkspaceManager>) -> Result<(), String> {
    info!("Closing workspace");

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    *manager_guard = None;

    info!("Workspace closed successfully");
    Ok(())
}

/// Add a repository to the workspace.
#[tauri::command]
pub async fn add_repository(
    request: AddRepositoryRequest,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<(), String> {
    info!("Adding repository: {}", request.name);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_mut().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let mut repo = RepositoryEntry::new(
        request.id.clone(),
        request.name.clone(),
        PathBuf::from(&request.path),
        request.remote_url.clone(),
    );
    repo.tags = request.tags.unwrap_or_default();
    repo.enabled = request.enabled.unwrap_or(true);

    workspace_manager.add_repository(repo).map_err(|e| {
        error!("Failed to add repository {}: {}", request.name, e);
        e.to_string()
    })?;

    info!("Repository '{}' added successfully", request.name);
    Ok(())
}

/// Remove a repository from the workspace.
#[tauri::command]
pub async fn remove_repository(
    repo_id: String,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<(), String> {
    info!("Removing repository: {}", repo_id);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_mut().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    workspace_manager.remove_repository(&repo_id).map_err(|e| {
        error!("Failed to remove repository {}: {}", repo_id, e);
        e.to_string()
    })?;

    info!("Repository '{}' removed successfully", repo_id);
    Ok(())
}

/// Get a specific repository by ID.
#[tauri::command]
pub async fn get_repository(
    repo_id: String,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<RepositoryInfo, String> {
    let manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_ref().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let repo = workspace_manager.get_repository(&repo_id).ok_or_else(|| {
        warn!("Repository not found: {}", repo_id);
        format!("Repository '{}' not found", repo_id)
    })?;

    Ok(RepositoryInfo::from(repo))
}

/// List all repositories in the workspace.
#[tauri::command]
pub async fn list_repositories(
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<Vec<RepositoryInfo>, String> {
    let manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_ref().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let repos = workspace_manager
        .get_workspace()
        .repositories
        .iter()
        .map(RepositoryInfo::from)
        .collect();

    Ok(repos)
}

/// List enabled repositories in the workspace.
#[tauri::command]
pub async fn list_enabled_repositories(
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<Vec<RepositoryInfo>, String> {
    let manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_ref().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let repos = workspace_manager
        .get_enabled_repositories()
        .iter()
        .map(|&repo| RepositoryInfo::from(repo))
        .collect();

    Ok(repos)
}

/// Reorder repositories according to the provided sequence of repository IDs.
#[tauri::command]
pub async fn reorder_repositories(
    ordered_ids: Vec<String>,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<Vec<RepositoryInfo>, String> {
    info!("Reordering repositories: {:?}", ordered_ids);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_mut().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let workspace = workspace_manager.get_workspace_mut();
    apply_repository_reorder(workspace, &ordered_ids)?;

    info!("Repositories reordered successfully");

    Ok(workspace
        .repositories
        .iter()
        .map(RepositoryInfo::from)
        .collect())
}

fn apply_repository_reorder(
    workspace: &mut Workspace,
    ordered_ids: &[String],
) -> Result<(), String> {
    if ordered_ids.is_empty() {
        return Err("Ordered repository list must not be empty".to_string());
    }

    let mut seen = std::collections::HashSet::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        if !seen.insert(id) {
            return Err(format!("Duplicate repository id '{}' in order list", id));
        }
        if !workspace.repositories.iter().any(|repo| &repo.id == id) {
            return Err(format!("Repository '{}' not found", id));
        }
    }

    let mut existing = std::mem::take(&mut workspace.repositories);
    let mut reordered = Vec::with_capacity(existing.len());
    for id in ordered_ids {
        if let Some(index) = existing.iter().position(|repo| &repo.id == id) {
            reordered.push(existing.remove(index));
        }
    }

    if !existing.is_empty() {
        reordered.extend(existing.into_iter());
    }

    workspace.repositories = reordered;
    workspace.updated_at = chrono::Utc::now().to_rfc3339();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn build_workspace() -> Workspace {
        let mut ws = Workspace::new("demo".to_string(), PathBuf::from("/demo"));
        ws.add_repository(RepositoryEntry::new(
            "repo1".to_string(),
            "Repo One".to_string(),
            PathBuf::from("repo-1"),
            "https://example.com/r1.git".to_string(),
        ))
        .unwrap();
        ws.add_repository(RepositoryEntry::new(
            "repo2".to_string(),
            "Repo Two".to_string(),
            PathBuf::from("repo-2"),
            "https://example.com/r2.git".to_string(),
        ))
        .unwrap();
        ws.add_repository(RepositoryEntry::new(
            "repo3".to_string(),
            "Repo Three".to_string(),
            PathBuf::from("repo-3"),
            "https://example.com/r3.git".to_string(),
        ))
        .unwrap();
        ws
    }

    #[test]
    fn test_apply_repository_reorder_moves_matching_ids() {
        let mut ws = build_workspace();
        let ids = vec!["repo3".to_string(), "repo1".to_string()];
        apply_repository_reorder(&mut ws, &ids).unwrap();

        let ordered: Vec<_> = ws.repositories.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ordered, vec!["repo3", "repo1", "repo2"]);
    }

    #[test]
    fn test_apply_repository_reorder_errors_on_unknown_id() {
        let mut ws = build_workspace();
        let ids = vec!["repoX".to_string()];
        let result = apply_repository_reorder(&mut ws, &ids);
        assert!(result.is_err());
        let ordered: Vec<_> = ws.repositories.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ordered, vec!["repo1", "repo2", "repo3"]);
    }

    #[test]
    fn test_apply_repository_reorder_errors_on_duplicates() {
        let mut ws = build_workspace();
        let ids = vec!["repo1".to_string(), "repo1".to_string()];
        let result = apply_repository_reorder(&mut ws, &ids);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_repository_reorder_errors_on_empty_input() {
        let mut ws = build_workspace();
        let before = ws.updated_at.clone();
        let ids: Vec<String> = vec![];
        let result = apply_repository_reorder(&mut ws, &ids);
        assert!(result.is_err());
        assert_eq!(ws.updated_at, before);
    }

    #[test]
    fn test_apply_repository_reorder_updates_timestamp_on_success() {
        let mut ws = build_workspace();
        let before = ws.updated_at.clone();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ids = vec!["repo2".to_string(), "repo1".to_string()];
        apply_repository_reorder(&mut ws, &ids).unwrap();
        assert_ne!(ws.updated_at, before);
    }
}

/// Query cached workspace repository statuses.
#[tauri::command]
pub async fn get_workspace_statuses(
    request: Option<StatusQuery>,
    manager: State<'_, SharedWorkspaceManager>,
    status_service: State<'_, SharedWorkspaceStatusService>,
) -> Result<WorkspaceStatusResponse, String> {
    let query = request.unwrap_or_default();

    let workspace_snapshot = {
        let guard = manager.lock().map_err(|e| {
            error!("Failed to lock workspace manager: {}", e);
            format!("Workspace manager lock error: {}", e)
        })?;

        let workspace = guard.as_ref().ok_or_else(|| {
            warn!("No workspace loaded");
            "No workspace loaded".to_string()
        })?;

        workspace.clone()
    };

    status_service
        .query_statuses(&workspace_snapshot, query)
        .await
        .map_err(|e| {
            error!("Failed to collect workspace statuses: {}", e);
            e.to_string()
        })
}

/// Clear the workspace status cache.
#[tauri::command]
pub async fn clear_workspace_status_cache(
    status_service: State<'_, SharedWorkspaceStatusService>,
) -> Result<(), String> {
    status_service.clear_cache();
    debug!(
        target = "workspace::status",
        "Workspace status cache cleared"
    );
    Ok(())
}

/// Invalidate cached status for a specific repository.
#[tauri::command]
pub async fn invalidate_workspace_status_entry(
    repo_id: String,
    status_service: State<'_, SharedWorkspaceStatusService>,
) -> Result<bool, String> {
    let removed = status_service.invalidate_repo(&repo_id);
    if removed {
        debug!(
            target = "workspace::status",
            repo_id = %repo_id,
            "Workspace status cache entry invalidated"
        );
    } else {
        debug!(
            target = "workspace::status",
            repo_id = %repo_id,
            "Workspace status cache entry not present"
        );
    }
    Ok(removed)
}

/// Start a batch clone task for workspace repositories.
#[tauri::command]
pub async fn workspace_batch_clone(
    request: WorkspaceBatchCloneRequest,
    manager: State<'_, SharedWorkspaceManager>,
    reg: State<'_, TaskRegistryState>,
    config: State<'_, SharedConfig>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    info!("Starting workspace batch clone");

    let workspace = {
        let guard = manager.lock().map_err(|e| {
            error!("Failed to lock workspace manager: {}", e);
            format!("Workspace manager lock error: {}", e)
        })?;
        let ws = guard.as_ref().ok_or_else(|| {
            warn!("No workspace loaded");
            "No workspace loaded".to_string()
        })?;
        ws.clone()
    };

    let include_disabled = request.include_disabled.unwrap_or(false);
    let repos = select_workspace_repos(&workspace, request.repo_ids.as_deref(), include_disabled)?;
    if repos.is_empty() {
        return Err("No repositories selected for batch operation".into());
    }

    let root_path = resolve_workspace_root(&workspace.root_path)?;
    let depth_u32 = parse_depth_option(&request.depth)?;
    let concurrency = resolve_concurrency(request.max_concurrency, &config)?;

    let mut specs = Vec::with_capacity(repos.len());
    for repo in repos {
        if repo.remote_url.trim().is_empty() {
            return Err(format!("Repository '{}' has no remote URL", repo.id));
        }
        let dest_path = resolve_repo_path(&root_path, &repo.path);
        ensure_clone_destination(&dest_path)?;
        let dest_str = path_to_string(&dest_path)?;

        let clone_opts = CloneOptions {
            repo_url: repo.remote_url.clone(),
            dest: dest_str,
            depth_u32,
            depth_value: request.depth.clone(),
            filter: request.filter.clone(),
            strategy_override: request.strategy_override.clone(),
            recurse_submodules: request.recurse_submodules.unwrap_or(repo.has_submodules),
        };

        specs.push(WorkspaceBatchChildSpec {
            repo_id: repo.id.clone(),
            repo_name: repo.name.clone(),
            operation: WorkspaceBatchChildOperation::Clone(clone_opts),
        });
    }

    let operation = WorkspaceBatchOperation::Clone;
    let total = specs.len() as u32;
    let (parent_id, parent_token) = reg.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total,
    });

    reg.clone().spawn_workspace_batch_task(
        Some(app),
        parent_id,
        parent_token,
        operation,
        specs,
        concurrency,
    );

    Ok(parent_id.to_string())
}

/// Start a batch fetch task for workspace repositories.
#[tauri::command]
pub async fn workspace_batch_fetch(
    request: WorkspaceBatchFetchRequest,
    manager: State<'_, SharedWorkspaceManager>,
    reg: State<'_, TaskRegistryState>,
    config: State<'_, SharedConfig>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    info!("Starting workspace batch fetch");

    let workspace = {
        let guard = manager.lock().map_err(|e| {
            error!("Failed to lock workspace manager: {}", e);
            format!("Workspace manager lock error: {}", e)
        })?;
        let ws = guard.as_ref().ok_or_else(|| {
            warn!("No workspace loaded");
            "No workspace loaded".to_string()
        })?;
        ws.clone()
    };

    let include_disabled = request.include_disabled.unwrap_or(false);
    let repos = select_workspace_repos(&workspace, request.repo_ids.as_deref(), include_disabled)?;
    if repos.is_empty() {
        return Err("No repositories selected for batch operation".into());
    }

    let root_path = resolve_workspace_root(&workspace.root_path)?;
    let depth_u32 = parse_depth_option(&request.depth)?;
    let concurrency = resolve_concurrency(request.max_concurrency, &config)?;

    let mut specs = Vec::with_capacity(repos.len());
    for repo in repos {
        let dest_path = resolve_repo_path(&root_path, &repo.path);
        ensure_existing_repo(&dest_path)?;
        let dest_str = path_to_string(&dest_path)?;
        let remote = if repo.remote_url.trim().is_empty() {
            "".to_string()
        } else {
            repo.remote_url.clone()
        };

        let fetch_opts = FetchOptions {
            repo_url: remote,
            dest: dest_str,
            preset: request.preset.clone(),
            depth_u32,
            depth_value: request.depth.clone(),
            filter: request.filter.clone(),
            strategy_override: request.strategy_override.clone(),
        };

        specs.push(WorkspaceBatchChildSpec {
            repo_id: repo.id.clone(),
            repo_name: repo.name.clone(),
            operation: WorkspaceBatchChildOperation::Fetch(fetch_opts),
        });
    }

    let operation = WorkspaceBatchOperation::Fetch;
    let total = specs.len() as u32;
    let (parent_id, parent_token) = reg.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total,
    });

    reg.clone().spawn_workspace_batch_task(
        Some(app),
        parent_id,
        parent_token,
        operation,
        specs,
        concurrency,
    );

    Ok(parent_id.to_string())
}

/// Start a batch push task for workspace repositories.
#[tauri::command]
pub async fn workspace_batch_push(
    request: WorkspaceBatchPushRequest,
    manager: State<'_, SharedWorkspaceManager>,
    reg: State<'_, TaskRegistryState>,
    config: State<'_, SharedConfig>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    info!("Starting workspace batch push");

    let workspace = {
        let guard = manager.lock().map_err(|e| {
            error!("Failed to lock workspace manager: {}", e);
            format!("Workspace manager lock error: {}", e)
        })?;
        let ws = guard.as_ref().ok_or_else(|| {
            warn!("No workspace loaded");
            "No workspace loaded".to_string()
        })?;
        ws.clone()
    };

    let include_disabled = request.include_disabled.unwrap_or(false);
    let repos = select_workspace_repos(&workspace, request.repo_ids.as_deref(), include_disabled)?;
    if repos.is_empty() {
        return Err("No repositories selected for batch operation".into());
    }

    let root_path = resolve_workspace_root(&workspace.root_path)?;
    let concurrency = resolve_concurrency(request.max_concurrency, &config)?;

    let mut specs = Vec::with_capacity(repos.len());
    for repo in repos {
        let dest_path = resolve_repo_path(&root_path, &repo.path);
        ensure_existing_repo(&dest_path)?;
        let dest_str = path_to_string(&dest_path)?;

        let push_opts = PushOptions {
            dest: dest_str,
            remote: request.remote.clone(),
            refspecs: request.refspecs.clone(),
            username: request.username.clone(),
            password: request.password.clone(),
            strategy_override: request.strategy_override.clone(),
        };

        specs.push(WorkspaceBatchChildSpec {
            repo_id: repo.id.clone(),
            repo_name: repo.name.clone(),
            operation: WorkspaceBatchChildOperation::Push(push_opts),
        });
    }

    let operation = WorkspaceBatchOperation::Push;
    let total = specs.len() as u32;
    let (parent_id, parent_token) = reg.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total,
    });

    reg.clone().spawn_workspace_batch_task(
        Some(app),
        parent_id,
        parent_token,
        operation,
        specs,
        concurrency,
    );

    Ok(parent_id.to_string())
}

fn resolve_workspace_root(root: &PathBuf) -> Result<PathBuf, String> {
    if root.is_absolute() {
        Ok(root.clone())
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("Failed to resolve current directory: {}", e))?;
        Ok(cwd.join(root))
    }
}

fn resolve_repo_path(root: &Path, repo_path: &PathBuf) -> PathBuf {
    if repo_path.is_absolute() {
        repo_path.clone()
    } else {
        root.join(repo_path)
    }
}

fn select_workspace_repos(
    workspace: &Workspace,
    repo_ids: Option<&[String]>,
    include_disabled: bool,
) -> Result<Vec<RepositoryEntry>, String> {
    if let Some(ids) = repo_ids {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let repo = workspace
                .get_repository(id)
                .ok_or_else(|| format!("Repository '{}' not found", id))?;
            if !include_disabled && !repo.enabled {
                return Err(format!("Repository '{}' is disabled", id));
            }
            out.push(repo.clone());
        }
        Ok(out)
    } else {
        Ok(workspace
            .repositories
            .iter()
            .filter(|repo| include_disabled || repo.enabled)
            .cloned()
            .collect())
    }
}

fn parse_depth_option(depth: &Option<serde_json::Value>) -> Result<Option<u32>, String> {
    match depth {
        Some(value) => {
            if value.is_null() {
                Ok(None)
            } else if let Some(n) = value.as_u64() {
                Ok(Some(n as u32))
            } else {
                Err("Depth must be a non-negative integer".into())
            }
        }
        None => Ok(None),
    }
}

fn resolve_concurrency(
    requested: Option<usize>,
    config: &State<'_, SharedConfig>,
) -> Result<usize, String> {
    let default = config
        .lock()
        .map_err(|e| format!("Failed to lock configuration: {}", e))?
        .workspace
        .max_concurrent_repos;
    let value = requested.unwrap_or(default);
    if value == 0 {
        Err("maxConcurrency must be greater than 0".into())
    } else {
        Ok(value)
    }
}

fn ensure_clone_destination(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!("Destination '{}' already exists", path.display()));
    }
    ensure_parent_dir(path)
}

fn ensure_existing_repo(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Repository path '{}' does not exist",
            path.display()
        ));
    }
    if !path.join(".git").exists() {
        return Err(format!(
            "Repository '{}' is not initialized",
            path.display()
        ));
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
    }
    Ok(())
}

fn path_to_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Path '{}' is not valid UTF-8", path.display()))
}

/// Update repository tags.
#[tauri::command]
pub async fn update_repository_tags(
    repo_id: String,
    tags: Vec<String>,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<(), String> {
    info!("Updating tags for repository: {}", repo_id);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_mut().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let workspace = workspace_manager.get_workspace_mut();
    let repo = workspace
        .repositories
        .iter_mut()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| {
            warn!("Repository not found: {}", repo_id);
            format!("Repository '{}' not found", repo_id)
        })?;

    repo.tags = tags;
    workspace.updated_at = chrono::Local::now().to_rfc3339();

    info!("Tags updated for repository '{}'", repo_id);
    Ok(())
}

/// Toggle repository enabled state.
#[tauri::command]
pub async fn toggle_repository_enabled(
    repo_id: String,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<bool, String> {
    info!("Toggling enabled state for repository: {}", repo_id);

    let mut manager_guard = manager.lock().map_err(|e| {
        error!("Failed to lock workspace manager: {}", e);
        format!("Workspace manager lock error: {}", e)
    })?;

    let workspace_manager = manager_guard.as_mut().ok_or_else(|| {
        warn!("No workspace loaded");
        "No workspace loaded".to_string()
    })?;

    let workspace = workspace_manager.get_workspace_mut();
    let repo = workspace
        .repositories
        .iter_mut()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| {
            warn!("Repository not found: {}", repo_id);
            format!("Repository '{}' not found", repo_id)
        })?;

    repo.enabled = !repo.enabled;
    workspace.updated_at = chrono::Local::now().to_rfc3339();

    info!(
        "Repository '{}' enabled state toggled to: {}",
        repo_id, repo.enabled
    );
    Ok(repo.enabled)
}

/// Get workspace configuration.
#[tauri::command]
pub async fn get_workspace_config() -> Result<WorkspaceConfig, String> {
    Ok(WorkspaceConfig::default())
}

/// Validate workspace file.
#[tauri::command]
pub async fn validate_workspace_file(path: String) -> Result<bool, String> {
    info!("Validating workspace file: {}", path);

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new(workspace_path.clone());

    match storage.validate() {
        Ok(_) => {
            info!("Workspace file validation passed: {}", path);
            Ok(true)
        }
        Err(e) => {
            warn!("Workspace file validation failed for {}: {}", path, e);
            Err(format!("Validation failed: {}", e))
        }
    }
}

/// Create backup of workspace file.
#[tauri::command]
pub async fn backup_workspace(path: String) -> Result<String, String> {
    info!("Creating backup of workspace file: {}", path);

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new(workspace_path.clone());

    let backup_path = storage.backup().map_err(|e| {
        error!("Failed to backup workspace {}: {}", path, e);
        format!("Failed to create backup: {}", e)
    })?;

    let backup_str = backup_path.to_string_lossy().to_string();
    info!("Workspace backup created: {}", backup_str);
    Ok(backup_str)
}

/// Restore workspace from backup.
#[tauri::command]
pub async fn restore_workspace(backup_path: String, workspace_path: String) -> Result<(), String> {
    info!("Restoring workspace from backup: {}", backup_path);

    let backup = PathBuf::from(&backup_path);
    let workspace = PathBuf::from(&workspace_path);
    let storage = WorkspaceStorage::new(workspace.clone());

    storage.restore_from_backup(&backup).map_err(|e| {
            error!("Failed to restore workspace from {}: {}", backup_path, e);
            format!("Failed to restore from backup: {}", e)
        })?;

    info!("Workspace restored from backup: {}", backup_path);
    Ok(())
}
