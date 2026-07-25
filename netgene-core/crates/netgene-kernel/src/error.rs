//! Kernel error types.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("Agent not found: {id}")]
    AgentNotFound { id: String },
    #[error("Bus send error")]
    BusSend,
    #[error("Intent parse error: {0}")]
    IntentParse(String),
}
