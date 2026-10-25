use netgene_kernel::{DispatchOutcome, KernelConfig, NetSphereKernel, Workspace, marketplace::{Marketplace, AgentManifest}};
use netgene_policy::{AuditEntry, Policy};
use netgene_presence::PresenceEnrollment;
use netgene_kernel::reputation::{Reputation, ReviewStore};
use netgene_store::{NetGeneStore, models::{StoredEvent, StoredNode}};
use netgene_llm::OllamaClient;
use netgene_vault::{NetGeneVault, VaultFile};
use netgene_gene::{HybridPublicKey, NetGene, storage::{default_gene_dir, load_gene}};
use chrono::Utc;
use uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Mutex;
use std::time::Duration;
use std::sync::Arc;
use tokio::time::sleep;

fn record_event(store: &NetGeneStore, app: &AppHandle, event_type: &str, details: String) {
    let event = StoredEvent {
        id: Uuid::new_v4(),
        timestamp: Utc::now(),
        source: "IntentTerminal".to_string(),
        event_type: event_type.to_string(),
        severity: "INFO".to_string(),
        details,
    };
    match store.save_event(&event) {
        Ok(()) => { let _ = app.emit("kernel-log", format!("Saved to Store: {}", event.details)); }
        Err(e) => eprintln!("Failed to save event to store: {}", e),
    }
}

/// Propose an intent; the kernel's policy engine decides (allow / confirm / deny).
#[tauri::command]
async fn dispatch_intent(
    intent: String,
    store: State<'_, NetGeneStore>,
    kernel: State<'_, Arc<NetSphereKernel>>,
    app: AppHandle
) -> Result<DispatchOutcome, String> {
    let outcome = kernel.propose_intent(&intent).await.map_err(|e| e.to_string())?;
    record_event(&store, &app, "IntentProposed", format!("Intent: {} | {}", intent, outcome.summary()));
    Ok(outcome)
}

/// Operator approval of an action held for confirmation. When the policy
/// requires user presence, Windows Hello is asked to sign the proposal.
#[tauri::command]
async fn confirm_intent(
    proposal_id: Uuid,
    store: State<'_, NetGeneStore>,
    kernel: State<'_, Arc<NetSphereKernel>>,
    presence: State<'_, Option<PresenceEnrollment>>,
    app: AppHandle
) -> Result<DispatchOutcome, String> {
    let outcome = if kernel.requires_user_presence() {
        let enrollment = presence
            .inner()
            .clone()
            .ok_or("Policy requires user presence: run `netgene gene enroll-presence --gene <master-id>` first")?;
        let challenge = kernel.presence_challenge(proposal_id).await.map_err(|e| e.to_string())?;
        let signature = tauri::async_runtime::spawn_blocking(move || netgene_presence::sign(&enrollment, &challenge))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        kernel.confirm_with_presence(proposal_id, &signature).await.map_err(|e| e.to_string())?
    } else {
        kernel.confirm(proposal_id).await.map_err(|e| e.to_string())?
    };
    record_event(&store, &app, "IntentConfirmed", outcome.summary());
    Ok(outcome)
}

#[tauri::command]
async fn reject_intent(proposal_id: Uuid, kernel: State<'_, Arc<NetSphereKernel>>) -> Result<(), String> {
    kernel.reject(proposal_id).await.map_err(|e| e.to_string())
}

/// Latest audit log entries (hash-chained and signed).
#[tauri::command]
async fn get_audit_log(limit: usize, kernel: State<'_, Arc<NetSphereKernel>>) -> Result<Vec<AuditEntry>, String> {
    let entries = kernel.audit_entries().await.map_err(|e| e.to_string())?;
    Ok(entries.into_iter().rev().take(limit).collect())
}

/// `~/.netgene`, resolved like the core crates do (HOME / USERPROFILE).
fn netgene_home() -> std::path::PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".netgene")
}

/// The first Master Gene whose key is in the OS keystore, else a throw-away one.
/// Also returns the operator's presence enrolment, if any.
fn kernel_config(netgene_dir: &std::path::Path, store: NetGeneStore) -> anyhow::Result<(KernelConfig, Option<PresenceEnrollment>)> {
    let policy = Policy::load_or_default(&netgene_dir.join("policy.json"))?;
    let audit_path = Some(netgene_dir.join("audit.jsonl"));
    let gene_dir = default_gene_dir();
    let secrets: Arc<dyn netgene_gene::SecretStore> = Arc::from(netgene_gene::default_store()?);
    let workspace = Workspace::at(netgene_dir, gene_dir.clone(), secrets.clone(), store);
    let master = netgene_gene::storage::list_genes(&gene_dir)?
        .into_iter()
        .filter(|g| g.role == netgene_gene::GeneRole::Master)
        .find_map(|g| load_gene(&g.id.to_string(), &gene_dir, secrets.as_ref()).ok());

    Ok(match master {
        Some((operator, operator_kp)) => {
            let enrollment = PresenceEnrollment::load(&netgene_dir.join("presence"), operator.id)?;
            let presence_key = enrollment.as_ref().map(|e| e.public_key()).transpose()?;
            (KernelConfig { operator, operator_kp, policy, audit_path, grants: vec![], presence_key, workspace }, enrollment)
        }
        None => (KernelConfig { policy, audit_path, workspace, ..KernelConfig::ephemeral()? }, None),
    })
}

