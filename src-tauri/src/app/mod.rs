//! Tauri application module.
//!
//! This module contains all the code for the Tauri desktop application,
//! including command handlers, state management, and application setup.
//!
//! ## Module Structure
//!
//! - `types`: Shared type definitions and state management
//! - `commands`: All Tauri command handlers
//!   - `config`: Configuration management commands
//!   - `oauth`: OAuth server and authentication commands
//!   - `tasks`: Task management commands
//!   - `git`: Git operation commands
//!   - `proxy`: Proxy detection and management commands
//!   - `http`: HTTP request commands
//! - `setup`: Application initialization and setup
//!
//! ## Usage
//!
//! The main entry point is the `run` function, which initializes and starts
//! the Tauri application:
//!
//! ```no_run
//! use fireworks_collaboration_lib::app;
//! app::run();
//! ```

#![cfg(feature = "tauri-app")]

pub mod commands;
pub mod setup;
pub mod types;

// Re-export the main run function
pub use setup::run;

// Re-export commonly used types
pub use types::{
    ConfigBaseDir, OAuthCallbackData, OAuthState, SharedConfig, SharedIpPool, SharedProxyManager,
    SystemProxy, SystemProxyResult, TaskRegistryState,
};
