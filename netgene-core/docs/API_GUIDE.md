# 🛠️ NetGene OS — Developer & API Integration Guide (v1.0.0)

Este guia explica como importar e usar os crates do NetGene OS programaticamente em aplicações Rust externas, ou como estender o sistema core.

---

## 1. Adicionar Crates ao Projeto

Adicione os crates desejados ao seu `Cargo.toml`:

```toml
[dependencies]
# Core obrigatório
netgene-gene    = { path = "../netgene-core/crates/netgene-gene" }
netgene-kernel  = { path = "../netgene-core/crates/netgene-kernel" }

# Persistência & CRDT
netgene-store   = { path = "../netgene-core/crates/netgene-store" }

# Inteligência & Otimização
netgene-quantum = { path = "../netgene-core/crates/netgene-quantum" }
netgene-llm     = { path = "../netgene-core/crates/netgene-llm" }

# Rede & Segurança
netgene-p2p     = { path = "../netgene-core/crates/netgene-p2p" }
netgene-safeguard = { path = "../netgene-core/crates/netgene-safeguard" }

# Extensões
netgene-neural  = { path = "../netgene-core/crates/netgene-neural" }
netgene-builder = { path = "../netgene-core/crates/netgene-builder" }
netgene-dao     = { path = "../netgene-core/crates/netgene-dao" }

# Runtime obrigatório
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

---

## 2. Gene Layer — Identidade Criptográfica

```rust
use netgene_gene::{
    identity::{NetGene, GeneRole},
    storage::{save_gene, load_gene, list_genes, default_gene_dir},
    token::JwtTokenManager,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = default_gene_dir(); // ~/.netgene/genes/

    // ── Gerar Master Gene ─────────────────────────────────────────────
    let (master, master_kp) = NetGene::generate_master("Apex-Node-01")?;
    save_gene(&master, &master_kp, &dir)?;

    println!("Master Gene ID:          {}", master.id);
    println!("Master Gene Fingerprint: {}", master.fingerprint);
    println!("Master Gene Short FP:    {}", master.short_fp);
    println!("Capabilities:            {:?}", master.capabilities);

    // ── Spawn Sub-Gene ────────────────────────────────────────────────
    let caps = vec!["node.spawn".to_string(), "network.read".to_string()];
    let (sub, sub_kp) = NetGene::spawn_sub_gene(
        &master, &master_kp,
        "Edge-Node-01", GeneRole::Node, caps
    )?;
    save_gene(&sub, &sub_kp, &dir)?;

    // ── Carregar Gene do disco ─────────────────────────────────────────
    let (loaded, _kp) = load_gene(&master.id.to_string(), &dir)?;
    println!("Loaded: {}", loaded.display_line());

    // ── Listar todos os Genes ──────────────────────────────────────────
    let all = list_genes(&dir)?;
    println!("{} genes armazenados", all.len());

    // ── Emitir Token JWT de Capacidade ────────────────────────────────
    let mgr = JwtTokenManager::new();
    let token = mgr.issue(&master, &["node.spawn", "network.read"])?;
    let claims = mgr.verify(&token)?;
    println!("Token para: {} | exp: {}", claims.sub, claims.exp);

    Ok(())
}
```

---

## 3. Netsphere Kernel — Agentes & Intents

```rust
use netgene_kernel::{NetSphereKernel, IntentParser};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Boot do Kernel (5 agentes: Builder, Monitor, Optimizer, Network, Evolution)
    let kernel = NetSphereKernel::boot().await?;

    // ── Listar agentes registados
    let agents = kernel.agent_list().await;
    for agent in &agents {
        println!("  {} [{}] — status: {}", agent.name, agent.agent_type, agent.status);
    }

    // ── Dispatch de intent em linguagem natural
    let result = kernel.dispatch_intent("spawn 3 quantum nodes").await?;
    println!("Kernel: {}", result);

    let result2 = kernel.dispatch_intent("optimize routes for the mesh").await?;
    println!("Kernel: {}", result2);

    // ── Operações de memória do Kernel
    {
        let mut mem = kernel.memory.lock().await;
        let id = mem.store(
            "deployment",
            serde_json::json!({ "node_count": 3, "template": "quantum" }),
            vec!["spawn".to_string(), "quantum".to_string()],
        );
        mem.log_event("Deployed 3 quantum nodes".to_string());

        // Recuperar por ID
        let entry = mem.get(&id).unwrap();
        println!("Memory entry kind: {}", entry.kind);

        // Pesquisar por tag
        let results = mem.search_by_tag("quantum");
        println!("{} entradas com tag 'quantum'", results.len());

        // Eventos recentes
        let events = mem.recent_events(5);
        for ev in events {
            println!("Event: {}", ev);
        }
    }

    // ── Analisar intent sem dispatch
    let intent = IntentParser::parse("criar 5 nós na rede quântica");
    println!("Action: {:?} | confidence: {:.0}%", intent.action, intent.confidence * 100.0);

    // ── Shutdown gracioso
    kernel.shutdown().await?;
    Ok(())
}
```

---

## 4. Persistência Sled DB & CRDT LWW-Register

```rust
use netgene_store::{NetGeneStore, StoredNode, StoredEvent, LWWRegister};
use chrono::Utc;
use uuid::Uuid;

