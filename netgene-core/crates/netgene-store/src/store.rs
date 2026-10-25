//! redb-backed store for NetGene OS (single-file, ACID, pure Rust).
//!
//! Replaces the unmaintained `sled` backend. A sled database left by an
//! older version (`~/.netgene/db/`) is migrated once into
//! `~/.netgene/store.redb` and then renamed to `db.sled-migrated`.

use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::{Path, PathBuf};
use tracing::info;
use serde::{de::DeserializeOwned, Serialize};

use crate::models::{StoredNode, StoredEvent, AgentMemoryRecord};

const NODES: TableDefinition<&str, &[u8]> = TableDefinition::new("nodes");
const EVENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("events");
const AGENT_MEMORY: TableDefinition<&str, &[u8]> = TableDefinition::new("agent_memory");
const CONFIG: TableDefinition<&str, &[u8]> = TableDefinition::new("config");
#[cfg(feature = "sled-migration")]
const SLED_MIGRATION_MARKER: &str = "migration:sled";

fn netgene_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".netgene")
}

/// Default database file: `~/.netgene/store.redb`.
pub fn default_db_path() -> PathBuf {
    netgene_home().join("store.redb")
}

/// Directory used by the sled backend of NetGene ≤ 0.6.
pub fn legacy_sled_dir() -> PathBuf {
    netgene_home().join("db")
}

/// Events are keyed by time so iteration order is chronological.
fn event_key(event: &StoredEvent) -> String {
    let nanos = event.timestamp.timestamp_nanos_opt().unwrap_or(0).max(0);
    format!("{:020}-{}", nanos, event.id)
}

#[derive(Clone)]
pub struct NetGeneStore {
    db: std::sync::Arc<Database>,
}

impl NetGeneStore {
    /// Open or create the database at `path` (default: `~/.netgene/store.redb`),
    /// migrating a legacy sled database when opening the default location.
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let db_path = path.map(|p| p.to_path_buf()).unwrap_or_else(default_db_path);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(&db_path).with_context(|| format!("Failed to open redb at {:?}", db_path))?;
        let store = Self::init(db)?;

        // The import and its marker commit together, so a failed migration is retried next time.
        #[cfg(feature = "sled-migration")]
        if path.is_none() {
            let legacy = legacy_sled_dir();
            if legacy.is_dir() && store.get_json::<String>(CONFIG, SLED_MIGRATION_MARKER)?.is_none() {
                let migrated = store.import_sled(&legacy)?;
                let backup = legacy.with_file_name("db.sled-migrated");
                std::fs::rename(&legacy, &backup)?;
                info!("📦 Migrated {} records from sled ({:?}) into redb; old data kept at {:?}", migrated, legacy, backup);
            }
        }

