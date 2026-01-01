//! Application setup and initialization.

// Imports only needed for the full app mode (with wry/WebView2) or tests
#[cfg(any(feature = "tauri-app", test))]
use std::path::PathBuf;

#[cfg(feature = "tauri-app")]
use std::sync::{Arc, Mutex};

#[cfg(any(feature = "tauri-app", test))]
use dirs_next as dirs;
#[cfg(feature = "tauri-app")]
use tauri::Manager;

#[cfg(feature = "tauri-app")]
use crate::{
    core::{
        config::{loader as cfg_loader, model::AppConfig},
        credential::audit::AuditLogger,
        ip_pool,
        tasks::TaskRegistry,
        workspace::WorkspaceStatusService,
    },
    logging,
};

#[cfg(feature = "tauri-app")]
use super::{
    commands::credential::initialize_credential_store,
    types::{
        ConfigBaseDir, OAuthState, SharedAuditLogger, SharedConfig, SharedCredentialFactory,
        SharedIpPool, SharedSubmoduleManager, SharedWorkspaceManager, SharedWorkspaceStatusService,
        TaskRegistryState,
    },
};

/// Initialize and run the Tauri application.
///
/// This function sets up logging, configures plugins, registers command handlers,
/// and initializes application state including configuration, task registry, and IP pool.
///
/// Note: This function requires the `tauri-app` feature (which includes wry/WebView2).
/// For tests without UI, use `tauri-core` feature instead.
#[cfg(feature = "tauri-app")]
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
            crate::app::commands::config::greet,
            crate::app::commands::oauth::start_oauth_server,
            crate::app::commands::oauth::get_oauth_callback_data,
            crate::app::commands::oauth::clear_oauth_state,
            crate::app::commands::proxy::get_system_proxy,
            crate::app::commands::config::get_config,
            crate::app::commands::config::set_config,
            crate::app::commands::config::export_team_config_template,
            crate::app::commands::config::import_team_config_template,
            crate::app::commands::config::check_tool_version,
            crate::app::commands::ip_pool::ip_pool_get_snapshot,
            crate::app::commands::ip_pool::ip_pool_update_config,
            crate::app::commands::ip_pool::ip_pool_request_refresh,
            crate::app::commands::ip_pool::ip_pool_start_preheater,
            crate::app::commands::ip_pool::ip_pool_clear_auto_disabled,
            crate::app::commands::ip_pool::ip_pool_pick_best,
            crate::app::commands::tasks::task_list,
            crate::app::commands::tasks::task_cancel,
            crate::app::commands::tasks::task_start_sleep,
            crate::app::commands::tasks::task_snapshot,
            crate::app::commands::git::git_clone,
            crate::app::commands::git::git_fetch,
            crate::app::commands::git::git_push,
            crate::app::commands::git::git_init,
            crate::app::commands::git::git_add,
            crate::app::commands::git::git_commit,
            crate::app::commands::git::git_branch,
            crate::app::commands::git::git_checkout,
            crate::app::commands::git::git_tag,
            crate::app::commands::git::git_remote_set,
            crate::app::commands::git::git_remote_add,
            crate::app::commands::git::git_remote_remove,
            crate::app::commands::git::git_list_branches,
            crate::app::commands::git::git_repo_status,
            crate::app::commands::git::git_delete_branch,
            crate::app::commands::git::git_remote_branches,
            crate::app::commands::git::git_worktree_list,
            crate::app::commands::git::git_worktree_add,
            crate::app::commands::git::git_worktree_remove,
            crate::app::commands::http::http_fake_request,
            crate::app::commands::metrics::metrics_snapshot,
            crate::app::commands::proxy::detect_system_proxy,
            crate::app::commands::proxy::force_proxy_fallback,
            crate::app::commands::proxy::force_proxy_recovery,
            crate::app::commands::credential::add_credential,
            crate::app::commands::credential::get_credential,
            crate::app::commands::credential::update_credential,
            crate::app::commands::credential::delete_credential,
            crate::app::commands::credential::list_credentials,
            crate::app::commands::credential::set_master_password,
            crate::app::commands::credential::unlock_store,
            crate::app::commands::credential::export_audit_log,
            crate::app::commands::credential::cleanup_expired_credentials,
            crate::app::commands::credential::cleanup_audit_logs,
            crate::app::commands::credential::is_credential_locked,
            crate::app::commands::credential::reset_credential_lock,
            crate::app::commands::credential::remaining_auth_attempts,
            crate::app::commands::workspace::create_workspace,
            crate::app::commands::workspace::load_workspace,
            crate::app::commands::workspace::save_workspace,
            crate::app::commands::workspace::get_workspace,
            crate::app::commands::workspace::close_workspace,
            crate::app::commands::workspace::add_repository,
            crate::app::commands::workspace::remove_repository,
            crate::app::commands::workspace::get_repository,
            crate::app::commands::workspace::list_repositories,
            crate::app::commands::workspace::list_enabled_repositories,
            crate::app::commands::workspace::reorder_repositories,
            crate::app::commands::workspace::get_workspace_statuses,
            crate::app::commands::workspace::clear_workspace_status_cache,
            crate::app::commands::workspace::invalidate_workspace_status_entry,
            crate::app::commands::workspace::update_repository_tags,
            crate::app::commands::workspace::toggle_repository_enabled,
            crate::app::commands::workspace::get_workspace_config,
            crate::app::commands::workspace::validate_workspace_file,
            crate::app::commands::workspace::backup_workspace,
            crate::app::commands::workspace::restore_workspace,
            crate::app::commands::workspace::workspace_batch_clone,
            crate::app::commands::workspace::workspace_batch_fetch,
            crate::app::commands::workspace::workspace_batch_push,
            crate::app::commands::submodule::list_submodules,
            crate::app::commands::submodule::has_submodules,
            crate::app::commands::submodule::init_all_submodules,
            crate::app::commands::submodule::init_submodule,
            crate::app::commands::submodule::update_all_submodules,
            crate::app::commands::submodule::update_submodule,
            crate::app::commands::submodule::sync_all_submodules,
            crate::app::commands::submodule::sync_submodule,
            crate::app::commands::submodule::get_submodule_config,
            // VitePress 命令
            crate::app::commands::vitepress::vitepress_detect_project,
            crate::app::commands::vitepress::vitepress_check_dependencies,
            crate::app::commands::vitepress::vitepress_get_doc_tree,
            crate::app::commands::vitepress::vitepress_read_document,
            crate::app::commands::vitepress::vitepress_save_document,
            crate::app::commands::vitepress::vitepress_create_document,
            crate::app::commands::vitepress::vitepress_create_folder,
            crate::app::commands::vitepress::vitepress_rename,
            crate::app::commands::vitepress::vitepress_delete,
            crate::app::commands::vitepress::vitepress_install_dependencies,
            crate::app::commands::vitepress::vitepress_start_dev_server,
            crate::app::commands::vitepress::vitepress_stop_dev_server,
            crate::app::commands::vitepress::vitepress_parse_config
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
#[cfg(feature = "tauri-app")]
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

    if let Err(err) = crate::core::metrics::init_basic_observability(&cfg.observability) {
        tracing::warn!(target = "metrics", error = %err, "failed to initialize basic observability metrics");
    }
    if let Err(err) = crate::core::metrics::init_aggregate_observability(&cfg.observability) {
        tracing::warn!(target = "metrics", error = %err, "failed to initialize aggregate observability metrics");
    }
    // NOTE: 暂时跳过 metrics export HTTP server 初始化（需要 Tokio runtime 环境），后续可在首个相关命令调用时延迟启动。

    // Manage configuration state
    app.manage(Arc::new(Mutex::new(cfg.clone())) as SharedConfig);

    let base_dir_clone = base_dir.clone();
    app.manage::<ConfigBaseDir>(base_dir);

    // Load and configure IP pool
    let effective = match ip_pool::load_effective_config_at(&cfg, &base_dir_clone) {
        Ok(cfg) => {
            tracing::info!(
                target = "ip_pool",
                "IP pool configuration loaded successfully"
            );
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
        tracing::error!(
            target = "ip_pool",
            "Failed to acquire IP pool lock during setup"
        );
    }

    // Initialize credential store
    let cred_config = cfg.credential.clone();
    let cred_store = match initialize_credential_store(&cred_config) {
        Ok(store) => {
            tracing::info!(
                target = "credential",
                storage_type = ?cred_config.storage,
                "Credential store initialized successfully"
            );
            Some(store)
        }
        Err(err) => {
            tracing::warn!(
                target = "credential",
                error = %err,
                "Failed to initialize credential store, attempting fallback to memory"
            );

            // Fallback to memory
            let mem_config = cred_config
                .clone()
                .with_storage(crate::core::credential::config::StorageType::Memory);
            match initialize_credential_store(&mem_config) {
                Ok(store) => {
                    tracing::info!(target = "credential", "Fallback to memory store successful");
                    Some(store)
                }
                Err(e) => {
                    tracing::error!(target="credential", error=%e, "Memory store fallback failed");
                    None
                }
            }
        }
    };

    // Manage credential factory state
    app.manage(Arc::new(Mutex::new(cred_store)) as SharedCredentialFactory);

    // Initialize audit logger
    let audit_mode = cred_config.audit_mode;
    let audit_logger = AuditLogger::new(audit_mode);
    app.manage(Arc::new(Mutex::new(audit_logger)) as SharedAuditLogger);

    tracing::info!(
        target = "credential",
        audit_mode = audit_mode,
        "Audit logger initialized"
    );

    // Initialize workspace manager (initially empty)
    app.manage(Arc::new(Mutex::new(None)) as SharedWorkspaceManager);
    tracing::info!(
        target = "workspace",
        "Workspace manager initialized (no workspace loaded)"
    );

    // Initialize workspace status service with current configuration
    let workspace_status_service = WorkspaceStatusService::new(&cfg.workspace);
    app.manage(Arc::new(workspace_status_service) as SharedWorkspaceStatusService);
    tracing::info!(target = "workspace", "Workspace status service initialized");

    // Initialize Git Runner (P26.1)
    let git_runner = crate::core::git::runner::Git2Runner::new();
    let runner_box =
        Box::new(git_runner.clone()) as Box<dyn crate::core::git::runner::GitRunner + Send + Sync>;
    app.manage(Box::new(git_runner) as Box<dyn crate::core::git::runner::GitRunner>);
    tracing::info!(target = "git", "Git runner initialized");

    // Initialize submodule manager with default config
    let submodule_config = cfg.submodule.clone();
    let submodule_manager =
        crate::core::submodule::SubmoduleManager::new(submodule_config, runner_box);
    app.manage(Arc::new(Mutex::new(submodule_manager)) as SharedSubmoduleManager);
    tracing::info!(
        target = "submodule",
        "Submodule manager initialized with default config"
    );

    // Initialize VitePress Dev Server State
    app.manage(crate::app::commands::vitepress::DevServerState::default());
    tracing::info!(target = "vitepress", "DevServerState initialized");

    Ok(())
}

/// Get fallback configuration directory.
///
/// This is used when the standard app config directory cannot be determined.
#[cfg(any(feature = "tauri-app", test))]
fn get_fallback_config_dir() -> PathBuf {
    let identifier = "top.jwyihao.fireworks-collaboration";

    if let Some(mut dir) = dirs::config_dir() {
        dir.push(identifier);
        dir
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_fallback_config_dir() {
        let dir = get_fallback_config_dir();
        // Just verify it returns something reasonable (not empty)
        assert!(dir.is_absolute() || dir.exists() || !dir.as_os_str().is_empty());

        // It should end with the identifier or be current dir
        let path_str = dir.to_string_lossy();
        if path_str.contains("top.jwyihao") {
            assert!(path_str.ends_with("top.jwyihao.fireworks-collaboration"));
        } else {
            // If dirs::config_dir() failed (unlikely on dev machine but possible in CI),
            // it falls back to current dir.
            assert!(!path_str.is_empty());
        }
    }
}
