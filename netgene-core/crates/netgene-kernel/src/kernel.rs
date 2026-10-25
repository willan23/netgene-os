//! NetSphere Kernel — the main orchestrator.
//!
//! Spawns and manages agents, routes intents, and maintains kernel state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::Result;
use serde::Serialize;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agent::{
    Agent, AgentId, AgentInfo, AgentMessage, MessageKind,
    BuilderAgent, MonitorAgent, OptimizerAgent, NetworkAgent, EvolutionAgent,
};
use crate::bus::MessageBus;
use netgene_gene::{crypto::GeneKeyPair, NetGene};
use netgene_llm::{LlmIntentEngine, OllamaClient};
use netgene_policy::{Action, AuditEntry, AuditLog, AuditRecord, Decision, Grant, Policy, PolicyEngine};
use crate::memory::KernelMemory;

/// Who the kernel acts as, which policy it enforces and where it audits.
pub struct KernelConfig {
    /// Gene on whose behalf intents are evaluated; its key signs the audit log.
    pub operator: NetGene,
    pub operator_kp: GeneKeyPair,
    pub policy: Policy,
    /// `None` keeps the audit log in memory.
    pub audit_path: Option<PathBuf>,
    /// Capabilities from verified capability tokens held by the operator.
    pub grants: Vec<Grant>,
    /// Enrolled hardware user-presence key (PKCS#1 RSA public key), if any.
    pub presence_key: Option<Vec<u8>>,
}

impl KernelConfig {
    /// Throw-away Master Gene, default policy, in-memory audit (tests, demos).
    pub fn ephemeral() -> Result<Self> {
        let (operator, operator_kp) = NetGene::generate_master("ephemeral-operator")?;
        Ok(Self { operator, operator_kp, policy: Policy::default(), audit_path: None, grants: vec![], presence_key: None })
    }
}

/// Result of submitting an intent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DispatchOutcome {
    Executed { action: Action, explanation: String },
    PendingConfirmation { proposal_id: Uuid, action: Action, reason: String, explanation: String },
    Denied { action: Option<Action>, reason: String, explanation: String },
}

impl DispatchOutcome {
    /// One-line human summary.
    pub fn summary(&self) -> String {
        match self {
            DispatchOutcome::Executed { action, explanation } =>
                format!("✅ Action dispatched: {}\n🧠 {}", action.kind, explanation),
            DispatchOutcome::PendingConfirmation { proposal_id, action, reason, .. } =>
                format!("⏸️ Confirmation required for {} ({}). Proposal: {}", action.kind, reason, proposal_id),
            DispatchOutcome::Denied { reason, .. } => format!("⛔ Denied by policy: {}", reason),
        }
    }
}

/// The Netsphere Kernel state.
pub struct NetSphereKernel {
    /// Bus sender — use this to inject messages.
    bus_tx: mpsc::Sender<AgentMessage>,
    /// Agent registry: id → info snapshot.
    agents: Arc<Mutex<HashMap<AgentId, AgentInfo>>>,
    /// Kernel memory.
    pub memory: Arc<Mutex<KernelMemory>>,
    operator: NetGene,
    policy: PolicyEngine,
    audit: Mutex<AuditLog>,
    /// Actions waiting for human confirmation: id → (original input, action).
    pending: Mutex<HashMap<Uuid, (String, Action)>>,
    grants: Vec<Grant>,
    presence_key: Option<Vec<u8>>,
    /// Running agent task handles.
    _task_handles: Vec<JoinHandle<()>>,
}

impl NetSphereKernel {
    /// Boot with an ephemeral operator and the default policy.
    pub async fn boot() -> Result<Self> {
        Self::boot_with(KernelConfig::ephemeral()?).await
    }

