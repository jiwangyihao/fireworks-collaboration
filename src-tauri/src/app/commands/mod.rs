//! Commands module - Re-exports all Tauri command handlers.

pub mod config;
pub mod credential;
pub mod git;
pub mod http;
pub mod oauth;
pub mod proxy;
pub mod tasks;

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
pub use tasks::{task_cancel, task_list, task_snapshot, task_start_sleep};
