//! Async message bus for inter-agent communication.
//!
//! The `MessageBus` routes messages between agents using Tokio MPSC channels.
//! It maintains a registry of agent senders and provides broadcast capability.

use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::agent::{AgentId, AgentMessage, MessageKind};

/// Channel buffer size per agent.
const CHANNEL_BUF: usize = 256;

/// Message bus that routes AgentMessages between agents.
pub struct MessageBus {
    /// Map of agent_id → sender channel.
    senders: HashMap<AgentId, mpsc::Sender<AgentMessage>>,
    /// Receiver for messages coming INTO the bus.
    bus_rx: mpsc::Receiver<AgentMessage>,
    /// Sender for messages going INTO the bus (cloneable).
    bus_tx: mpsc::Sender<AgentMessage>,
}

impl MessageBus {
    /// Create a new message bus.
    pub fn new() -> Self {
        let (bus_tx, bus_rx) = mpsc::channel(1024);
        Self {
            senders: HashMap::new(),
            bus_rx,
            bus_tx,
        }
    }

    /// Register a new agent, returning its (rx, bus_tx).
    /// The agent reads from rx; sends responses back to bus_tx.
    pub fn register_agent(&mut self, agent_id: AgentId) -> (mpsc::Receiver<AgentMessage>, mpsc::Sender<AgentMessage>) {
        let (tx, rx) = mpsc::channel(CHANNEL_BUF);
        self.senders.insert(agent_id, tx);
        (rx, self.bus_tx.clone())
    }

    /// Get a clone of the bus sender (for external callers to inject messages).
    pub fn sender(&self) -> mpsc::Sender<AgentMessage> {
        self.bus_tx.clone()
    }

    /// Route loop — reads from bus_rx and delivers to appropriate agents.
    pub async fn run(mut self) {
        while let Some(msg) = self.bus_rx.recv().await {
            if msg.kind == MessageKind::Shutdown {
                // Broadcast shutdown to all agents
                for (id, tx) in &self.senders {
                    let shutdown = AgentMessage::new(None, Some(*id), MessageKind::Shutdown, serde_json::json!({}));
                    let _ = tx.send(shutdown).await;
                }
                break;
            }

            match msg.to {
                Some(target_id) => {
                    if let Some(tx) = self.senders.get(&target_id) {
                        if tx.send(msg).await.is_err() {
                            warn!(target = %target_id, "Failed to deliver message — agent channel closed");
                        }
                    } else {
                        warn!(target = %target_id, "No agent registered with this ID");
                    }
                }
                None => {
                    // Broadcast to all agents
                    debug!("Broadcasting message to {} agents", self.senders.len());
                    for (id, tx) in &self.senders {
                        let mut broadcast = msg.clone();
                        broadcast.to = Some(*id);
                        let _ = tx.send(broadcast).await;
                    }
                }
            }
        }
    }
}
