//! P2P network message and event definitions.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Messages broadcasted across the NetGene P2P Gossipsub network topic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshMessage {
    /// Node identity announcement
    NodeAnnounce {
        gene_id: String,
        node_name: String,
        listen_addrs: Vec<String>,
        capabilities: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    /// Telemetry & anomaly notification
    AnomalyAlert {
        id: Uuid,
        source_node: String,
        severity: String,
        metric: String,
        value: f64,
        timestamp: DateTime<Utc>,
    },
    /// Self-healing action coordination
    HealingActionBroadcast {
        action_id: Uuid,
        target_node: String,
        action: String,
        timestamp: DateTime<Utc>,
    },
    /// General intent or sync payload
    IntentBroadcast {
        sender: String,
        intent: String,
        timestamp: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_message_node_announce_roundtrip() {
        let msg = MeshMessage::NodeAnnounce {
            gene_id: "gene-master-01".to_string(),
            node_name: "edge-node-01".to_string(),
            listen_addrs: vec!["/ip4/127.0.0.1/tcp/7000".to_string()],
            capabilities: vec!["quantum.run".to_string(), "node.spawn".to_string()],
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MeshMessage = serde_json::from_str(&json).unwrap();
        if let MeshMessage::NodeAnnounce { gene_id, node_name, .. } = back {
            assert_eq!(gene_id, "gene-master-01");
            assert_eq!(node_name, "edge-node-01");
        } else {
            panic!("Expected NodeAnnounce");
        }
    }

    #[test]
    fn test_mesh_message_anomaly_alert_roundtrip() {
        let id = Uuid::new_v4();
        let msg = MeshMessage::AnomalyAlert {
            id,
            source_node: "node-42".to_string(),
            severity: "CRITICAL".to_string(),
            metric: "cpu".to_string(),
            value: 99.9,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_vec(&msg).unwrap();
        let back: MeshMessage = serde_json::from_slice(&json).unwrap();
        if let MeshMessage::AnomalyAlert { id: back_id, severity, .. } = back {
            assert_eq!(back_id, id);
            assert_eq!(severity, "CRITICAL");
        } else {
            panic!("Expected AnomalyAlert");
        }
    }

    #[test]
    fn test_mesh_message_intent_broadcast_roundtrip() {
        let msg = MeshMessage::IntentBroadcast {
            sender: "gene-master-01".to_string(),
            intent: "spawn 3 quantum nodes".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MeshMessage = serde_json::from_str(&json).unwrap();
        if let MeshMessage::IntentBroadcast { sender, intent, .. } = back {
            assert_eq!(sender, "gene-master-01");
            assert!(intent.contains("quantum"));
        } else {
            panic!("Expected IntentBroadcast");
        }
    }

    #[test]
    fn test_mesh_message_healing_broadcast_roundtrip() {
        let action_id = Uuid::new_v4();
        let msg = MeshMessage::HealingActionBroadcast {
            action_id,
            target_node: "node-01".to_string(),
            action: "RESTART_PROCESS".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MeshMessage = serde_json::from_str(&json).unwrap();
        if let MeshMessage::HealingActionBroadcast { action_id: id, action, .. } = back {
            assert_eq!(id, action_id);
            assert_eq!(action, "RESTART_PROCESS");
        } else {
            panic!("Expected HealingActionBroadcast");
        }
    }
}
