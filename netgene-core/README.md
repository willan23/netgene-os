# 🧬 NetGene OS — Living, Self-Evolving Distributed Network OS

```text
╔═══════════════════════════════════════════════════════╗
║     🧬  NetGene OS  v0.3 — Megastructure Terminal     ║
║   Living · Self-Evolving · Quantum-Enhanced Network   ║
╚═══════════════════════════════════════════════════════╝
```

**NetGene OS** is an autonomous, self-evolving, distributed network operating system written in Rust. It combines multi-agent collective intelligence, quantum-inspired optimization, libp2p mesh networking, local LLM intent parsing (Ollama `llama3`), BCI neural stream adapters, and post-quantum cryptographic identity.

---

## 🏛️ System Architecture

```text
                               ┌────────────────────────────────────────────────────────┐
                               │           NetGene Command Center & Dashboard           │
                               │        TUI (Ratatui)  ·  3D WebGL (Three.js)          │
                               └───────────────────────────┬────────────────────────────┘
                                                           │
┌──────────────────────────────────────────────────────────▼──────────────────────────────────────────────────────────┐
│                                                 Netsphere Kernel                                                    │
│ ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌─────────────────────┐ │
│ │   BuilderAgent   │  │   MonitorAgent   │  │  OptimizerAgent  │  │   NetworkAgent   │  │   EvolutionAgent    │ │
│ └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  └──────────┬──────────┘ │
│          └─────────────────────┼─────────────────────┼─────────────────────┼───────────────────────┘           │
│                                │           Async Message Bus (Tokio)       │                                   │
└────────────────────────────────┼─────────────────────┼─────────────────────┼───────────────────────────────────┘
                                 │                     │                     │
      ┌──────────────────────────┴───────┐   ┌─────────┴─────────────┐   ┌───┴──────────────────────────────┐
      │   📂 netgene-store (Sled DB)     │   │ 🧠 netgene-llm        │   │ 🌐 netgene-p2p (libp2p Mesh)     │
      │   ACID & CRDT LWW-Register       │   │ Ollama (llama3)       │   │ Gossipsub, Kademlia, mDNS        │
      └──────────────────────────────────┘   └───────────────────────┘   └──────────────────────────────────┘
                                 │                     │                     │
      ┌──────────────────────────┴───────┐   ┌─────────┴─────────────┐   ┌───┴──────────────────────────────┐
      │   🔑 netgene-gene                │   │ ⚛️ netgene-quantum    │   │ 🧠 netgene-neural                │
      │   Ed25519 & PQC (Kyber/Dilithium)│   │ QAOA, SQA & QPU Cloud │   │ OpenBCI EEG Signal Stream        │
      └──────────────────────────────────┘   └───────────────────────┘   └──────────────────────────────────┘
```

---

## 📦 Workspace Crates (21 Crates)

