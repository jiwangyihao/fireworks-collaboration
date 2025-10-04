//! Commands module - Re-exports all Tauri command handlers.

pub mod config;
pub mod credential;
pub mod git;
pub mod http;
pub mod oauth;
pub mod proxy;
pub mod submodule;
pub mod tasks;
pub mod workspace;

// Re-export all command functions
pub use config::{get_config, greet, set_config};
pub use credential::{
    add_credential, delete_credential, export_audit_log, get_credential, list_credentials,
    set_master_password, unlock_store, update_credential, SharedAuditLogger,
    SharedCredentialFactory,
};
pub use git::{
    git_add, git_branch, git_checkout, git_clone, git_commit, git_fetch, git_init, git_push,
    git_remote_add, git_remote_remove, git_remote_set, git_tag,
};
pub use http::http_fake_request;
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
    add_repository, backup_workspace, close_workspace, create_workspace, get_repository,
    get_workspace, get_workspace_config, list_enabled_repositories, list_repositories,
    load_workspace, remove_repository, restore_workspace, save_workspace,
    toggle_repository_enabled, update_repository_tags, validate_workspace_file,
    SharedWorkspaceManager,
};
