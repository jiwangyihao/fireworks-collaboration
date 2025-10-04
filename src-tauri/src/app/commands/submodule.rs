//! 子模块 Tauri 命令
//!
//! 提供前端可调用的子模块操作命令

use crate::core::submodule::{SubmoduleConfig, SubmoduleInfo, SubmoduleManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;
use tracing::{error, info};

/// 共享子模块管理器状态
pub type SharedSubmoduleManager = Arc<Mutex<SubmoduleManager>>;

/// 子模块操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleCommandResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl SubmoduleCommandResult {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn ok_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

/// 列出仓库中的所有子模块
#[tauri::command]
pub async fn list_submodules(
    repo_path: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<Vec<SubmoduleInfo>, String> {
    info!(target: "submodule::command", "Listing submodules in: {}", repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    let submodules = manager.list_submodules(&repo_path).map_err(|e| {
        error!(target: "submodule::command", "Failed to list submodules: {}", e);
        e.to_string()
    })?;

    info!(target: "submodule::command", "Found {} submodules", submodules.len());
    Ok(submodules)
}

/// 检查仓库是否有子模块
#[tauri::command]
pub async fn has_submodules(
    repo_path: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<bool, String> {
    info!(target: "submodule::command", "Checking if repository has submodules: {}", repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    let has_subs = manager.has_submodules(&repo_path).map_err(|e| {
        error!(target: "submodule::command", "Failed to check submodules: {}", e);
        e.to_string()
    })?;

    info!(target: "submodule::command", "Repository has submodules: {}", has_subs);
    Ok(has_subs)
}

/// 初始化所有子模块
#[tauri::command]
pub async fn init_all_submodules(
    repo_path: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Initializing all submodules in: {}", repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.init_all(&repo_path) {
        Ok(initialized) => {
            let count = initialized.len();
            info!(target: "submodule::command", "Initialized {} submodules", count);
            Ok(SubmoduleCommandResult::ok_with_data(
                format!("Successfully initialized {} submodules", count),
                serde_json::to_value(&initialized).unwrap_or_default(),
            ))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to initialize submodules: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to initialize submodules: {}",
                e
            )))
        }
    }
}

/// 初始化指定子模块
#[tauri::command]
pub async fn init_submodule(
    repo_path: String,
    submodule_name: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Initializing submodule '{}' in: {}", submodule_name, repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.init(&repo_path, &submodule_name) {
        Ok(_) => {
            info!(target: "submodule::command", "Successfully initialized submodule: {}", submodule_name);
            Ok(SubmoduleCommandResult::ok(format!(
                "Successfully initialized submodule '{}'",
                submodule_name
            )))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to initialize submodule: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to initialize submodule '{}': {}",
                submodule_name, e
            )))
        }
    }
}

/// 更新所有子模块
#[tauri::command]
pub async fn update_all_submodules(
    repo_path: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Updating all submodules in: {}", repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.update_all(&repo_path, 0) {
        Ok(updated) => {
            let count = updated.len();
            info!(target: "submodule::command", "Updated {} submodules", count);
            Ok(SubmoduleCommandResult::ok_with_data(
                format!("Successfully updated {} submodules", count),
                serde_json::to_value(&updated).unwrap_or_default(),
            ))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to update submodules: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to update submodules: {}",
                e
            )))
        }
    }
}

/// 更新指定子模块
#[tauri::command]
pub async fn update_submodule(
    repo_path: String,
    submodule_name: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Updating submodule '{}' in: {}", submodule_name, repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.update(&repo_path, &submodule_name) {
        Ok(_) => {
            info!(target: "submodule::command", "Successfully updated submodule: {}", submodule_name);
            Ok(SubmoduleCommandResult::ok(format!(
                "Successfully updated submodule '{}'",
                submodule_name
            )))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to update submodule: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to update submodule '{}': {}",
                submodule_name, e
            )))
        }
    }
}

/// 同步所有子模块的 URL
#[tauri::command]
pub async fn sync_all_submodules(
    repo_path: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Syncing all submodules in: {}", repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.sync_all(&repo_path) {
        Ok(synced) => {
            let count = synced.len();
            info!(target: "submodule::command", "Synced {} submodules", count);
            Ok(SubmoduleCommandResult::ok_with_data(
                format!("Successfully synced {} submodules", count),
                serde_json::to_value(&synced).unwrap_or_default(),
            ))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to sync submodules: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to sync submodules: {}",
                e
            )))
        }
    }
}

/// 同步指定子模块的 URL
#[tauri::command]
pub async fn sync_submodule(
    repo_path: String,
    submodule_name: String,
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleCommandResult, String> {
    info!(target: "submodule::command", "Syncing submodule '{}' in: {}", submodule_name, repo_path);

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    match manager.sync(&repo_path, &submodule_name) {
        Ok(_) => {
            info!(target: "submodule::command", "Successfully synced submodule: {}", submodule_name);
            Ok(SubmoduleCommandResult::ok(format!(
                "Successfully synced submodule '{}'",
                submodule_name
            )))
        }
        Err(e) => {
            error!(target: "submodule::command", "Failed to sync submodule: {}", e);
            Ok(SubmoduleCommandResult::err(format!(
                "Failed to sync submodule '{}': {}",
                submodule_name, e
            )))
        }
    }
}

/// 获取子模块配置
#[tauri::command]
pub async fn get_submodule_config(
    manager: State<'_, SharedSubmoduleManager>,
) -> Result<SubmoduleConfig, String> {
    info!(target: "submodule::command", "Getting submodule configuration");

    let manager = manager.lock().map_err(|e| {
        error!(target: "submodule::command", "Failed to lock manager: {}", e);
        format!("Failed to lock manager: {}", e)
    })?;

    Ok(manager.config().clone())
}