| Crate | Purpose |
|---|---|
| [`netgene-gene`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-gene) | Cryptographic identity, Master & Sub-Genes, Ed25519 & Post-Quantum Cryptography (ML-KEM / ML-DSA). |
| [`netgene-kernel`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-kernel) | Collective intelligence multi-agent orchestrator (`Builder`, `Monitor`, `Optimizer`, `Network`, `Evolution`). |
| [`netgene-quantum`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-quantum) | QAOA-sim, Simulated Quantum Annealing (SQA), QUBO solver, and AWS Braket / IBM Q Cloud adapter. |
| [`netgene-safeguard`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-safeguard) | Z-score anomaly detector and self-healing state machine engine. |
| [`netgene-builder`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-builder) | Intent-based organic node provisioning engine. |
| [`netgene-store`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-store) | Sled DB persistent storage with CRDT LWW-Register synchronization. |
| [`netgene-llm`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-llm) | Local Ollama (`llama3`) client and Intent Engine 2.0. |
| [`netgene-p2p`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-p2p) | libp2p mesh network node (Gossipsub `netgene-mesh-v1`, mDNS, Kademlia DHT, Identify). |
| [`netgene-neural`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-neural) | Brain-Computer Interface (BCI) OpenBCI EEG telemetry & thought-to-action pipeline. |
| [`netgene-wasm`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-wasm) | WebAssembly sandbox and gene module verifier. |
| [`netgene-ebpf`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-ebpf) | eBPF kernel security probes and high-performance telemetry. |
| [`netgene-k8s`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-k8s) | Kubernetes CRDs Operator for organic scale-out. |
| [`netgene-qpu`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-qpu) | OpenQASM 3.0 Transpiler and physical QPU connectors. |
| [`netgene-mobile`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-mobile) | Mobile PWA backend and WebAuthn Passkey engine. |
| [`netgene-dao`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-dao) | P2P Autonomous Governance and Proof-of-Utility tokenomics. |
| [`netgene-tui`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-tui) | Terminal Dashboard application built with Ratatui & Crossterm. |
| [`netgene-cloud`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-cloud) | Cloud P2P Mesh Node for wide-area routing. |
| [`netgene-lite`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-lite) | IoT Lite Node for ESP32 and constrained environments. |
| [`netgene-tpm`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-tpm) | TPM 2.0 Hardware Enclave Integration for Master Gene sealing. |
| [`netgene-vault`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-vault) | Encrypted P2P Vault using AES-GCM and CRDT sync. |
| [`netgene-cli`](file:///w:/NetGene%20OS/netgene-core/crates/netgene-cli) | Main unified CLI binary executable (`netgene`). |

---

## ⚡ Quickstart

### Prerequisites
- [Rust](https://www.rust-lang.org/) 1.75+
- Optional: [Ollama](https://ollama.com/) for local LLM intent parsing (`ollama run llama3`)

### Build
```bash
cd "netgene-core"
cargo build --release
```

---

## 🕹️ CLI Usage Examples

### 🔑 Identity & Gene Layer
```bash
netgene gene init --name "Master-01"
netgene gene show
```

### 🧠 Intent Engine (Local LLM - Ollama `llama3`)
```bash
netgene llm status
netgene llm parse "provision 3 quantum nodes with high availability"
netgene llm chat "Explain the NetGene OS architecture"
```

### 📂 Persistent Store (Sled DB) & 🌍 Real World Seeding
```bash
netgene seed data
netgene store save-node --id node-01 --name "Primary Gateway" --template gateway
netgene store nodes
netgene store dump
```

### 🌐 P2P Mesh Network (libp2p)
```bash
# Node 1 (Terminal 1)
netgene p2p listen --port 7777

# Node 2 (Terminal 2 - Connect to Node 1)
netgene p2p connect /ip4/127.0.0.1/tcp/7777 --port 7779

# Broadcast mesh message
netgene p2p broadcast "System-wide sync message"
```

### ☁️ New Extensions (Cloud, Lite, TPM, Vault)
```bash
netgene cloud mesh --region "us-east-1"
netgene lite provision --chip "esp32"
netgene tpm attest --nonce "random_nonce"
netgene vault store --file "secret.txt"
```

### ⚛️ Quantum Optimization
```bash
netgene quantum optimize --nodes 8
netgene quantum info
```

### 🧠 Neural BCI Stream
```bash
netgene neural status
netgene neural stream --target "core-gateway" --beta 0.85 --gamma 0.70
```

### 📟 Interactive Terminal Dashboard
```bash
netgene tui
```

---

## 🌐 WebGL 3D Megastructure Dashboard

Open [`dashboard/index.html`](file:///w:/NetGene%20OS/netgene-core/dashboard/index.html) in any browser to experience:
- **WebGL 3D Megastructure**: Three.js interactive cybernetic node graph.
- **Real-Time KPIs**: Network Health, Active Agents, Quantum Gain, Self-Heals.
- **Neural BCI Panel**: Live telemetry for Alpha, Beta, and Gamma EEG frequency bands.

---

## 📚 Documentation & Roadmap

- 🗺️ [**`docs/ROADMAP_PHASES.md`**](file:///w:/NetGene%20OS/netgene-core/docs/ROADMAP_PHASES.md) — Documento completo das Fases Implementadas (1-4) e Fases Futuras (5-8)
- 📐 [**`docs/ARCHITECTURE.md`**](file:///w:/NetGene%20OS/netgene-core/docs/ARCHITECTURE.md) — Arquitetura técnica detalhada do sistema
- 💻 [**`docs/CLI_REFERENCE.md`**](file:///w:/NetGene%20OS/netgene-core/docs/CLI_REFERENCE.md) — Manual de comandos da CLI `netgene.exe`
- 🔌 [**`docs/API_GUIDE.md`**](file:///w:/NetGene%20OS/netgene-core/docs/API_GUIDE.md) — Guia de integração Rust para desenvolvedores

---

## 📜 License
Dual-licensed under MIT or Apache-2.0.