fn main() -> anyhow::Result<()> {
    // ── Abrir store (ou in-memory para testes)
    let store = NetGeneStore::open(None)?;           // ~/.netgene/db
    // let store = NetGeneStore::in_memory()?;       // para testes

    // ── Guardar nó
    let node = StoredNode {
        id: "node-01".to_string(),
        name: "Gateway Alpha".to_string(),
        template: "gateway".to_string(),
        ip: "10.42.0.1".to_string(),
        port: 7000,
        status: "ACTIVE".to_string(),
        last_seen: Utc::now(),
    };
    store.save_node(&node)?;

    // ── Ler nó
    if let Some(n) = store.get_node("node-01")? {
        println!("Node: {} @ {}:{}", n.name, n.ip, n.port);
    }

    // ── Listar todos os nós
    let nodes = store.list_nodes()?;
    println!("{} nós registados", nodes.len());

    // ── Guardar evento
    let event = StoredEvent {
        id: Uuid::new_v4(),
        kind: "anomaly_detected".to_string(),
        payload: serde_json::json!({ "metric": "cpu", "z_score": 3.8 }),
        timestamp: Utc::now(),
        source_gene_id: "gene-master-01".to_string(),
    };
    store.save_event(&event)?;
    let events = store.list_events(10)?;
    println!("{} eventos recentes", events.len());

    // ── Memória de Agente
    store.set_agent_memory(
        "BuilderAgent", "last_provisioned",
        serde_json::json!("node-quantum-7f3a")
    )?;
    let val = store.get_agent_memory("BuilderAgent", "last_provisioned")?;
    println!("Agent memory: {:?}", val);

    // ── CRDT LWW-Register Merge
    use chrono::Duration;
    let t1 = Utc::now();
    let t2 = t1 + Duration::milliseconds(100);

    let mut reg_a = LWWRegister::with_timestamp("status:degraded", t1, "node-A");
    let reg_b = LWWRegister::with_timestamp("status:active", t2, "node-B");
    let updated = reg_a.merge(reg_b);
    println!("CRDT merged: {} (updated={})", reg_a.value, updated);
    // Output: "status:active" (t2 > t1, node-B ganhou)

    store.flush()?;
    Ok(())
}
```

---

## 5. Módulo Quântico — QAOA, SQA & Routing

```rust
use netgene_quantum::{NetworkGraph, QAOAOptimizer, QuantumAnnealer};
use nalgebra::DMatrix;
use rand::Rng;

