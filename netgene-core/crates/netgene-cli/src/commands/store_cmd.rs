//! Store CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_store::{NetGeneStore, StoredNode};

#[derive(Subcommand)]
pub enum StoreCommand {
    /// Show store database status and path
    Status,
    /// List all nodes in persistent storage
    Nodes,
    /// Save a node to persistent storage
    SaveNode {
        #[arg(long)]
        id: String,
        #[arg(short, long)]
        name: String,
        #[arg(short, long, default_value = "edge")]
        template: String,
        #[arg(long, default_value = "10.42.0.1")]
        ip: String,
        #[arg(short, long, default_value = "7000")]
        port: u16,
    },
    /// Dump all persistent records
    Dump,
}

pub async fn run(cmd: StoreCommand, json: bool) -> Result<()> {
    let store = NetGeneStore::open(None)?;

    match cmd {
        StoreCommand::Status => {
            println!("📂 NetGene Persistent Store (Sled DB)");
            println!("   Path: ~/.netgene/db");
            println!("   Engine: Sled v0.34 (ACID, Zero-Config)");
            println!("   CRDT: LWW-Register ready");
        }

        StoreCommand::Nodes => {
            let nodes = store.list_nodes()?;
            if nodes.is_empty() {
                println!("No nodes found in persistent store. Use `netgene store save-node`.");
            } else {
                println!("📂 Persistent Nodes ({})", nodes.len());
                for n in nodes {
                    println!("   • {} │ {} │ {}:{} │ {}", n.id, n.name, n.ip, n.port, n.status);
                }
            }
        }

        StoreCommand::SaveNode { id, name, template, ip, port } => {
            let node = StoredNode {
                id: id.clone(),
                name,
                template,
                ip,
                port,
                status: "ACTIVE".to_string(),
                last_seen: chrono::Utc::now(),
            };
            store.save_node(&node)?;
            println!("✅ Node '{}' saved to persistent store", id);
        }

        StoreCommand::Dump => {
            let nodes = store.list_nodes()?;
            let events = store.list_events(50)?;
            if json {
                println!("{}", serde_json::json!({ "nodes": nodes, "events": events }));
            } else {
                println!("📂 Database Dump:");
                println!("   Nodes: {}", nodes.len());
                println!("   Events: {}", events.len());
            }
        }
    }

    Ok(())
}
