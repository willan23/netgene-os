//! Agent definitions and implementations.
//!
//! Each agent is an autonomous async task that processes messages from the bus
//! and produces actions or responses.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use async_trait::async_trait;
use anyhow::Result;
use tracing::{info, warn, debug};

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

/// A message passed between agents or from the kernel to agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message ID.
    pub id: Uuid,
    /// Sending agent/kernel ID.
    pub from: Option<AgentId>,
    /// Target agent ID (None = broadcast).
    pub to: Option<AgentId>,
    /// Message type/intent.
    pub kind: MessageKind,
    /// JSON payload.
    pub payload: serde_json::Value,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
}

/// Classification of messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageKind {
    /// Intent from the user/system to perform an action.
    Intent,
    /// Status report from an agent.
    StatusReport,
    /// Request from one agent to another.
    Request,
    /// Response to a request.
    Response,
    /// Alert/anomaly detected.
    Alert,
    /// Heartbeat (keep-alive).
    Heartbeat,
    /// Shutdown signal.
    Shutdown,
    /// Federated learning / Swarm Intelligence sync request.
    SwarmSync,
    /// Federated learning / Swarm Intelligence update from Kernel.
    FederatedUpdate,
}

impl AgentMessage {
    pub fn new(from: Option<AgentId>, to: Option<AgentId>, kind: MessageKind, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            kind,
            payload,
            timestamp: Utc::now(),
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

/// Core Agent trait — all agents implement this.
#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> AgentId;
    fn name(&self) -> &str;
    fn agent_type(&self) -> &str;
    fn status(&self) -> AgentStatus;
    fn info(&self) -> AgentInfo;

    /// Process a single message.
    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>>;

    /// Agent main loop — reads from rx, writes responses to tx.
    async fn run(
        &mut self,
        mut rx: mpsc::Receiver<AgentMessage>,
        tx: mpsc::Sender<AgentMessage>,
    ) -> Result<()> {
        info!(agent = %self.name(), id = %self.id(), "Agent started");
        while let Some(msg) = rx.recv().await {
            if msg.kind == MessageKind::Shutdown {
                info!(agent = %self.name(), "Agent received shutdown");
                break;
            }
            debug!(agent = %self.name(), msg_id = %msg.id, kind = ?msg.kind, "Handling message");
            match self.handle(msg).await {
                Ok(Some(response)) => {
                    let _ = tx.send(response).await;
                }
                Ok(None) => {}
                Err(e) => warn!(agent = %self.name(), err = %e, "Error handling message"),
            }
        }
        info!(agent = %self.name(), "Agent stopped");
        Ok(())
    }
}

// ─── Builder Agent ─────────────────────────────────────────────────────────

pub struct BuilderAgent {
    pub id: AgentId,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
    pub provisioned_nodes: Vec<String>,
}

impl BuilderAgent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: Utc::now(),
            provisioned_nodes: vec![],
        }
    }
}

#[async_trait]
impl Agent for BuilderAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { "BuilderAgent" }
    fn agent_type(&self) -> &str { "builder" }
    fn status(&self) -> AgentStatus { self.status.clone() }
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id,
            name: self.name().to_string(),
            agent_type: self.agent_type().to_string(),
            status: self.status(),
            messages_processed: self.messages_processed,
            started_at: self.started_at,
        }
    }

    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>> {
        self.status = AgentStatus::Processing;
        self.messages_processed += 1;

        let result = match msg.kind {
            MessageKind::Intent => {
                let action = msg.payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
                match action {
                    "spawn_node" | "provision_nodes" => {
                        let node_id = format!("node-{}", Uuid::new_v4().to_string()[..8].to_string());
                        self.provisioned_nodes.push(node_id.clone());
                        info!(agent = "BuilderAgent", node = %node_id, "Node provisioned");
                        Some(AgentMessage::new(
                            Some(self.id),
                            msg.from,
                            MessageKind::Response,
                            serde_json::json!({ "status": "ok", "node_id": node_id }),
                        ))
                    }
                    _ => None,
                }
            }
            MessageKind::Heartbeat => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::StatusReport,
                    serde_json::json!({
                        "status": "alive",
                        "nodes_provisioned": self.provisioned_nodes.len()
                    }),
                ))
            }
            MessageKind::SwarmSync => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::Response,
                    serde_json::json!({
                        "agent": self.name(),
                        "local_knowledge": self.provisioned_nodes
                    }),
                ))
            }
            MessageKind::FederatedUpdate => {
                info!(agent = "BuilderAgent", "Received federated knowledge update from Kernel");
                None
            }
            _ => None,
        };

        self.status = AgentStatus::Idle;
        Ok(result)
    }
}