fn main() -> anyhow::Result<()> {
    // ── Routing Quântico em grafo de 8 nós
    let graph = NetworkGraph::demo_topology(8);
    println!("Grafo: {} nós, {} arestas", graph.node_count(), graph.edge_count());

    let nodes: Vec<String> = graph.nodes().iter().map(|n| n.id.clone()).collect();
    let result = graph.quantum_route(&nodes[0], &nodes[nodes.len()-1])?;

    println!("Algoritmo:     {}", result.algorithm);
    println!("Melhoria:      +{:.1}% vs Dijkstra clássico", result.improvement_pct);
    println!("Custo total:   {:.2}ms", result.total_cost);
    println!("Caminho:       {}", result.path.join(" → "));

    // ── QAOA em problema QUBO customizado
    let size = 8usize;
    let mut rng = rand::thread_rng();
    let q_data: Vec<f64> = (0..size*size).map(|_| rng.gen_range(-2.0..2.0)).collect();
    let q = DMatrix::from_row_slice(size, size, &q_data);

    let optimizer = QAOAOptimizer::new(3, 100); // p=3, 100 iterações
    let qaoa_result = optimizer.optimize_qubo(&q)?;
    println!("\nQAOA Resultado:");
    println!("  Objetivo:  {:.4}", qaoa_result.objective);
    println!("  Melhoria:  +{:.1}%", qaoa_result.improvement_pct);
    println!("  Solução:   {:?}", qaoa_result.solution);

    // ── Simulated Quantum Annealing standalone
    let annealer = QuantumAnnealer::new(10.0, 0.001, 500);
    let sqa_result = annealer.anneal(&q)?;
    println!("\nSQA Resultado:");
    println!("  Energia inicial: {:.4}", sqa_result.initial_energy);
    println!("  Energia final:   {:.4}", sqa_result.energy);
    println!("  Melhoria:        +{:.1}%", sqa_result.improvement_pct);

    Ok(())
}
```

---

## 6. LLM Intent Engine — Ollama Local

```rust
use netgene_llm::{OllamaClient, LlmIntentEngine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Cliente Ollama (URL e modelo opcionais — usa defaults localhost:11434 + llama3)
    let client = OllamaClient::new(None, Some("llama3"));
    let engine = LlmIntentEngine::new(client);

    // ── Parsing de intent (usa Ollama se online, fallback se offline)
    let intent = engine.parse("provision 4 quantum gateway nodes with HA").await?;
    println!("Action:      {}", intent.action);
    println!("Parameters:  {}", intent.parameters);
    println!("Explanation: {}", intent.explanation);

    // Exemplo de saída:
    // Action:      provision_nodes
    // Parameters:  {"count":4,"template":"quantum"}
    // Explanation: Provision 4 quantum nodes with high-availability template

    Ok(())
}
```

---

## 7. P2P Mesh — Publicar Mensagem

```rust
use netgene_p2p::events::MeshMessage;
use chrono::Utc;
use uuid::Uuid;

