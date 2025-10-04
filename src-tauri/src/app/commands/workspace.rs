//! Workspace management commands.
//!
//! This module provides Tauri command handlers for workspace operations,
//! including create, load, save, add/remove repositories, and status queries.

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, warn};

use crate::core::workspace::{
    model::{RepositoryEntry, Workspace, WorkspaceConfig},
    storage::WorkspaceStorage,
};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Shared workspace state (just holds the current workspace).
pub type SharedWorkspaceManager = Arc<Mutex<Option<Workspace>>>;

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
            root_path: ws.root_path.clone(),
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
            path: repo.path.clone(),
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

/// Create a new workspace.
#[tauri::command]
pub async fn create_workspace(
    request: CreateWorkspaceRequest,
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<WorkspaceInfo, String> {
    info!("Creating workspace: {}", request.name);

    let mut ws = Workspace::new(
        request.name.clone(),
        PathBuf::from(&request.root_path),
    );

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

    storage
        .save(workspace)
        .map_err(|e| {
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
pub async fn close_workspace(
    manager: State<'_, SharedWorkspaceManager>,
) -> Result<(), String> {
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

    let repo = RepositoryEntry {
        id: request.id.clone(),
        name: request.name.clone(),
        path: request.path,
        remote_url: request.remote_url,
        tags: request.tags.unwrap_or_default(),
        enabled: request.enabled.unwrap_or(true),
    };

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
pub async fn validate_workspace_file(
    path: String,
) -> Result<bool, String> {
    info!("Validating workspace file: {}", path);

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new();

    match storage.validate(&workspace_path) {
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
pub async fn backup_workspace(
    path: String,
) -> Result<String, String> {
    info!("Creating backup of workspace file: {}", path);

    let workspace_path = PathBuf::from(&path);
    let storage = WorkspaceStorage::new();

    let backup_path = storage.backup(&workspace_path).map_err(|e| {
        error!("Failed to backup workspace {}: {}", path, e);
        format!("Failed to create backup: {}", e)
    })?;

    let backup_str = backup_path.to_string_lossy().to_string();
    info!("Workspace backup created: {}", backup_str);
    Ok(backup_str)
}

/// Restore workspace from backup.
#[tauri::command]
pub async fn restore_workspace(
    backup_path: String,
    workspace_path: String,
) -> Result<(), String> {
    info!("Restoring workspace from backup: {}", backup_path);

    let backup = PathBuf::from(&backup_path);
    let workspace = PathBuf::from(&workspace_path);
    let storage = WorkspaceStorage::new();

    storage
        .restore_from_backup(&backup, &workspace)
        .map_err(|e| {
            error!("Failed to restore workspace from {}: {}", backup_path, e);
            format!("Failed to restore from backup: {}", e)
        })?;

    info!("Workspace restored from backup: {}", backup_path);
    Ok(())
}
