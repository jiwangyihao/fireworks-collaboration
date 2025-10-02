//! Configuration management commands.

use tauri::State;

use crate::core::{
    config::{loader as cfg_loader, model::AppConfig},
    ip_pool,
};

use super::super::types::{ConfigBaseDir, SharedConfig, SharedIpPool};

/// Get the current application configuration.
#[tauri::command]
pub async fn get_config(cfg: State<'_, SharedConfig>) -> Result<AppConfig, String> {
    cfg.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

/// Set and save the application configuration.
///
/// This command updates the configuration in memory, saves it to disk,
/// and refreshes the IP pool configuration if needed.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_config(
    newCfg: AppConfig,
    cfg: State<'_, SharedConfig>,
    base: State<'_, ConfigBaseDir>,
    pool: State<'_, SharedIpPool>,
) -> Result<(), String> {
    // Update in-memory configuration
    {
        let mut guard = cfg.lock().map_err(|e| e.to_string())?;
        *guard = newCfg.clone();
    }

    // Save configuration to disk
    cfg_loader::save_at(&newCfg, &*base).map_err(|e| e.to_string())?;

    // Refresh IP pool configuration
    match ip_pool::load_effective_config_at(&newCfg, base.as_path()) {
        Ok(effective) => {
            if let Ok(mut guard) = pool.inner().lock() {
                guard.update_config(effective);
                tracing::info!(target = "config", "IP pool configuration updated successfully");
            } else {
                tracing::error!(
                    target = "ip_pool",
                    "Failed to acquire IP pool lock while applying config"
                );
            }
        }
        Err(err) => {
            tracing::error!(
                target = "ip_pool",
                error = %err,
                "Failed to refresh IP pool configuration"
            );
        }
    }

    Ok(())
}

/// Simple greeting command for testing purposes.
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
