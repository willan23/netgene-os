//! Executors for the catalog actions.
//!
//! Each approved action does real, verifiable work and returns its result:
//!
//! | Action | Effect |
//! |---|---|
//! | `system.status` | Operator, policy, audit, genes, agents and store state |
//! | `safeguard.scan` | Samples host CPU, memory and process count into the anomaly detector |
//! | `node.provision` | Creates sub-Genes of the operator (hybrid lineage, keys in the secret store) |
//! | `agent.list` | Signed WASM agents installed in `~/.netgene/agents` and whether they are trusted |
//! | `agent.run` | Runs one of them in the sandbox with the permissions policy grants; its KV persists |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::storage::{list_genes, save_gene, trusted_lineage};
use netgene_gene::{GeneRole, HybridPublicKey, MemorySecretStore, NetGene, SecretStore};
use netgene_safeguard::AnomalyDetector;
use netgene_store::{NetGeneStore, StoredNode};
use netgene_wasm::{ExecutionInput, GeneModule, WasmSandbox, DEFAULT_FUEL};

use crate::marketplace::validate_agent_id;

const AGENT_MEMORY_LIMIT: usize = 16 * 1024 * 1024;
const MODULE_SUFFIX: &str = ".gene-module.json";
/// Capabilities a provisioned node gene starts with.
const NODE_CAPABILITIES: &[&str] = &["network.read"];

/// Where actions persist their effects.
#[derive(Clone)]
pub struct Workspace {
    /// Gene records (`~/.netgene/genes`); `None` keeps provisioned genes in memory.
    pub gene_dir: Option<PathBuf>,
    pub secrets: Arc<dyn SecretStore>,
    /// Installed WASM agents (`~/.netgene/agents/<name>.gene-module.json`).
    pub agents_dir: Option<PathBuf>,
    pub store: NetGeneStore,
}

impl Workspace {
    /// Nothing touches the disk (tests, demos).
    pub fn ephemeral() -> Result<Self> {
        Ok(Self { gene_dir: None, secrets: Arc::new(MemorySecretStore::new()), agents_dir: None, store: NetGeneStore::in_memory()? })
    }

    /// The standard layout under a NetGene home directory.
    pub fn at(home: &Path, gene_dir: PathBuf, secrets: Arc<dyn SecretStore>, store: NetGeneStore) -> Self {
        Self { gene_dir: Some(gene_dir), secrets, agents_dir: Some(home.join("agents")), store }
    }

    pub fn agent_path(&self, name: &str) -> Result<PathBuf> {
        validate_agent_id(name)?;
        let dir = self.agents_dir.as_ref().context("This kernel has no agents directory")?;
        Ok(dir.join(format!("{name}{MODULE_SUFFIX}")))
    }
}

/// Copy a signed module into the agents directory after checking that a trusted gene signed it.
pub fn install_agent(workspace: &Workspace, trusted: &[HybridPublicKey], module: &GeneModule) -> Result<PathBuf> {
    verifier(trusted.to_vec()).verify(module)?;
    let path = workspace.agent_path(&module.name)?;
    std::fs::create_dir_all(path.parent().context("agents dir")?)?;
    std::fs::write(&path, serde_json::to_vec_pretty(module)?)?;
    Ok(path)
}

/// Checks trust and signature only (any declared permission is acceptable here).
fn verifier(trusted: Vec<HybridPublicKey>) -> WasmSandbox {
    WasmSandbox::new(AGENT_MEMORY_LIMIT, trusted).allow_permissions(netgene_wasm::PERMISSIONS.iter().map(|(p, _)| *p))
}

pub(crate) struct Executor {
    pub workspace: Workspace,
    pub operator: NetGene,
    pub operator_kp: GeneKeyPair,
    /// Genes provisioned by an ephemeral kernel.
    pub in_memory_genes: Vec<NetGene>,
    pub detector: AnomalyDetector,
    pub system: sysinfo::System,
}

impl Executor {
    pub fn new(workspace: Workspace, operator: NetGene, operator_kp: GeneKeyPair) -> Self {
        Self {
            workspace,
            operator,
            operator_kp,
            in_memory_genes: vec![],
            detector: AnomalyDetector::default(),
            system: sysinfo::System::new(),
        }
    }

    fn genes(&self) -> Result<Vec<NetGene>> {
        let mut genes = match &self.workspace.gene_dir {
            Some(dir) => list_genes(dir)?,
            None => vec![],
        };
        genes.extend(self.in_memory_genes.iter().cloned());
        if !genes.iter().any(|g| g.id == self.operator.id) {
            genes.push(self.operator.clone());
        }
        Ok(genes)
    }

    /// Keys allowed to author agents: the operator and its lineage.
    pub fn trusted_authors(&self) -> Result<Vec<HybridPublicKey>> {
        Ok(trusted_lineage(&self.operator, &self.genes()?).iter().map(HybridPublicKey::from_gene).collect())
    }

    pub fn status(&self, extra: Value) -> Result<Value> {
        let genes = self.genes()?;
        let agents = self.installed_agents()?;
        Ok(json!({
            "operator": { "id": self.operator.id, "name": self.operator.name, "role": self.operator.role, "fingerprint": self.operator.short_fp },
            "genes": genes.len(),
            "agents_installed": agents.len(),
            "store": { "nodes": self.workspace.store.list_nodes()?.len(), "events": self.workspace.store.list_events(10_000)?.len() },
            "kernel": extra,
        }))
    }

    /// Feed one metric sample (from telemetry or eBPF) into the detector.
    pub fn observe(&mut self, metric: &str, value: f64) -> Option<netgene_safeguard::AnomalyEvent> {
        self.detector.ingest(metric, value)
    }

