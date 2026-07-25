//! Error types for the Gene Layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeneError {
    #[error("Parent gene is not active — cannot spawn sub-genes")]
    ParentNotActive,

    #[error("Gene not found: {id}")]
    NotFound { id: String },

    #[error("Gene is revoked and cannot be used")]
    Revoked,

    #[error("Insufficient capability: required '{required}'")]
    InsufficientCapability { required: String },

    #[error("Token verification failed: {reason}")]
    TokenVerificationFailed { reason: String },

    #[error("Storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
