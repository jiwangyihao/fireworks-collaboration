//! Commands module - Re-exports all Tauri command handlers.

pub mod config;
pub mod credential;
pub mod git;
pub mod http;
pub mod ip_pool;
pub mod metrics;
pub mod oauth;
pub mod proxy;
pub mod submodule;
pub mod tasks;
pub mod workspace;

// Re-export all command functions
pub use config::{
    check_tool_version, export_team_config_template, get_config, greet,
    import_team_config_template, set_config,
};
pub use credential::{
    add_credential, delete_credential, export_audit_log, get_credential, list_credentials,
    set_master_password, unlock_store, update_credential, SharedAuditLogger,
    SharedCredentialFactory,
};
pub use git::{
    git_add, git_branch, git_checkout, git_clone, git_commit, git_delete_branch, git_fetch,
    git_init, git_list_branches, git_push, git_remote_add, git_remote_branches, git_remote_remove,
    git_remote_set, git_repo_status, git_tag, git_worktree_add, git_worktree_list,
    git_worktree_remove,
};
pub use http::http_fake_request;
pub use ip_pool::{
    ip_pool_clear_auto_disabled, ip_pool_get_snapshot, ip_pool_pick_best, ip_pool_request_refresh,
    ip_pool_update_config,
};
pub use metrics::metrics_snapshot;
pub use oauth::{clear_oauth_state, get_oauth_callback_data, start_oauth_server};
pub use proxy::{
    detect_system_proxy, force_proxy_fallback, force_proxy_recovery, get_system_proxy,
};
pub use submodule::{
    get_submodule_config, has_submodules, init_all_submodules, init_submodule, list_submodules,
    sync_all_submodules, sync_submodule, update_all_submodules, update_submodule,
    SharedSubmoduleManager,
};
pub use tasks::{task_cancel, task_list, task_snapshot, task_start_sleep};
pub use workspace::{
    add_repository, backup_workspace, clear_workspace_status_cache, close_workspace,
    create_workspace, get_repository, get_workspace, get_workspace_config, get_workspace_statuses,
    invalidate_workspace_status_entry, list_enabled_repositories, list_repositories,
    load_workspace, remove_repository, reorder_repositories, restore_workspace, save_workspace,
    toggle_repository_enabled, update_repository_tags, validate_workspace_file,
    workspace_batch_clone, workspace_batch_fetch, workspace_batch_push, SharedWorkspaceManager,
    SharedWorkspaceStatusService,
};
