//! Data models and records stored within NetGene Store.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Generic record wrapper with vector clock and timestamp for CRDT resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record<T> {
    pub id: String,
    pub payload: T,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub version: u64,
}

impl<T> Record<T> {
    pub fn new(id: impl Into<String>, payload: T, node_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            payload,
            timestamp: Utc::now(),
            node_id: node_id.into(),
            version: 1,
        }
    }
}

/// Persistent record for a network node state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredNode {
    pub id: String,
    pub name: String,
    pub template: String,
    pub ip: String,
    pub port: u16,
    pub status: String,
    pub last_seen: DateTime<Utc>,
}

/// System telemetry event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub event_type: String,
    pub severity: String,
    pub details: String,
}

/// Agent long-term memory key-value entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemoryRecord {
    pub agent_id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}
