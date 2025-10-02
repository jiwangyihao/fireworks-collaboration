//! Application setup and initialization.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use dirs_next as dirs;
use tauri::Manager;

use crate::{
    core::{
        config::{loader as cfg_loader, model::AppConfig},
        ip_pool,
        tasks::TaskRegistry,
    },
    logging,
};

use super::types::{ConfigBaseDir, OAuthState, SharedConfig, SharedIpPool, TaskRegistryState};

/// Initialize and run the Tauri application.
///
/// This function sets up logging, configures plugins, registers command handlers,
/// and initializes application state including configuration, task registry, and IP pool.
pub fn run() {
    // Initialize logging system
    logging::init_logging();
    
    let mut builder = tauri::Builder::default()
        // Register Tauri plugins
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        // Initialize managed state
        .manage(OAuthState::new(Mutex::new(None)))
        .manage(Arc::new(TaskRegistry::new()) as TaskRegistryState)
        .manage(ip_pool::global::obtain_global_pool())
        // Register command handlers
        .invoke_handler(tauri::generate_handler![
            super::commands::greet,
            super::commands::start_oauth_server,
            super::commands::get_oauth_callback_data,
            super::commands::clear_oauth_state,
            super::commands::get_system_proxy,
            super::commands::get_config,
            super::commands::set_config,
            super::commands::task_list,
            super::commands::task_cancel,
            super::commands::task_start_sleep,
            super::commands::task_snapshot,
            super::commands::git_clone,
            super::commands::git_fetch,
            super::commands::git_push,
            super::commands::git_init,
            super::commands::git_add,
            super::commands::git_commit,
            super::commands::git_branch,
            super::commands::git_checkout,
            super::commands::git_tag,
            super::commands::git_remote_set,
            super::commands::git_remote_add,
            super::commands::git_remote_remove,
            super::commands::http_fake_request,
            super::commands::detect_system_proxy,
            super::commands::force_proxy_fallback,
            super::commands::force_proxy_recovery
        ]);
    
    // Setup application state and configuration
    builder = builder.setup(|app| {
        setup_app_state(app)?;
        Ok(())
    });
    
    // Run the application
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Setup application state including configuration and IP pool.
fn setup_app_state(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Determine configuration base directory
    let base_dir: PathBuf = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| get_fallback_config_dir());
    
    tracing::info!(
        target = "app",
        path = %base_dir.display(),
        "Using configuration directory"
    );
    
    // Set global configuration base directory for dynamic loading
    cfg_loader::set_global_base_dir(&base_dir);
    
    // Load or initialize configuration
    let cfg = cfg_loader::load_or_init_at(&base_dir).unwrap_or_else(|e| {
        tracing::warn!(
            target = "app",
            error = %e,
            "Failed to load configuration, using defaults"
        );
        AppConfig::default()
    });
    
    // Manage configuration state
    app.manage(Arc::new(Mutex::new(cfg.clone())) as SharedConfig);
    
    let base_dir_clone = base_dir.clone();
    app.manage::<ConfigBaseDir>(base_dir);
    
    // Load and configure IP pool
    let effective = match ip_pool::load_effective_config_at(&cfg, &base_dir_clone) {
        Ok(cfg) => {
            tracing::info!(target = "ip_pool", "IP pool configuration loaded successfully");
            cfg
        }
        Err(err) => {
            tracing::error!(
                target = "ip_pool",
                error = %err,
                "Failed to load IP pool config; using defaults"
            );
            ip_pool::EffectiveIpPoolConfig::from_parts(
                cfg.ip_pool.clone(),
                ip_pool::IpPoolFileConfig::default(),
            )
        }
    };
    
    // Update IP pool with effective configuration
    let pool_state = app.state::<SharedIpPool>();
    if let Ok(mut guard) = pool_state.inner().lock() {
        guard.update_config(effective);
        tracing::info!(target = "ip_pool", "IP pool initialized successfully");
    } else {
        tracing::error!(target = "ip_pool", "Failed to acquire IP pool lock during setup");
    }
    
    Ok(())
}

/// Get fallback configuration directory.
///
/// This is used when the standard app config directory cannot be determined.
fn get_fallback_config_dir() -> PathBuf {
    let identifier = "top.jwyihao.fireworks-collaboration";
    
    if let Some(mut dir) = dirs::config_dir() {
        dir.push(identifier);
        dir
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}