    pub async fn scan(&mut self) -> Result<Value> {
        // CPU usage needs two refreshes some time apart.
        self.system.refresh_cpu_usage();
        tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let total = self.system.total_memory().max(1) as f64;
        let samples = BTreeMap::from([
            ("host.cpu_percent", self.system.global_cpu_usage() as f64),
            ("host.memory_percent", self.system.used_memory() as f64 / total * 100.0),
            ("host.processes", self.system.processes().len() as f64),
        ]);
        let anomalies: Vec<_> = samples.iter().filter_map(|(metric, value)| self.observe(metric, *value)).collect();
        Ok(json!({
            "samples": samples,
            "anomalies": anomalies,
            "note": "anomalies are flagged once a metric has 10 baseline samples (one per scan or telemetry tick)",
        }))
    }

    pub fn provision(&mut self, params: &Value) -> Result<Value> {
        let count = params.get("count").and_then(Value::as_u64).unwrap_or(1);
        let template = params.get("template").and_then(Value::as_str).unwrap_or("node");
        validate_agent_id(template).context("template must be a short lowercase name")?;

        let mut created = vec![];
        for _ in 0..count {
            let name = format!("{}-{}", template, &uuid::Uuid::new_v4().simple().to_string()[..8]);
            let caps = NODE_CAPABILITIES.iter().map(|c| c.to_string()).collect();
            let (gene, kp) = NetGene::spawn_sub_gene(&self.operator, &self.operator_kp, &name, GeneRole::Node, caps)?;
            match &self.workspace.gene_dir {
                Some(dir) => save_gene(&gene, &kp, dir, self.workspace.secrets.as_ref())?,
                None => self.in_memory_genes.push(gene.clone()),
            }
            self.workspace.store.save_node(&StoredNode {
                id: gene.id.to_string(),
                name: gene.name.clone(),
                template: template.to_string(),
                ip: String::new(),
                port: 0,
                status: "provisioned".into(),
                last_seen: chrono::Utc::now(),
            })?;
            created.push(json!({ "id": gene.id, "name": gene.name, "fingerprint": gene.short_fp, "parent": self.operator.id }));
        }
        Ok(json!({ "created": created, "persisted": self.workspace.gene_dir.is_some() }))
    }

    fn installed_agents(&self) -> Result<Vec<GeneModule>> {
        let Some(dir) = &self.workspace.agents_dir else { return Ok(vec![]) };
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let mut modules = vec![];
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.to_string_lossy().ends_with(MODULE_SUFFIX) {
                match std::fs::read(&path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<GeneModule>(&b)?)) {
                    Ok(module) => modules.push(module),
                    Err(e) => tracing::warn!("Skipping unreadable agent {:?}: {}", path, e),
                }
            }
        }
        modules.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(modules)
    }

    pub fn list_agents(&self) -> Result<Value> {
        let sandbox = verifier(self.trusted_authors()?);
        let agents: Vec<Value> = self
            .installed_agents()?
            .iter()
            .map(|m| json!({
                "name": m.name,
                "version": m.version,
                "permissions": m.permissions,
                "author": m.author_key.short_fingerprint(),
                "trusted": sandbox.verify(m).is_ok(),
            }))
            .collect();
        Ok(json!({ "agents": agents }))
    }

    pub async fn run_agent(&self, params: &Value, allowed: &[String]) -> Result<Value> {
        let name = params.get("agent").and_then(Value::as_str).context("agent.run needs an 'agent' name")?;
        let input = params.get("input").and_then(Value::as_str).unwrap_or_default().as_bytes().to_vec();
        let path = self.workspace.agent_path(name)?;
        let module: GeneModule = serde_json::from_slice(&std::fs::read(&path).with_context(|| format!("Agent '{}' is not installed", name))?)?;

        // A module only runs if policy grants every permission it declared.
        if let Some(missing) = module.permissions.iter().find(|p| !allowed.contains(p)) {
            anyhow::bail!("Policy does not grant permission '{}' requested by agent '{}' (see agent_permissions in policy.json)", missing, name);
        }
        let granted = module.permissions.clone();
        let sandbox = WasmSandbox::new(AGENT_MEMORY_LIMIT, self.trusted_authors()?).with_fuel(DEFAULT_FUEL).allow_permissions(granted.clone());

        // The agent's key-value state survives between runs.
        let store = self.workspace.store.clone();
        let memory_key = module.module_id.to_string();
        let kv = load_kv(&store, &memory_key)?;
        let result = tokio::task::spawn_blocking(move || sandbox.execute(&module, ExecutionInput { input, kv })).await??;
        save_kv(&store, &memory_key, &result.kv)?;

        Ok(json!({
            "agent": name,
            "return_value": result.return_value,
            "output": String::from_utf8_lossy(&result.output),
            "logs": result.logs,
            "fuel_consumed": result.fuel_consumed,
            "execution_time_ms": result.execution_time_ms,
            "permissions_granted": granted,
            "kv_keys": result.kv.len(),
        }))
    }
}

fn load_kv(store: &NetGeneStore, agent: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let Some(Value::Object(map)) = store.get_agent_memory(agent, "kv")? else { return Ok(BTreeMap::new()) };
    map.into_iter()
        .map(|(k, v)| Ok((k, BASE64.decode(v.as_str().context("agent KV value is not a string")?)?)))
        .collect()
}

fn save_kv(store: &NetGeneStore, agent: &str, kv: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let map: serde_json::Map<String, Value> = kv.iter().map(|(k, v)| (k.clone(), Value::String(BASE64.encode(v)))).collect();
    store.set_agent_memory(agent, "kv", Value::Object(map))
}