        info!("📂 NetGene Store initialized at {:?}", db_path);
        Ok(store)
    }

    /// Store kept in memory only (tests).
    pub fn in_memory() -> Result<Self> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
        Self::init(db)
    }

    fn init(db: Database) -> Result<Self> {
        // Create every table up front so read transactions never hit a missing table.
        let tx = db.begin_write()?;
        for table in [NODES, EVENTS, AGENT_MEMORY, CONFIG] {
            tx.open_table(table)?;
        }
        tx.commit()?;
        Ok(Self { db: std::sync::Arc::new(db) })
    }

    fn put_json<V: Serialize>(&self, table: TableDefinition<&str, &[u8]>, key: &str, val: &V) -> Result<()> {
        let bytes = serde_json::to_vec(val)?;
        let tx = self.db.begin_write()?;
        tx.open_table(table)?.insert(key, bytes.as_slice())?;
        tx.commit()?;
        Ok(())
    }

    fn get_json<V: DeserializeOwned>(&self, table: TableDefinition<&str, &[u8]>, key: &str) -> Result<Option<V>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(table)?;
        match table.get(key)? {
            Some(guard) => Ok(Some(serde_json::from_slice(guard.value())?)),
            None => Ok(None),
        }
    }

    // Node operations
    pub fn save_node(&self, node: &StoredNode) -> Result<()> {
        self.put_json(NODES, &node.id, node)
    }

    pub fn get_node(&self, node_id: &str) -> Result<Option<StoredNode>> {
        self.get_json(NODES, node_id)
    }

    pub fn list_nodes(&self) -> Result<Vec<StoredNode>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(NODES)?;
        let mut list = Vec::new();
        for item in table.iter()? {
            let (_, val) = item?;
            list.push(serde_json::from_slice(val.value())?);
        }
        Ok(list)
    }

    // Event operations
    pub fn save_event(&self, event: &StoredEvent) -> Result<()> {
        self.put_json(EVENTS, &event_key(event), event)
    }

    /// The `limit` most recent events, newest first.
    pub fn list_events(&self, limit: usize) -> Result<Vec<StoredEvent>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(EVENTS)?;
        let mut list = Vec::new();
        for item in table.iter()?.rev().take(limit) {
            let (_, val) = item?;
            list.push(serde_json::from_slice(val.value())?);
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
        self.put_json(AGENT_MEMORY, &format!("{}:{}", agent_id, key), &record)
    }

    pub fn get_agent_memory(&self, agent_id: &str, key: &str) -> Result<Option<serde_json::Value>> {
        Ok(self
            .get_json::<AgentMemoryRecord>(AGENT_MEMORY, &format!("{}:{}", agent_id, key))?
            .map(|record| record.value))
    }

    /// Writes are durable at commit; kept for API compatibility.
    pub fn flush(&self) -> Result<()> {
        Ok(())
    }

    /// Copy every record of a legacy sled database into this store.
    #[cfg(feature = "sled-migration")]
    pub fn import_sled(&self, sled_dir: &Path) -> Result<usize> {
        let legacy = sled::open(sled_dir).with_context(|| format!("Failed to open legacy sled DB at {:?}", sled_dir))?;
        let mut count = 0;
        let tx = self.db.begin_write()?;
        {
            let mut nodes = tx.open_table(NODES)?;
            for item in legacy.open_tree("nodes")?.iter() {
                let (k, v) = item?;
                nodes.insert(std::str::from_utf8(&k)?, v.as_ref())?;
                count += 1;
            }
            let mut events = tx.open_table(EVENTS)?;
            for item in legacy.open_tree("events")?.iter() {
                let (_, v) = item?;
                let event: StoredEvent = serde_json::from_slice(&v)?;
                events.insert(event_key(&event).as_str(), v.as_ref())?;
                count += 1;
            }
            for (name, def) in [("agent_memory", AGENT_MEMORY), ("config", CONFIG)] {
                let mut table = tx.open_table(def)?;
                for item in legacy.open_tree(name)?.iter() {
                    let (k, v) = item?;
                    table.insert(std::str::from_utf8(&k)?, v.as_ref())?;
                    count += 1;
                }
            }
            let marker = serde_json::to_vec(&chrono::Utc::now().to_rfc3339())?;
            tx.open_table(CONFIG)?.insert(SLED_MIGRATION_MARKER, marker.as_slice())?;
        }
        tx.commit()?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn node(id: &str) -> StoredNode {
        StoredNode {
            id: id.to_string(),
            name: "Test Node".to_string(),
            template: "edge".to_string(),
            ip: "10.42.0.10".to_string(),
            port: 7010,
            status: "ACTIVE".to_string(),
            last_seen: Utc::now(),
        }
    }

    fn event(offset_secs: i64, details: &str) -> StoredEvent {
        StoredEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now() + Duration::seconds(offset_secs),
            source: "test".into(),
            event_type: "Test".into(),
            severity: "INFO".into(),
            details: details.into(),
        }
    }

    #[test]
    fn test_store_save_and_list_nodes() -> Result<()> {
        let store = NetGeneStore::in_memory()?;
        store.save_node(&node("node-test-1"))?;
        let nodes = store.list_nodes()?;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "node-test-1");
        assert_eq!(store.get_node("node-test-1")?.unwrap().name, "Test Node");
        assert!(store.get_node("missing")?.is_none());
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

    #[test]
    fn test_events_are_newest_first() -> Result<()> {
        let store = NetGeneStore::in_memory()?;
        for (offset, d) in [(-30, "old"), (0, "mid"), (30, "new")] {
            store.save_event(&event(offset, d))?;
        }
        let latest: Vec<_> = store.list_events(2)?.into_iter().map(|e| e.details).collect();
        assert_eq!(latest, vec!["new", "mid"]);
        Ok(())
    }

    #[test]
    fn test_data_persists_across_reopen() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("store.redb");
        NetGeneStore::open(Some(&path))?.save_node(&node("persisted"))?;
        assert!(NetGeneStore::open(Some(&path))?.get_node("persisted")?.is_some());
        Ok(())
    }

    #[cfg(feature = "sled-migration")]
    #[test]
    fn test_import_legacy_sled() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let sled_dir = dir.path().join("db");
        {
            let legacy = sled::open(&sled_dir)?;
            legacy.open_tree("nodes")?.insert("n1", serde_json::to_vec(&node("n1"))?)?;
            let e = event(0, "legacy event");
            legacy.open_tree("events")?.insert(e.id.to_string(), serde_json::to_vec(&e)?)?;
            legacy.flush()?;
        }
        let store = NetGeneStore::open(Some(&dir.path().join("store.redb")))?;
        assert_eq!(store.import_sled(&sled_dir)?, 2);
        assert!(store.get_node("n1")?.is_some());
        assert_eq!(store.list_events(10)?[0].details, "legacy event");
        assert!(store.get_json::<String>(CONFIG, SLED_MIGRATION_MARKER)?.is_some(), "marker commits with the data");
        Ok(())
    }
}
