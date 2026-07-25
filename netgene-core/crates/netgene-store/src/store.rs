//! Core Sled DB Wrapper for NetGene OS.

use anyhow::{Context, Result};
use sled::{Db, Tree};
use std::path::{Path, PathBuf};
use tracing::info;
use serde::{de::DeserializeOwned, Serialize};

use crate::models::{StoredNode, StoredEvent, AgentMemoryRecord};

pub fn default_db_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".netgene").join("db")
}

#[derive(Clone)]
pub struct NetGeneStore {
    db: Db,
    nodes: Tree,
    events: Tree,
    agent_memory: Tree,
    #[allow(dead_code)]
    config: Tree,
}

impl NetGeneStore {
    /// Opens or creates the Sled database at the specified path or default location.
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let db_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_db_dir);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = sled::open(&db_path)
            .with_context(|| format!("Failed to open sled DB at {:?}", db_path))?;

        let nodes = db.open_tree("nodes")?;
        let events = db.open_tree("events")?;
        let agent_memory = db.open_tree("agent_memory")?;
        let config = db.open_tree("config")?;

        info!("📂 NetGene Store initialized at {:?}", db_path);

        Ok(Self {
            db,
            nodes,
            events,
            agent_memory,
            config,
        })
    }

    pub fn in_memory() -> Result<Self> {
        let db = sled::Config::new().temporary(true).open()?;
        let nodes = db.open_tree("nodes")?;
        let events = db.open_tree("events")?;
        let agent_memory = db.open_tree("agent_memory")?;
        let config = db.open_tree("config")?;

        Ok(Self {
            db,
            nodes,
            events,
            agent_memory,
            config,
        })
    }

    // Helper JSON serialization methods
    fn put_json<V: Serialize>(&self, tree: &Tree, key: &str, val: &V) -> Result<()> {
        let bytes = serde_json::to_vec(val)?;
        tree.insert(key, bytes)?;
        Ok(())
    }

    fn get_json<V: DeserializeOwned>(&self, tree: &Tree, key: &str) -> Result<Option<V>> {
        if let Some(ivec) = tree.get(key)? {
            let val = serde_json::from_slice(&ivec)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    // Node operations
    pub fn save_node(&self, node: &StoredNode) -> Result<()> {
        self.put_json(&self.nodes, &node.id, node)
    }

    pub fn get_node(&self, node_id: &str) -> Result<Option<StoredNode>> {
        self.get_json(&self.nodes, node_id)
    }

    pub fn list_nodes(&self) -> Result<Vec<StoredNode>> {
        let mut list = Vec::new();
        for item in self.nodes.iter() {
            let (_, val) = item?;
            let node: StoredNode = serde_json::from_slice(&val)?;
            list.push(node);
        }
        Ok(list)
    }

    // Event operations
    pub fn save_event(&self, event: &StoredEvent) -> Result<()> {
        self.put_json(&self.events, &event.id.to_string(), event)
    }

    pub fn list_events(&self, limit: usize) -> Result<Vec<StoredEvent>> {
        let mut list = Vec::new();
        for item in self.events.iter().rev().take(limit) {
            let (_, val) = item?;
            let evt: StoredEvent = serde_json::from_slice(&val)?;
            list.push(evt);
        }
        Ok(list)
    }

    // Agent Memory operations
    pub fn set_agent_memory(&self, agent_id: &str, key: &str, value: serde_json::Value) -> Result<()> {
        let record = AgentMemoryRecord {
            agent_id: agent_id.to_string(),
            key: key.to_string(),
            value,
            updated_at: chrono::Utc::now(),
        };
        let tree_key = format!("{}:{}", agent_id, key);
        self.put_json(&self.agent_memory, &tree_key, &record)
    }

    pub fn get_agent_memory(&self, agent_id: &str, key: &str) -> Result<Option<serde_json::Value>> {
        let tree_key = format!("{}:{}", agent_id, key);
        if let Some(record) = self.get_json::<AgentMemoryRecord>(&self.agent_memory, &tree_key)? {
            Ok(Some(record.value))
        } else {
            Ok(None)
        }
    }

    // Flush to disk
    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_store_save_and_list_nodes() -> Result<()> {
        let store = NetGeneStore::in_memory()?;
        let node = StoredNode {
            id: "node-test-1".to_string(),
            name: "Test Node".to_string(),
            template: "edge".to_string(),
            ip: "10.42.0.10".to_string(),
            port: 7010,
            status: "ACTIVE".to_string(),
            last_seen: Utc::now(),
        };

        store.save_node(&node)?;
        let nodes = store.list_nodes()?;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "node-test-1");

        let fetched = store.get_node("node-test-1")?.unwrap();
        assert_eq!(fetched.name, "Test Node");

        Ok(())
    }

    #[test]
    fn test_store_agent_memory() -> Result<()> {
        let store = NetGeneStore::in_memory()?;
        store.set_agent_memory("BuilderAgent", "last_provisioned", serde_json::json!("node-100"))?;

        let val = store.get_agent_memory("BuilderAgent", "last_provisioned")?.unwrap();
        assert_eq!(val, serde_json::json!("node-100"));

        Ok(())
    }
}