fn create_mesh_messages() {
    // ── Anúncio de nó
    let announce = MeshMessage::NodeAnnounce {
        gene_id: "gene-master-01".to_string(),
        node_name: "apex-node-01".to_string(),
        listen_addrs: vec!["/ip4/192.168.1.10/tcp/7700".to_string()],
        capabilities: vec!["quantum.run".to_string(), "node.spawn".to_string()],
        timestamp: Utc::now(),
    };

    // ── Alerta de anomalia
    let alert = MeshMessage::AnomalyAlert {
        id: Uuid::new_v4(),
        source_node: "node-03".to_string(),
        severity: "CRITICAL".to_string(),
        metric: "cpu_load".to_string(),
        value: 99.8,
        timestamp: Utc::now(),
    };

    // ── Broadcast de intent
    let intent = MeshMessage::IntentBroadcast {
        sender: "gene-master-01".to_string(),
        intent: "spawn 3 quantum nodes".to_string(),
        timestamp: Utc::now(),
    };

    // Serialize para JSON (para Gossipsub)
    let json = serde_json::to_vec(&announce).unwrap();
    println!("Mensagem serializada: {} bytes", json.len());
}
```

---

## 8. Safeguard — Deteção & Auto-Remediação

```rust
use netgene_safeguard::{AnomalyDetector, SelfHealingEngine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut detector = AnomalyDetector::new(50, 2.5); // window=50, z_threshold=2.5
    let mut healer = SelfHealingEngine::new(true);    // auto_heal=true

    // ── Alimentar valores normais (warm-up mínimo de 10 amostras)
    for i in 0..30 {
        let noise = if i % 2 == 0 { 0.5 } else { -0.5 };
        detector.ingest("cpu_load", 20.0 + noise);
    }

    // ── Injetar anomalia
    if let Some(anomaly) = detector.ingest("cpu_load", 200.0) {
        println!("⚠️  Anomalia detetada!");
        println!("   Métrica:   {}", anomaly.metric);
        println!("   Valor:     {:.2}", anomaly.value);
        println!("   Z-Score:   {:.2}", anomaly.z_score);
        println!("   Severidade: {}", anomaly.severity);

        // ── Avaliação da ação de remediação
        let action = healer.evaluate(&anomaly);
        println!("   Ação:      {}", action);

        // ── Aplicar remediação
        let result = healer.apply(action).await?;
        println!("   Resultado: {} (success={})", result.notes, result.success);
    }

    // ── Ciclo auto-completo
    if let Some(anomaly) = detector.ingest("cpu_load", 500.0) {
        if let Some(result) = healer.auto_heal_from_anomaly(&anomaly).await {
            println!("Auto-heal: {} curas aplicadas", healer.heal_count());
        }
    }

    // ── Estatísticas
    println!("Total de anomalias:  {}", detector.events.len());
    println!("Anomalias críticas:  {}", detector.critical_count());
    println!("Curas aplicadas:     {}", healer.heal_count());

    Ok(())
}
```

---

## 9. BCI Neural Adapter — Stream EEG

```rust
use netgene_neural::NeuralStreamAdapter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (adapter, _rx) = NeuralStreamAdapter::new();

    // ── Processar leitura EEG (beta=0.85, gamma=0.70)
    let event = adapter.process_signal("node-01", 0.85, 0.70).await?;

    println!("Nó alvo:        {}", event.target_node);
    println!("Beta Power:     {:.2}", event.beta_power);
    println!("Gamma Power:    {:.2}", event.gamma_power);
    println!("Carga Cognitiva: {:.1}%", event.cognitive_load * 100.0);
    println!("Ação Convertida: {}", event.converted_action);
    // Output: "OPTIMIZE_ROUTE" (beta > 0.75, gamma > 0.70)

    Ok(())
}
```

---

## 10. DAO — Governance & Proof-of-Utility

```rust
use netgene_dao::{GovernanceEngine, ProposalType, ProofOfUtilityEngine};

fn main() -> anyhow::Result<()> {
    let engine = GovernanceEngine::new(66.0); // quórum: 66%

    // ── Submeter proposta
    let mut proposal = engine.submit_proposal(
        "Upgrade QAOA Layers to 5",
        "Improve quantum routing accuracy by 20% in dense networks",
        "gene-master-01",
        ProposalType::QuantumWeightUpdate { default_layers: 5 },
    );
    println!("Proposta: {} [{}]", proposal.title, proposal.status);

    // ── Votar (com prevenção de voto duplo)
    engine.vote(&mut proposal, "gene-node-02", true, 100)?;
    engine.vote(&mut proposal, "gene-node-03", true, 50)?;

    println!("Votos YES: {} | Status: {}", proposal.votes_yes, proposal.status);
    // Status → "PASSED" quando votes_yes >= 100

    // ── Calcular recompensa Proof-of-Utility
    let receipt = ProofOfUtilityEngine::calculate_reward(
        "node-alpha-01",
        1000,   // qpu_shots
        50000,  // packets_inspected
    );
    println!("Recompensa GENE: {:.2} tokens", receipt.total_utility_tokens);
    // = (1000 × 0.05) + (50000 × 0.001) = 50 + 50 = 100 tokens

    Ok(())
}
```

---

## 11. Builder Engine — Provisionamento por Intent

```rust
use netgene_builder::{BuilderEngine, NodeTemplate};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut engine = BuilderEngine::new();

    // ── Provisionar por tipo
    let nodes = engine.provision(NodeTemplate::Quantum, 3).await?;
    for n in &nodes {
        println!("Nó: {} @ {}:{} config={}", n.id, n.ip, n.port, n.config);
    }

    // ── Provisionar por intent em NL
    let summary = engine.from_intent("spawn 2 gateway nodes").await?;
    println!("{}", summary);

    // ── Listar todos provisionados
    println!("{} nós provisionados no total", engine.list().len());

    Ok(())
}
```

---

## 12. Testing — Padrões Recomendados

```rust
// ── Testes síncronos (sem Tokio)
#[test]
fn test_lww_crdt_merge() {
    use netgene_store::LWWRegister;
    use chrono::{Utc, Duration};

    let t1 = Utc::now();
    let t2 = t1 + Duration::milliseconds(100);
    let mut reg = LWWRegister::with_timestamp("old", t1, "node-A");
    let newer = LWWRegister::with_timestamp("new", t2, "node-B");
    assert!(reg.merge(newer));
    assert_eq!(reg.value, "new");
}

