pub mod service;
pub mod errors;
pub mod default_impl;
pub mod transport;

// 对外提供中性命名的默认实现，避免外部到具体模块路径
pub use default_impl::DefaultGitService;
pub use errors::{GitError, ErrorCategory};
pub use service::{GitService, ProgressPayload};
