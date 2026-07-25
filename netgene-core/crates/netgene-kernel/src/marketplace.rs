use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<String>,
}

pub struct Marketplace {
    registry_path: PathBuf,
}

impl Marketplace {
    pub fn new() -> Self {
        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push(".netgene");
        path.push("marketplace");
        
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
            
            // Generate some dummy agents representing remote marketplace items
            let deepseek = AgentManifest {
                id: "deepseek-coder-bdi".into(),
                name: "DeepSeek Coder Agent".into(),
                version: "1.0.0".into(),
                description: "Autonomous coding agent powered by DeepSeek. Generates and reviews code dynamically.".into(),
                author: "NetGene Community".into(),
                capabilities: vec!["code-gen".into(), "review".into()],
            };
            let _ = fs::write(path.join("deepseek-coder.json"), serde_json::to_string_pretty(&deepseek).unwrap());

            let sentinel = AgentManifest {
                id: "sentinel-guard-v2".into(),
                name: "Sentinel Guard".into(),
                version: "2.1.0".into(),
                description: "Zero-Trust network observer. Detects anomalies using Z-Score algorithms and heals routes.".into(),
                author: "CyberSec Org".into(),
                capabilities: vec!["anomaly-detection".into(), "self-healing".into()],
            };
            let _ = fs::write(path.join("sentinel-guard.json"), serde_json::to_string_pretty(&sentinel).unwrap());

            let swarm_coord = AgentManifest {
                id: "swarm-coordinator".into(),
                name: "Swarm Hive-Mind Coordinator".into(),
                version: "1.0.0".into(),
                description: "Orquestra a inteligência distribuída entre nós locais, unificando as métricas federadas.".into(),
                author: "NetGene Core Team".into(),
                capabilities: vec!["federated-learning".into(), "sync".into()],
            };
            let _ = fs::write(path.join("swarm-coordinator.json"), serde_json::to_string_pretty(&swarm_coord).unwrap());

            let trader = AgentManifest {
                id: "crypto-trader-bot".into(),
                name: "Quantum Arbitrage Trader".into(),
                version: "0.5.0-beta".into(),
                description: "Utiliza SQA e QAOA para encontrar rotas de arbitragem em micro-segundos através do Mesh.".into(),
                author: "DeFi NetGene".into(),
                capabilities: vec!["defi".into(), "qaoa-arbitrage".into()],
            };
            let _ = fs::write(path.join("crypto-trader-bot.json"), serde_json::to_string_pretty(&trader).unwrap());
        }
        
        Self { registry_path: path }
    }

    pub fn list_available_agents(&self) -> Vec<AgentManifest> {
        let mut agents = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.registry_path) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(manifest) = serde_json::from_str::<AgentManifest>(&content) {
                            agents.push(manifest);
                        }
                    }
                }
            }
        }
        agents
    }
    
    pub fn install_agent(&self, agent_id: &str) -> Result<AgentManifest> {
        let agents = self.list_available_agents();
        if let Some(agent) = agents.into_iter().find(|a| a.id == agent_id) {
            Ok(agent)
        } else {
            Err(anyhow::anyhow!("Agent {} not found in marketplace", agent_id))
        }
    }

    pub fn publish_agent(&self, json_content: &str) -> Result<AgentManifest> {
        let manifest: AgentManifest = serde_json::from_str(json_content)?;
        let file_name = format!("{}.json", manifest.id);
        fs::write(self.registry_path.join(file_name), json_content)?;
        Ok(manifest)
    }
}
