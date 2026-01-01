//! Configuration management commands.

use std::path::PathBuf;

use tauri::State;

use crate::core::{
    config::{
        loader as cfg_loader,
        model::AppConfig,
        team_template::{
            apply_template_to_config, backup_config_file, export_template, load_template_from_path,
            write_template_to_path, TemplateExportOptions, TemplateImportOptions,
            TemplateImportOutcome, TemplateImportReport,
        },
    },
    ip_pool::{self, config as ip_pool_cfg},
};

use super::super::types::{
    ConfigBaseDir, SharedConfig, SharedIpPool, SharedWorkspaceStatusService,
};

/// Get the current application configuration.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_config(cfg: State<'_, SharedConfig>) -> Result<AppConfig, String> {
    cfg.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

/// Set and save the application configuration.
///
/// This command updates the configuration in memory, saves it to disk,
/// and refreshes the IP pool configuration if needed.
#[tauri::command(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub async fn set_config(
    new_config: AppConfig,
    cfg: State<'_, SharedConfig>,
    base: State<'_, ConfigBaseDir>,
    pool: State<'_, SharedIpPool>,
    status_service: State<'_, SharedWorkspaceStatusService>,
) -> Result<(), String> {
    // Update in-memory configuration
    {
        let mut guard = cfg.lock().map_err(|e| e.to_string())?;
        *guard = new_config.clone();
    }

    // Update workspace status service configuration
    status_service.update_from_config(&new_config.workspace);

    // Save configuration to disk
    cfg_loader::save_at(&new_config, &*base).map_err(|e| e.to_string())?;

    // Refresh IP pool configuration
    match ip_pool::load_effective_config_at(&new_config, base.as_path()) {
        Ok(effective) => {
            if let Ok(mut guard) = pool.inner().lock() {
                guard.update_config(effective);
                tracing::info!(
                    target = "config",
                    "IP pool configuration updated successfully"
                );
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

/// Export the current configuration into a team template file.
#[tauri::command(rename_all = "camelCase")]
pub async fn export_team_config_template(
    destination: Option<String>,
    options: Option<TemplateExportOptions>,
    cfg: State<'_, SharedConfig>,
    base: State<'_, ConfigBaseDir>,
) -> Result<String, String> {
    let snapshot = cfg.lock().map_err(|e| e.to_string())?.clone();
    let export_opts = options.unwrap_or_default();

    let template =
        export_template(&snapshot, base.as_path(), &export_opts).map_err(|e| e.to_string())?;

    let dest_path = destination.map(PathBuf::from).unwrap_or_else(|| {
        let mut path = cfg_loader::config_path_at(base.as_path());
        path.set_file_name("team-config-template.json");
        path
    });

    write_template_to_path(&template, &dest_path).map_err(|e| e.to_string())?;

    Ok(dest_path.to_string_lossy().to_string())
}

/// Import configuration from a team template file.
#[tauri::command(rename_all = "camelCase")]
pub async fn import_team_config_template(
    source: Option<String>,
    options: Option<TemplateImportOptions>,
    cfg: State<'_, SharedConfig>,
    base: State<'_, ConfigBaseDir>,
    pool: State<'_, SharedIpPool>,
) -> Result<TemplateImportReport, String> {
    let template_path = source.map(PathBuf::from).unwrap_or_else(|| {
        let mut path = cfg_loader::config_path_at(base.as_path());
        path.set_file_name("team-config-template.json");
        path
    });

    let template = load_template_from_path(&template_path).map_err(|e| e.to_string())?;
    let import_opts = options.unwrap_or_default();

    let ip_file = ip_pool_cfg::load_or_init_file_at(base.as_path()).map_err(|e| e.to_string())?;

    let mut guard = cfg.lock().map_err(|e| e.to_string())?;
    let TemplateImportOutcome {
        mut report,
        updated_ip_pool_file,
    } = apply_template_to_config(&mut *guard, Some(ip_file), &template, &import_opts)
        .map_err(|e| e.to_string())?;

    let backup_path = backup_config_file(base.as_path()).map_err(|e| e.to_string())?;

    cfg_loader::save_at(&*guard, base.as_path()).map_err(|e| e.to_string())?;

    if let Some(ip_cfg) = updated_ip_pool_file {
        ip_pool_cfg::save_file_at(&ip_cfg, base.as_path()).map_err(|e| e.to_string())?;
    }

    let new_cfg_snapshot = guard.clone();
    drop(guard);

    match ip_pool::load_effective_config_at(&new_cfg_snapshot, base.as_path()) {
        Ok(effective) => {
            if let Ok(mut pool_guard) = pool.inner().lock() {
                pool_guard.update_config(effective);
                tracing::info!(target = "config", "Team template import applied to IP pool");
            } else {
                tracing::error!(
                    target = "ip_pool",
                    "Failed to acquire IP pool lock after template import"
                );
            }
        }
        Err(err) => {
            tracing::error!(
                target = "ip_pool",
                error = %err,
                "Failed to refresh IP pool after template import"
            );
        }
    }

    if let Some(path) = backup_path {
        report.backup_path = Some(path.to_string_lossy().to_string());
    }

    Ok(report)
}

/// Simple greeting command for testing purposes.
#[tauri::command(rename_all = "camelCase")]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Check the version of an installed tool (node, pnpm, git, etc.)
/// This uses lossy UTF-8 decoding to handle encoding issues on Windows.
/// Tries to run the tool directly first (avoiding shell encoding issues),
/// then falls back to cmd.exe if direct execution fails.
#[tauri::command(rename_all = "camelCase")]
pub fn check_tool_version(tool: &str) -> Result<String, String> {
    use std::process::Command;

    // Try running the tool directly first (works if it's in PATH as an .exe)
    let direct_result = Command::new(tool).arg("-v").output();

    let output = if let Ok(out) = direct_result {
        if out.status.success() {
            out
        } else {
            // Direct execution returned an error, try through cmd
            Command::new("cmd.exe")
                .args(["/c", &format!("{} -v", tool)])
                .output()
                .map_err(|e| format!("Failed to execute {}: {}", tool, e))?
        }
    } else {
        // Direct execution failed (not in PATH or not an exe), try through cmd
        Command::new("cmd.exe")
            .args(["/c", &format!("{} -v", tool)])
            .output()
            .map_err(|e| format!("Failed to execute {}: {}", tool, e))?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg: String = format!("{}{}", stderr.trim(), stdout.trim())
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            .collect();
        return Err(format!("Command failed: {}", error_msg));
    }

    // Use lossy conversion to handle non-UTF-8 bytes
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Filter to ASCII printable characters only
    let version: String = stdout
        .chars()
        .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect();

    Ok(version.trim().to_string())
}