// ── Testes assíncronos (com Tokio)
#[tokio::test]
async fn test_kernel_boots_with_five_agents() {
    let kernel = NetSphereKernel::boot().await.unwrap();
    assert_eq!(kernel.agent_list().await.len(), 5);
    kernel.shutdown().await.unwrap();
}

// ── Testes in-memory store (sem disco)
#[test]
fn test_store_node_roundtrip() -> anyhow::Result<()> {
    use netgene_store::{NetGeneStore, StoredNode};
    let store = NetGeneStore::in_memory()?;
    let node = StoredNode {
        id: "test-node".to_string(),
        name: "Test".to_string(),
        template: "edge".to_string(),
        ip: "10.0.0.1".to_string(),
        port: 7000,
        status: "ACTIVE".to_string(),
        last_seen: chrono::Utc::now(),
    };
    store.save_node(&node)?;
    assert!(store.get_node("test-node")?.is_some());
    Ok(())
}
```

---

## 13. Exemplos de Composição de Crates

### Pipeline completo: Gene → Kernel → Quantum → DAO

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use netgene_gene::identity::NetGene;
    use netgene_kernel::NetSphereKernel;
    use netgene_quantum::NetworkGraph;
    use netgene_dao::{GovernanceEngine, ProposalType, ProofOfUtilityEngine};

    // 1. Criar identidade
    let (master, _kp) = NetGene::generate_master("Apex-Operator")?;
    println!("Gene: {} | fp:{}", master.id, master.short_fp);

    // 2. Boot kernel e dispatch intent
    let kernel = NetSphereKernel::boot().await?;
    kernel.dispatch_intent("spawn 5 quantum nodes with HA").await?;

    // 3. Otimização quântica de rotas
    let graph = NetworkGraph::demo_topology(10);
    let nodes: Vec<_> = graph.nodes().iter().map(|n| n.id.clone()).collect();
    let route = graph.quantum_route(&nodes[0], &nodes[nodes.len()-1])?;
    println!("Rota quântica: {} (+{:.1}%)", route.path.join("→"), route.improvement_pct);

    // 4. Proposta de governança
    let dao = GovernanceEngine::new(66.0);
    let mut prop = dao.submit_proposal(
        "Adopt new routing result",
        &format!("Accept quantum route with {:.1}% improvement", route.improvement_pct),
        &master.id.to_string(),
        ProposalType::QuantumWeightUpdate { default_layers: 5 },
    );
    dao.vote(&mut prop, "gene-voter-01", true, 100)?;
    println!("Proposta: {}", prop.status);

    // 5. Calcular tokens de utilidade
    let receipt = ProofOfUtilityEngine::calculate_reward("node-01", 500, 25000);
    println!("GENE tokens: {:.2}", receipt.total_utility_tokens);

    kernel.shutdown().await?;
    Ok(())
}
```
