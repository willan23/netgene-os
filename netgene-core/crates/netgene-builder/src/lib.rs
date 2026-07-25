//! # NetGene Builder Engine
//!
//! Intent-based node provisioning and organic replication.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::info;

/// Node template type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeTemplate {
    Edge,
    Core,
    Gateway,
    Quantum,
    Custom(String),
}

impl std::fmt::Display for NodeTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeTemplate::Edge => write!(f, "edge"),
            NodeTemplate::Core => write!(f, "core"),
            NodeTemplate::Gateway => write!(f, "gateway"),
            NodeTemplate::Quantum => write!(f, "quantum"),
            NodeTemplate::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// A provisioned NetGene node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionedNode {
    pub id: String,
    pub template: NodeTemplate,
    pub ip: String,
    pub port: u16,
    pub provisioned_at: DateTime<Utc>,
    pub status: String,
    pub config: serde_json::Value,
}

/// The Builder Engine.
pub struct BuilderEngine {
    pub nodes: Vec<ProvisionedNode>,
}

impl BuilderEngine {
    pub fn new() -> Self {
        Self { nodes: vec![] }
    }

    /// Provision a new node from template.
    pub async fn provision(&mut self, template: NodeTemplate, count: u32) -> Result<Vec<ProvisionedNode>> {
        let mut new_nodes = vec![];

        for i in 0..count {
            // Simulate provisioning delay
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let id = format!("{}-{}", template, Uuid::new_v4().to_string()[..8].to_string());
            let node = ProvisionedNode {
                id: id.clone(),
                template: template.clone(),
                ip: format!("10.{}.{}.{}", 42, (self.nodes.len() / 256) % 256, self.nodes.len() % 256),
                port: 7000 + self.nodes.len() as u16,
                provisioned_at: Utc::now(),
                status: "running".to_string(),
                config: serde_json::json!({
                    "zero_trust": true,
                    "mtls": true,
                    "auto_scale": true,
                    "sequence": self.nodes.len() + i as usize,
                }),
            };

            info!(node_id = %id, template = %template, "Node provisioned");
            self.nodes.push(node.clone());
            new_nodes.push(node);
        }

        Ok(new_nodes)
    }

    /// Parse and execute a natural language build intent.
    pub async fn from_intent(&mut self, intent: &str) -> Result<String> {
        let lower = intent.to_lowercase();
        let template = if lower.contains("quantum") { NodeTemplate::Quantum }
            else if lower.contains("gateway") { NodeTemplate::Gateway }
            else if lower.contains("core") { NodeTemplate::Core }
            else { NodeTemplate::Edge };

        let count: u32 = lower.split_whitespace()
            .find_map(|w| w.parse().ok())
            .unwrap_or(1);

        let nodes = self.provision(template, count).await?;
        Ok(format!("✅ Provisioned {} node(s): {}", nodes.len(),
            nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>().join(", ")))
    }

    /// List all provisioned nodes.
    pub fn list(&self) -> &[ProvisionedNode] {
        &self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provision_single_edge_node() -> Result<()> {
        let mut engine = BuilderEngine::new();
        let nodes = engine.provision(NodeTemplate::Edge, 1).await?;
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].id.starts_with("edge-"));
        assert_eq!(nodes[0].status, "running");
        Ok(())
    }

    #[tokio::test]
    async fn test_provision_multiple_nodes() -> Result<()> {
        let mut engine = BuilderEngine::new();
        let nodes = engine.provision(NodeTemplate::Quantum, 3).await?;
        assert_eq!(nodes.len(), 3);
        assert_eq!(engine.list().len(), 3);
        // All IDs should be unique
        let ids: std::collections::HashSet<_> = nodes.iter().map(|n| &n.id).collect();
        assert_eq!(ids.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_from_intent_quantum() -> Result<()> {
        let mut engine = BuilderEngine::new();
        let result = engine.from_intent("spawn 2 quantum nodes").await?;
        assert!(result.contains("Provisioned"));
        assert_eq!(engine.list().len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_from_intent_gateway() -> Result<()> {
        let mut engine = BuilderEngine::new();
        let result = engine.from_intent("create 1 gateway node").await?;
        assert!(result.contains("Provisioned"));
        // Gateway nodes should have gateway- prefix
        assert!(engine.list().iter().any(|n| n.id.starts_with("gateway-")));
        Ok(())
    }

    #[tokio::test]
    async fn test_from_intent_defaults_to_edge() -> Result<()> {
        let mut engine = BuilderEngine::new();
        engine.from_intent("provision some nodes").await?;
        // Default template is edge
        assert!(engine.list().iter().any(|n| n.id.starts_with("edge-")));
        Ok(())
    }

    #[tokio::test]
    async fn test_node_config_contains_zero_trust() -> Result<()> {
        let mut engine = BuilderEngine::new();
        let nodes = engine.provision(NodeTemplate::Core, 1).await?;
        let config = &nodes[0].config;
        assert_eq!(config["zero_trust"], serde_json::json!(true));
        assert_eq!(config["mtls"], serde_json::json!(true));
        Ok(())
    }

    #[test]
    fn test_node_template_display() {
        assert_eq!(format!("{}", NodeTemplate::Edge), "edge");
        assert_eq!(format!("{}", NodeTemplate::Core), "core");
        assert_eq!(format!("{}", NodeTemplate::Gateway), "gateway");
        assert_eq!(format!("{}", NodeTemplate::Quantum), "quantum");
        assert_eq!(format!("{}", NodeTemplate::Custom("ai".to_string())), "custom:ai");
    }

    #[tokio::test]
    async fn test_list_grows_with_provisioning() -> Result<()> {
        let mut engine = BuilderEngine::new();
        assert_eq!(engine.list().len(), 0);
        engine.provision(NodeTemplate::Edge, 2).await?;
        assert_eq!(engine.list().len(), 2);
        engine.provision(NodeTemplate::Core, 1).await?;
        assert_eq!(engine.list().len(), 3);
        Ok(())
    }
}