// ─── Monitor Agent ──────────────────────────────────────────────────────────

pub struct MonitorAgent {
    pub id: AgentId,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
    pub alerts: Vec<String>,
}

impl MonitorAgent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: Utc::now(),
            alerts: vec![],
        }
    }
}

#[async_trait]
impl Agent for MonitorAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { "MonitorAgent" }
    fn agent_type(&self) -> &str { "monitor" }
    fn status(&self) -> AgentStatus { self.status.clone() }
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id,
            name: self.name().to_string(),
            agent_type: self.agent_type().to_string(),
            status: self.status(),
            messages_processed: self.messages_processed,
            started_at: self.started_at,
        }
    }

    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>> {
        self.status = AgentStatus::Processing;
        self.messages_processed += 1;

        let result = match msg.kind {
            MessageKind::Alert => {
                let alert_msg = msg.payload
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown alert")
                    .to_string();
                warn!(agent = "MonitorAgent", alert = %alert_msg, "Alert received");
                self.alerts.push(alert_msg.clone());
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::Response,
                    serde_json::json!({ "acknowledged": true, "alert": alert_msg }),
                ))
            }
            MessageKind::Heartbeat => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::StatusReport,
                    serde_json::json!({
                        "status": "monitoring",
                        "alerts_total": self.alerts.len()
                    }),
                ))
            }
            MessageKind::SwarmSync => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::Response,
                    serde_json::json!({
                        "agent": self.name(),
                        "local_knowledge": self.alerts
                    }),
                ))
            }
            MessageKind::FederatedUpdate => {
                info!(agent = "MonitorAgent", "Received federated knowledge update from Kernel");
                None
            }
            _ => None,
        };

        self.status = AgentStatus::Idle;
        Ok(result)
    }
}

// ─── Optimizer Agent ────────────────────────────────────────────────────────

pub struct OptimizerAgent {
    pub id: AgentId,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
    pub optimizations_run: u64,
}

impl OptimizerAgent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: Utc::now(),
            optimizations_run: 0,
        }
    }
}

#[async_trait]
impl Agent for OptimizerAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { "OptimizerAgent" }
    fn agent_type(&self) -> &str { "optimizer" }
    fn status(&self) -> AgentStatus { self.status.clone() }
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id,
            name: self.name().to_string(),
            agent_type: self.agent_type().to_string(),
            status: self.status(),
            messages_processed: self.messages_processed,
            started_at: self.started_at,
        }
    }

    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>> {
        self.status = AgentStatus::Processing;
        self.messages_processed += 1;

        let result = match msg.kind {
            MessageKind::Intent => {
                let action = msg.payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
                if action == "optimize_routes" || action == "optimize_network" {
                    self.optimizations_run += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    let improvement = 15 + (self.optimizations_run % 20);
                    info!(
                        agent = "OptimizerAgent",
                        improvement = %improvement,
                        "Route optimization complete"
                    );
                    Some(AgentMessage::new(
                        Some(self.id),
                        msg.from,
                        MessageKind::Response,
                        serde_json::json!({
                            "status": "optimized",
                            "improvement_pct": improvement,
                            "algorithm": "QAOA-sim"
                        }),
                    ))
                } else {
                    None
                }
            }
            MessageKind::Heartbeat => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::StatusReport,
                    serde_json::json!({
                        "status": "ready",
                        "optimizations_run": self.optimizations_run
                    }),
                ))
            }
            _ => None,
        };

        self.status = AgentStatus::Idle;
        Ok(result)
    }
}

