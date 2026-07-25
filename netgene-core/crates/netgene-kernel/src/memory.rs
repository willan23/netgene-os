//! Kernel memory — vector store + event log.
//!
//! Phase 1: Simple in-memory HashMap store.
//! Phase 2: Replace with `petgraph` graph DB + real embedding vectors.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub kind: String,
    pub content: serde_json::Value,
    pub tags: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// Simple in-memory store for the kernel.
#[derive(Default)]
pub struct KernelMemory {
    entries: HashMap<Uuid, MemoryEntry>,
    event_log: Vec<String>,
}

impl KernelMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a new memory entry.
    pub fn store(&mut self, kind: &str, content: serde_json::Value, tags: Vec<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.entries.insert(id, MemoryEntry {
            id,
            kind: kind.to_string(),
            content,
            tags,
            timestamp: Utc::now(),
        });
        id
    }

    /// Retrieve an entry by ID.
    pub fn get(&self, id: &Uuid) -> Option<&MemoryEntry> {
        self.entries.get(id)
    }

    /// Search entries by tag.
    pub fn search_by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        self.entries.values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Search entries by kind.
    pub fn search_by_kind(&self, kind: &str) -> Vec<&MemoryEntry> {
        self.entries.values()
            .filter(|e| e.kind == kind)
            .collect()
    }

    /// Append to the event log.
    pub fn log_event(&mut self, event: String) {
        self.event_log.push(format!("[{}] {}", Utc::now().format("%H:%M:%S"), event));
    }

    /// Get the last N events.
    pub fn recent_events(&self, n: usize) -> Vec<&String> {
        self.event_log.iter().rev().take(n).collect()
    }

    /// Total number of stored entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}
