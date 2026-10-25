use netgene_kernel::{NetSphereKernel, marketplace::{Marketplace, AgentManifest}};
use netgene_store::{NetGeneStore, models::{StoredEvent, StoredNode}};
use netgene_llm::OllamaClient;
use netgene_vault::{NetGeneVault, VaultFile};
use netgene_gene::{NetGene, OsKeyring, storage::{default_gene_dir, load_gene}};
use chrono::Utc;
use uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Mutex;
use std::time::Duration;
use std::sync::Arc;
use tokio::time::sleep;

#[tauri::command]
async fn dispatch_intent(
    intent: String,
    store: State<'_, NetGeneStore>,
    kernel: State<'_, Arc<NetSphereKernel>>,
    app: AppHandle
) -> Result<String, String> {
    let res = kernel.dispatch_intent(&intent).await.map_err(|e| e.to_string())?;
    
    // Registar evento automaticamente no store
    let event = StoredEvent {
        id: Uuid::new_v4(),
        timestamp: Utc::now(),
        source: "IntentTerminal".to_string(),
        event_type: "IntentDispatched".to_string(),
        severity: "INFO".to_string(),
        details: format!("Intent: {} | Result: {}", intent, res),
    };
    
    if let Err(e) = store.save_event(&event) {
        eprintln!("Failed to save event to store: {}", e);
    } else {
        // Emitir logo via event emitter para atualizar a UI instantaneamente
        let _ = app.emit("kernel-log", format!("Saved to Store: {}", event.details));
    }

    Ok(format!("Intent dispatched: {}", res))
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
    let (publisher, kp) = load_gene(&gene_id, &default_gene_dir(), &OsKeyring::new()).map_err(|e| e.to_string())?;
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
            get_network_health,
            get_network_topology,
            get_stored_events,
            get_agents_state,
            get_safeguard_metrics,
            list_marketplace_agents,
            install_agent,
            publish_agent,
            list_genes,
            vault_store_file,
            vault_list_files,
            vault_retrieve_file,
            llm_chat
        ])
        .setup(|app| {
            let marketplace = Marketplace::new();
            app.manage(marketplace);
            
            let store = NetGeneStore::open(None)
                .expect("Failed to initialize NetGeneStore");
            app.manage(store);

            // Inicializar Kernel 1 vez e guardar no estado global
            let kernel = tauri::async_runtime::block_on(async {
                NetSphereKernel::boot().await.expect("Failed to boot NetSphereKernel")
            });
            app.manage(Arc::new(kernel));
            
            start_telemetry(app.handle().clone());

            let vault_path = app.path().home_dir()?.join(".netgene").join("vault");
            let vault = NetGeneVault::open(vault_path, &OsKeyring::new()).expect("Failed to initialize Vault");
            app.manage(Arc::new(Mutex::new(vault)));
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
