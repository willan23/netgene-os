//! Agent records shown by the kernel (installed WASM agents and marketplace manifests).

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Unique agent identifier.
pub type AgentId = Uuid;

/// Operational status of an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Processing,
    Waiting,
    Error(String),
    Stopped,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Idle => write!(f, "IDLE"),
            AgentStatus::Processing => write!(f, "PROCESSING"),
            AgentStatus::Waiting => write!(f, "WAITING"),
            AgentStatus::Error(e) => write!(f, "ERROR: {}", e),
            AgentStatus::Stopped => write!(f, "STOPPED"),
        }
    }
}

/// Agent metadata snapshot (for display).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    pub agent_type: String,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
}
