//! Task management commands.

use tauri::State;

use crate::core::tasks::{TaskKind, TaskSnapshot};

use super::super::types::{AppHandle, TaskRegistryState, TauriRuntime};

/// List all tasks in the registry.
#[tauri::command(rename_all = "camelCase")]
pub async fn task_list(reg: State<'_, TaskRegistryState>) -> Result<Vec<TaskSnapshot>, String> {
    Ok(reg.list())
}

/// Get a snapshot of a specific task by ID.
#[tauri::command(rename_all = "camelCase")]
pub async fn task_snapshot(
    id: String,
    reg: State<'_, TaskRegistryState>,
) -> Result<Option<TaskSnapshot>, String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    Ok(reg.snapshot(&uuid))
}

/// Cancel a running task by ID.
#[tauri::command(rename_all = "camelCase")]
pub async fn task_cancel(id: String, reg: State<'_, TaskRegistryState>) -> Result<bool, String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    Ok(reg.cancel(&uuid))
}

/// Start a sleep task for testing purposes.
///
/// Creates a task that sleeps for the specified duration in milliseconds.
#[tauri::command(rename_all = "camelCase")]
pub async fn task_start_sleep(
    ms: u64,
    reg: State<'_, TaskRegistryState>,
    app: tauri::AppHandle<TauriRuntime>,
) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::Sleep { ms });
    reg.clone()
        .spawn_sleep_task(Some(AppHandle::from_tauri(app.clone())), id, token, ms);
    Ok(id.to_string())
}