    /// Boot the kernel: create agents, wire the bus, start tasks.
    pub async fn boot_with(config: KernelConfig) -> Result<Self> {
        info!("🧬 Netsphere Kernel booting (5 core agents)...");
        let audit = match config.audit_path {
            Some(path) => AuditLog::open(path, config.operator_kp)?,
            None => AuditLog::in_memory(config.operator_kp),
        };

        let mut bus = MessageBus::new();
        let agents_map: Arc<Mutex<HashMap<AgentId, AgentInfo>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let memory = Arc::new(Mutex::new(KernelMemory::new()));
        let bus_tx = bus.sender();
        let mut handles = vec![];

        // 1. Builder Agent
        let mut builder = BuilderAgent::new();
        let builder_id = builder.id();
        let (b_rx, b_tx) = bus.register_agent(builder_id);
        agents_map.lock().await.insert(builder_id, builder.info());
        handles.push(tokio::spawn(async move {
            if let Err(e) = builder.run(b_rx, b_tx).await {
                warn!(err = %e, "BuilderAgent crashed");
            }
        }));

        // 2. Monitor Agent
        let mut monitor = MonitorAgent::new();
        let monitor_id = monitor.id();
        let (m_rx, m_tx) = bus.register_agent(monitor_id);
        agents_map.lock().await.insert(monitor_id, monitor.info());
        handles.push(tokio::spawn(async move {
            if let Err(e) = monitor.run(m_rx, m_tx).await {
                warn!(err = %e, "MonitorAgent crashed");
            }
        }));

        // 3. Optimizer Agent
        let mut optimizer = OptimizerAgent::new();
        let optimizer_id = optimizer.id();
        let (o_rx, o_tx) = bus.register_agent(optimizer_id);
        agents_map.lock().await.insert(optimizer_id, optimizer.info());
        handles.push(tokio::spawn(async move {
            if let Err(e) = optimizer.run(o_rx, o_tx).await {
                warn!(err = %e, "OptimizerAgent crashed");
            }
        }));

        // 4. Network Agent (Phase 2)
        let mut network_agent = NetworkAgent::new();
        let network_id = network_agent.id();
        let (n_rx, n_tx) = bus.register_agent(network_id);
        agents_map.lock().await.insert(network_id, network_agent.info());
        handles.push(tokio::spawn(async move {
            if let Err(e) = network_agent.run(n_rx, n_tx).await {
                warn!(err = %e, "NetworkAgent crashed");
            }
        }));

        // 5. Evolution Agent (Phase 2)
        let mut evo_agent = EvolutionAgent::new();
        let evo_id = evo_agent.id();
        let (e_rx, e_tx) = bus.register_agent(evo_id);
        agents_map.lock().await.insert(evo_id, evo_agent.info());
        handles.push(tokio::spawn(async move {
            if let Err(e) = evo_agent.run(e_rx, e_tx).await {
                warn!(err = %e, "EvolutionAgent crashed");
            }
        }));

        // Start message bus router
        tokio::spawn(async move { bus.run().await });

        info!("✅ Netsphere Kernel online with 5 agents (Builder, Monitor, Optimizer, Network, Evolution)");

        Ok(Self {
            bus_tx,
            agents: agents_map,
            memory,
            operator: config.operator,
            policy: PolicyEngine::new(config.policy),
            audit: Mutex::new(audit),
            pending: Mutex::new(HashMap::new()),
            grants: config.grants,
            presence_key: config.presence_key,
            _task_handles: handles,
        })
    }

