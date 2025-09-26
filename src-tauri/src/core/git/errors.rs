use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Network,
    Tls,
    Verify,
    Protocol,
    Proxy,
    Auth,
    Cancel,
    Internal,
}

#[derive(Error, Debug)]
pub enum GitError {
    #[error("{category:?}: {message}")]
    Categorized {
        category: ErrorCategory,
        message: String,
    },
}

impl GitError {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        GitError::Categorized {
            category,
            message: message.into(),
        }
    }
}
