//! Error types for agy-common.

use thiserror::Error;

/// Core error enumeration across agy-suite.
#[derive(Debug, Error)]
pub enum AgyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Process execution error: {0}")]
    Process(String),

    #[error("Admission gate blocked: {0}")]
    GateBlocked(String),
}

/// Convenience alias for `Result<T, AgyError>`.
pub type Result<T> = std::result::Result<T, AgyError>;