// ─── Network Agent (Phase 2) ────────────────────────────────────────────────

pub struct NetworkAgent {
    pub id: AgentId,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
    pub active_peers: usize,
}

impl NetworkAgent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: Utc::now(),
            active_peers: 1, // local seed node
        }
    }
}

#[async_trait]
impl Agent for NetworkAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { "NetworkAgent" }
    fn agent_type(&self) -> &str { "network" }
    fn status(&self) -> AgentStatus { self.status.clone() }
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id,
            name: self.name().to_string(),
            agent_type: self.agent_type().to_string(),
            status: self.status(),
            messages_processed: self.messages_processed,
            started_at: self.started_at,
        }
    }

    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>> {
        self.status = AgentStatus::Processing;
        self.messages_processed += 1;

        let result = match msg.kind {
            MessageKind::Heartbeat => {
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::StatusReport,
                    serde_json::json!({
                        "active_peers": self.active_peers,
                        "protocol": "libp2p/netgene-mesh-v1"
                    }),
                ))
            }
            _ => None,
        };

        self.status = AgentStatus::Idle;
        Ok(result)
    }
}

// ─── Evolution Agent (Phase 2) ──────────────────────────────────────────────

pub struct EvolutionAgent {
    pub id: AgentId,
    pub status: AgentStatus,
    pub messages_processed: u64,
    pub started_at: DateTime<Utc>,
    pub mutations_proposed: u64,
    pub executed_commands: Vec<String>,
}

impl EvolutionAgent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: Utc::now(),
            mutations_proposed: 0,
            executed_commands: vec![],
        }
    }
}

#[async_trait]
impl Agent for EvolutionAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { "EvolutionAgent" }
    fn agent_type(&self) -> &str { "evolution" }
    fn status(&self) -> AgentStatus { self.status.clone() }
    fn info(&self) -> AgentInfo {
        AgentInfo {
            id: self.id,
            name: self.name().to_string(),
            agent_type: self.agent_type().to_string(),
            status: self.status(),
            messages_processed: self.messages_processed,
            started_at: self.started_at,
        }
    }

    async fn handle(&mut self, msg: AgentMessage) -> Result<Option<AgentMessage>> {
        self.status = AgentStatus::Processing;
        self.messages_processed += 1;

        let result = match msg.kind {
            MessageKind::Intent | MessageKind::Request => {
                self.mutations_proposed += 1;
                let command_text = msg.payload
                    .get("command")
                    .or_else(|| msg.payload.get("action"))
                    .or_else(|| msg.payload.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Custom Evolution Directive")
                    .to_string();

                info!(agent = "EvolutionAgent", command = %command_text, "Executing system evolution directive");
                self.executed_commands.push(command_text.clone());

                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::Response,
                    serde_json::json!({
                        "status": "EXECUTED",
                        "command": command_text,
                        "mutation_id": format!("MUT-{}", self.mutations_proposed),
                        "fitness_score": 0.992,
                        "details": format!("EvolutionAgent executed directive: '{}'", command_text)
                    }),
                ))
            }
            MessageKind::Heartbeat => {
                self.mutations_proposed += 1;
                Some(AgentMessage::new(
                    Some(self.id),
                    msg.from,
                    MessageKind::StatusReport,
                    serde_json::json!({
                        "system_generation": self.mutations_proposed,
                        "executed_commands_count": self.executed_commands.len(),
                        "fitness_score": 0.985
                    }),
                ))
            }
            _ => None,
        };

        self.status = AgentStatus::Idle;
        Ok(result)
    }
}
