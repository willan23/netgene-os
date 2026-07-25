//! NetSphere Kernel — the main orchestrator.
//!
//! Spawns and manages agents, routes intents, and maintains kernel state.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use anyhow::Result;
use tracing::{info, warn};

use crate::agent::{
    Agent, AgentId, AgentInfo, AgentMessage, MessageKind,
    BuilderAgent, MonitorAgent, OptimizerAgent, NetworkAgent, EvolutionAgent,
};
use crate::bus::MessageBus;
use crate::intent::{IntentAction, IntentParser};
use netgene_llm::{LlmIntentEngine, OllamaClient};
use crate::memory::KernelMemory;

/// The Netsphere Kernel state.
pub struct NetSphereKernel {
    /// Bus sender — use this to inject messages.
    bus_tx: mpsc::Sender<AgentMessage>,
    /// Agent registry: id → info snapshot.
    agents: Arc<Mutex<HashMap<AgentId, AgentInfo>>>,
    /// Kernel memory.
    pub memory: Arc<Mutex<KernelMemory>>,
    /// Running agent task handles.
    _task_handles: Vec<JoinHandle<()>>,
}

impl NetSphereKernel {
    /// Boot the kernel: create agents, wire the bus, start tasks.
    pub async fn boot() -> Result<Self> {
        info!("🧬 Netsphere Kernel booting (5 core agents)...");

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
            _task_handles: handles,
        })
    }

    /// Parse and dispatch a natural language intent to the appropriate agent using BDI / Ollama.
    pub async fn dispatch_intent(&self, input: &str) -> Result<String> {
        let client = OllamaClient::new(Some("http://localhost:11434"), Some("llama3"));
        let engine = LlmIntentEngine::new(client);

        info!("Enviando intent para o LLM Engine: {}", input);
        
        let llm_intent = engine.parse(input).await.unwrap_or_else(|e| {
            warn!("LLM Parse failed: {}. Usando fallback.", e);
            netgene_llm::LlmParsedIntent {
                action: "unknown".to_string(),
                parameters: serde_json::json!({}),
                explanation: format!("Erro LLM: {}", e),
            }
        });

        // Log to memory
        {
            let mut mem = self.memory.lock().await;
            mem.log_event(format!("BDI Intent: {} (Action: {})", input, llm_intent.action));
        }

        let msg = match llm_intent.action.as_str() {
            "provision_nodes" => {
                let count = llm_intent.parameters.get("count").and_then(|c| c.as_u64()).unwrap_or(1);
                AgentMessage::new(
                    None, None, MessageKind::Intent,
                    serde_json::json!({ "action": "spawn_node", "count": count }),
                )
            }
            "optimize_network" => {
                AgentMessage::new(
                    None, None, MessageKind::Intent,
                    serde_json::json!({ "action": "optimize_routes" }),
                )
            }
            "system_status" => {
                AgentMessage::new(None, None, MessageKind::Heartbeat, serde_json::json!({}))
            }
            "trigger_anomaly_scan" => {
                AgentMessage::new(
                    None, None, MessageKind::Alert,
                    serde_json::json!({ "message": "Self-heal requested by operator" }),
                )
            }
            _ => {
                let fallback = IntentParser::parse(input);
                if let IntentAction::Unknown { raw } = fallback.action {
                     return Ok(format!("⚠️ Intent not recognized by BDI: '{}'\nLLM Output: {}", raw, llm_intent.explanation));
                } else {
                     return Ok(format!("✅ Intent dispatched by Fallback: {:?}\nBDI Explanation: {}", fallback.action, llm_intent.explanation));
                }
            }
        };

        self.bus_tx.send(msg).await?;
        
        Ok(format!(
            "🧠 BDI Reasoning: {}\n✅ Action dispatched: {}",
            llm_intent.explanation,
            llm_intent.action
        ))
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