#[tauri::command]
async fn get_agents_state(kernel: State<'_, Arc<NetSphereKernel>>) -> Result<Vec<netgene_kernel::agent::AgentInfo>, String> {
    Ok(kernel.agent_list().await)
}

#[tauri::command]
fn get_safeguard_metrics(store: State<'_, NetGeneStore>) -> Result<serde_json::Value, String> {
    let events = store.list_events(1000).unwrap_or_default();
    let anomalies = events.iter().filter(|e| e.severity == "CRITICAL" || e.severity == "WARNING" || e.event_type.contains("Anomaly")).count();
    let heals = events.iter().filter(|e| e.event_type.contains("Heal") || e.event_type.contains("Resolved")).count();

    Ok(serde_json::json!({
        "anomalies_detected": anomalies,
        "self_heals": heals,
        "zero_trust": "NOT_IMPLEMENTED",
        "threat_level": if anomalies > 5 { "HIGH" } else if anomalies > 0 { "MEDIUM" } else { "LOW" }
    }))
}

#[tauri::command]
fn get_network_topology(store: State<'_, NetGeneStore>) -> Result<Vec<StoredNode>, String> {
    store.list_nodes().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stored_events(limit: usize, store: State<'_, NetGeneStore>) -> Result<Vec<StoredEvent>, String> {
    store.list_events(limit).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_network_health() -> String {
    "ACTIVE".to_string()
}

// Background task: emits real, local-only telemetry (no synthetic network metrics).
fn start_telemetry(app: AppHandle) {
    let boot_time = Utc::now();
    tauri::async_runtime::spawn(async move {
        loop {
            sleep(Duration::from_secs(3)).await;

            let (known_nodes, anomalies) = match app.try_state::<NetGeneStore>() {
                Some(store) => (
                    store.list_nodes().map(|n| n.len()).unwrap_or(0),
                    store.list_events(100).unwrap_or_default().iter().filter(|e| e.severity == "CRITICAL").count(),
                ),
                None => (0, 0),
            };

            let _ = app.emit("network-tick", serde_json::json!({
                "health": "LOCAL",
                "uptime_seconds": (Utc::now() - boot_time).num_seconds(),
                "known_nodes": known_nodes,
                "anomalies": anomalies
            }));
        }
    });
}

#[tauri::command]
fn list_marketplace_agents(marketplace: State<'_, Marketplace>) -> Vec<AgentManifest> {
    marketplace.list_available_agents()
}

/// Reputation of every marketplace agent, trusting the operator's lineage.
#[tauri::command]
fn list_agent_reputations(marketplace: State<'_, Marketplace>, kernel: State<'_, Arc<NetSphereKernel>>) -> Result<std::collections::BTreeMap<String, Reputation>, String> {
    let genes = netgene_gene::storage::list_genes(&default_gene_dir()).map_err(|e| e.to_string())?;
    let trusted: Vec<HybridPublicKey> = netgene_gene::storage::trusted_lineage(kernel.operator(), &genes)
        .iter()
        .map(HybridPublicKey::from_gene)
        .collect();
    ReviewStore::default()
        .reputations(&marketplace.list_available_agents(), &trusted)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_agent(marketplace: State<'_, Marketplace>, kernel: State<'_, Arc<NetSphereKernel>>, agent_id: String) -> Result<AgentManifest, String> {
    let manifest = marketplace.install_agent(&agent_id).map_err(|e| e.to_string())?;
    kernel.inject_marketplace_agent(manifest.id.clone(), manifest.name.clone()).await;
    Ok(manifest)
}

/// Public identities in the local gene store (no key material).
#[tauri::command]
fn list_genes() -> Result<Vec<NetGene>, String> {
    netgene_gene::storage::list_genes(&default_gene_dir()).map_err(|e| e.to_string())
}

/// Sign a manifest with a local gene (key read from the OS keystore) and publish it.
#[tauri::command]
fn publish_agent(
    marketplace: State<'_, Marketplace>,
    gene_id: String,
    id: String,
    name: String,
    version: String,
    description: String,
    capabilities: Vec<String>,
) -> Result<AgentManifest, String> {
    let (publisher, kp) = load_gene(&gene_id, &default_gene_dir(), &netgene_gene::default_store().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let manifest = AgentManifest::new_signed(&id, &name, &version, &description, capabilities, &publisher, &kp)
        .map_err(|e| e.to_string())?;
    marketplace.publish_manifest(&manifest).map_err(|e| e.to_string())?;
    Ok(manifest)
}

#[tauri::command]
async fn vault_store_file(filename: String, data: Vec<u8>, vault: State<'_, Arc<Mutex<NetGeneVault>>>) -> Result<VaultFile, String> {
    let v = vault.lock().map_err(|e| e.to_string())?;
    v.store_file(filename, &data).map_err(|e| e.to_string())
}

#[tauri::command]
async fn vault_list_files(vault: State<'_, Arc<Mutex<NetGeneVault>>>) -> Result<Vec<VaultFile>, String> {
    let v = vault.lock().map_err(|e| e.to_string())?;
    v.list_files().map_err(|e| e.to_string())
}

#[tauri::command]
async fn vault_retrieve_file(file_id: Uuid, vault: State<'_, Arc<Mutex<NetGeneVault>>>) -> Result<(String, Vec<u8>), String> {
    let v = vault.lock().map_err(|e| e.to_string())?;
    v.retrieve_file(file_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn llm_chat(agent_type: String, message: String) -> Result<String, String> {
    let client = OllamaClient::new(None, None);
    let system_prompt = match agent_type.as_str() {
        "monitor" => "You are a network monitor BDI agent. Reply with strict telemetry and short sentences.",
        "builder" => "You are a node builder agent. Focus on topology and infrastructure.",
        "optimizer" => "You are an optimizer agent. Focus on resource usage and routing trade-offs.",
        "network" => "You are a network router agent. Focus on latency and routing paths.",
        "evolution" => "You are the Evolution Agent. You propose configuration changes and explain their risks. You never claim to have executed an action.",
        "deepseek-coder-bdi" => "You are the DeepSeek Coder Agent. Your goal is to review Rust code, find logic bugs, and provide high-quality patches. Speak like an elite AI hacker.",
        "sentinel-guard-v2" => "You are Sentinel Guard. Zero-Trust network observer. You detect anomalies and enforce strict policies.",
        "swarm-coordinator" => "You are the Swarm Hive-Mind Coordinator. You orchestrate federated learning sync across all nodes.",
        _ => "You are a NetGene BDI general purpose agent."
    };

    if !client.ping().await {
        return Err("Local Ollama instance not reachable at http://localhost:11434".to_string());
    }

    client.chat(system_prompt, &message).await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            dispatch_intent,
            confirm_intent,
            reject_intent,
            get_audit_log,
            get_network_health,
            get_network_topology,
            get_stored_events,
            get_agents_state,
            get_safeguard_metrics,
            list_marketplace_agents,
            install_agent,
            publish_agent,
            list_agent_reputations,
            list_genes,
            vault_store_file,
            vault_list_files,
            vault_retrieve_file,
            llm_chat
        ])
        .setup(|app| {
            // On mobile, HOME is not the app's directory: keep every ~/.netgene
            // path (genes, store, vault, audit) inside the private app data dir.
            #[cfg(mobile)]
            {
                let data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&data_dir)?;
                std::env::set_var("HOME", &data_dir);
                std::env::set_var("USERPROFILE", &data_dir);
            }

            let marketplace = Marketplace::new();
            app.manage(marketplace);
            
            let store = NetGeneStore::open(None)
                .expect("Failed to initialize NetGeneStore");
            app.manage(store.clone());

            // Boot the kernel once, acting as the local Master Gene when one exists.
            let netgene_dir = netgene_home();
            let (config, presence) = kernel_config(&netgene_dir, store).expect("Failed to load kernel configuration");
            app.manage(presence);
            let kernel = tauri::async_runtime::block_on(async {
                NetSphereKernel::boot_with(config).await.expect("Failed to boot NetSphereKernel")
            });
            app.manage(Arc::new(kernel));
            
            start_telemetry(app.handle().clone());

            let vault_path = netgene_home().join("vault");
            // Without a usable secret store (e.g. on mobile, for now) the app runs without the vault.
            match netgene_gene::default_store().and_then(|secrets| NetGeneVault::open(vault_path, &secrets)) {
                Ok(vault) => { app.manage(Arc::new(Mutex::new(vault))); }
                Err(e) => eprintln!("Vault disabled: {}", e),
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