    /// Submit a natural-language intent. The LLM (or the rule-based fallback)
    /// only *proposes* an action; the policy engine decides whether it runs
    /// now, waits for confirmation, or is denied. Every step is audited.
    pub async fn propose_intent(&self, input: &str) -> Result<DispatchOutcome> {
        let client = OllamaClient::new(Some("http://localhost:11434"), Some("llama3"));
        let engine = LlmIntentEngine::new(client);

        let llm_intent = engine.parse(input).await.unwrap_or_else(|e| {
            warn!("LLM parse failed: {}", e);
            netgene_llm::LlmParsedIntent {
                action: "unknown".to_string(),
                parameters: serde_json::json!({}),
                explanation: format!("LLM error: {}", e),
            }
        });
        let explanation = llm_intent.explanation.clone();

        {
            let mut mem = self.memory.lock().await;
            mem.log_event(format!("Intent: {} (proposed: {})", input, llm_intent.action));
        }

        let Some(action) = Action::from_llm_intent(&llm_intent.action, llm_intent.parameters.clone()) else {
            let reason = format!("'{}' is not an executable action", llm_intent.action);
            self.audit(input, None, "deny", &reason).await?;
            return Ok(DispatchOutcome::Denied { action: None, reason, explanation });
        };

        match self.evaluate(&action) {
            Decision::Allow => {
                self.audit(input, Some(&action), "allow", "").await?;
                self.execute(input, &action).await?;
                Ok(DispatchOutcome::Executed { action, explanation })
            }
            Decision::Confirm { reason } => {
                self.audit(input, Some(&action), "confirm", &reason).await?;
                let proposal_id = Uuid::new_v4();
                self.pending.lock().await.insert(proposal_id, (input.to_string(), action.clone()));
                Ok(DispatchOutcome::PendingConfirmation { proposal_id, action, reason, explanation })
            }
            Decision::Deny { reason } => {
                self.audit(input, Some(&action), "deny", &reason).await?;
                Ok(DispatchOutcome::Denied { action: Some(action), reason, explanation })
            }
        }
    }

    /// Whether confirmations need a hardware user-presence proof.
    pub fn requires_user_presence(&self) -> bool {
        self.policy.policy().require_user_presence
    }

    /// Bytes the operator's presence key must sign to approve `proposal_id`.
    pub async fn presence_challenge(&self, proposal_id: Uuid) -> Result<Vec<u8>> {
        let pending = self.pending.lock().await;
        let (_, action) = pending
            .get(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;
        netgene_presence::challenge(proposal_id, &action.kind, &action.params)
    }

    /// Human approval of a pending proposal (when the policy does not require
    /// a user-presence proof).
    pub async fn confirm(&self, proposal_id: Uuid) -> Result<DispatchOutcome> {
        if self.requires_user_presence() {
            anyhow::bail!("Policy requires a user-presence proof to confirm (use confirm_with_presence)");
        }
        self.confirm_inner(proposal_id, format!("proposal {}", proposal_id)).await
    }

    /// Human approval backed by a hardware user-presence signature over
    /// [`Self::presence_challenge`].
    pub async fn confirm_with_presence(&self, proposal_id: Uuid, signature: &[u8]) -> Result<DispatchOutcome> {
        let key = self
            .presence_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No user-presence key enrolled for the operator"))?;
        let challenge = self.presence_challenge(proposal_id).await?;
        if !netgene_presence::verify(key, &challenge, signature) {
            let pending = self.pending.lock().await;
            if let Some((input, action)) = pending.get(&proposal_id) {
                self.audit(input, Some(action), "presence_failed", &format!("proposal {}", proposal_id)).await?;
            }
            anyhow::bail!("User-presence signature is invalid for proposal {}", proposal_id);
        }
        self.confirm_inner(proposal_id, format!("proposal {} (user presence verified)", proposal_id)).await
    }

    async fn confirm_inner(&self, proposal_id: Uuid, detail: String) -> Result<DispatchOutcome> {
        let (input, action) = self
            .pending
            .lock()
            .await
            .remove(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;

        // Re-check: the operator could have been suspended since the proposal.
        if let Decision::Deny { reason } = self.evaluate(&action) {
            self.audit(&input, Some(&action), "deny", &reason).await?;
            return Ok(DispatchOutcome::Denied { action: Some(action), reason, explanation: String::new() });
        }

        self.audit(&input, Some(&action), "confirmed", &detail).await?;
        self.execute(&input, &action).await?;
        Ok(DispatchOutcome::Executed { action, explanation: "confirmed by operator".to_string() })
    }

    /// Discard a pending proposal.
    pub async fn reject(&self, proposal_id: Uuid) -> Result<()> {
        let (input, action) = self
            .pending
            .lock()
            .await
            .remove(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;
        self.audit(&input, Some(&action), "rejected", &format!("proposal {}", proposal_id)).await?;
        Ok(())
    }

    /// Policy decision for the operator, counting only unexpired grants.
    fn evaluate(&self, action: &Action) -> Decision {
        let now = chrono::Utc::now();
        let granted: Vec<String> = self
            .grants
            .iter()
            .filter(|g| g.is_active(now))
            .flat_map(|g| g.capabilities.iter().cloned())
            .collect();
        self.policy.evaluate_with_grants(&self.operator, &granted, action)
    }

    /// Compatibility wrapper returning a human-readable summary.
    pub async fn dispatch_intent(&self, input: &str) -> Result<String> {
        Ok(self.propose_intent(input).await?.summary())
    }

    /// Entries of the audit log.
    pub async fn audit_entries(&self) -> Result<Vec<AuditEntry>> {
        self.audit.lock().await.entries()
    }

    /// The policy currently enforced.
    pub fn policy(&self) -> &Policy {
        self.policy.policy()
    }

    async fn audit(&self, input: &str, action: Option<&Action>, decision: &str, detail: &str) -> Result<()> {
        self.audit.lock().await.append(AuditRecord {
            actor_gene_id: self.operator.id.to_string(),
            input: input.to_string(),
            action: action.map(|a| a.kind.clone()),
            params: action.map(|a| a.params.clone()).unwrap_or(serde_json::Value::Null),
            decision: decision.to_string(),
            detail: detail.to_string(),
        })?;
        Ok(())
    }

    /// Translate an approved catalog action into agent messages.
    async fn execute(&self, input: &str, action: &Action) -> Result<()> {
        let msg = match action.kind.as_str() {
            "node.provision" => {
                let count = action.params.get("count").and_then(|c| c.as_u64()).unwrap_or(1);
                AgentMessage::new(None, None, MessageKind::Intent, serde_json::json!({ "action": "spawn_node", "count": count }))
            }
            "network.optimize" => {
                AgentMessage::new(None, None, MessageKind::Intent, serde_json::json!({ "action": "optimize_routes" }))
            }
            "system.status" => AgentMessage::new(None, None, MessageKind::Heartbeat, serde_json::json!({})),
            "safeguard.scan" => AgentMessage::new(
                None, None, MessageKind::Alert,
                serde_json::json!({ "message": "Anomaly scan requested by operator" }),
            ),
            other => anyhow::bail!("No executor for action '{}'", other),
        };
        self.bus_tx.send(msg).await?;
        self.audit(input, Some(action), "executed", "").await
    }

    pub async fn dispatch_swarm_sync(&self) -> Result<usize> {
        info!("🐝 Iniciando Federated Swarm Sync...");
        let msg = AgentMessage::new(None, None, MessageKind::SwarmSync, serde_json::json!({}));
        self.bus_tx.send(msg).await?;
        
        let mut mem = self.memory.lock().await;
        mem.log_event("Federated Swarm Sync Initiated".to_string());
        
        Ok(self.agents.lock().await.len())
    }

    pub async fn inject_marketplace_agent(&self, id: String, name: String) {
        let agent_id = uuid::Uuid::new_v4();
        let info = AgentInfo {
            id: agent_id,
            name,
            agent_type: id,
            status: crate::agent::AgentStatus::Idle,
            messages_processed: 0,
            started_at: chrono::Utc::now(),
        };
        self.agents.lock().await.insert(agent_id, info);
    }

    /// Get snapshot of all agent infos.
    pub async fn agent_list(&self) -> Vec<AgentInfo> {
        let map = self.agents.lock().await;
        map.values().cloned().collect()
    }

    /// Graceful shutdown.
    pub async fn shutdown(&self) -> Result<()> {
        info!("Netsphere Kernel shutting down...");
        let msg = AgentMessage::new(None, None, MessageKind::Shutdown, serde_json::json!({}));
        self.bus_tx.send(msg).await?;
        Ok(())
    }
}
