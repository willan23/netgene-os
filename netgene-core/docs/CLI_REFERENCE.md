# 📖 NetGene CLI — Complete Command Reference (v1.0.0 Apex Megastructure)

The `netgene` binary (`netgene.exe`) exposes every layer of NetGene OS through a unified CLI built with **Clap v4**. All 15 subcommand groups are documented below exactly as implemented.

---

## Global Options

```bash
netgene [OPTIONS] <SUBCOMMAND>
```

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable `DEBUG`-level tracing (default: `INFO`) |
| `--json` | — | Emit structured JSON output instead of human-readable text |
| `--help` | `-h` | Print help for any command or subcommand |
| `--version` | `-V` | Print version (`v1.0.0`) |

> All flags are **global** and can be placed before or after the subcommand.

---

## 1. 🔑 `gene` — Identity & Cryptographic Key Management

Manages the **Gene Layer** — the root identity system of NetGene OS.  
Keys are stored at `~/.netgene/genes/` as JSON files.

```bash
# Generate a new Master Gene (root cryptographic identity)
netgene gene init --name "Master-01"
netgene gene init -n "MyNode" --json

# List all stored genes (master + sub-genes)
netgene gene show
netgene gene show --json

# Spawn a Sub-Gene derived from a Master Gene
netgene gene spawn --parent <PARENT_GENE_ID> --name "Edge-01" --role node
netgene gene spawn --parent <ID> --name "AgentX" --role agent
netgene gene spawn --parent <ID> --name "Watcher" --role observer

# Verify and display a gene by ID
netgene gene verify <GENE_ID>
```

### Gene Roles
| Role | Capabilities | Description |
|------|-------------|-------------|
| `master` | All capabilities | Root identity, signs all sub-genes |
| `node` | `node.spawn`, `network.read` | Network node identity |
| `agent` | `agent.run`, `network.read` | AI agent identity |
| `observer` | `network.read` | Read-only monitor identity |

---

## 2. 🖥️ `node` — Network Topology & Node Management

Manages NetGene network nodes stored in the persistent Sled DB.

```bash
# List all registered network nodes
netgene node list
netgene node list --json

# Add a node to the store
netgene node add --name "gateway-01" --template "gateway" --ip "10.42.0.1" --port 7000

# Remove a node by ID
netgene node remove --id <NODE_ID>

# Show node details
netgene node show --id <NODE_ID>
```

---

## 3. 🤖 `agent` — Netsphere Kernel BDI Agents

Interacts with the **Netsphere Kernel** and its 5 autonomous BDI agents.

```bash
# List all registered agents and their status
netgene agent list

# Show kernel status summary
netgene agent status

# Dispatch a natural language intent to the kernel
netgene agent dispatch "spawn 3 quantum nodes"
netgene agent dispatch "optimize routes for the mesh"
netgene agent dispatch "heal the network"
netgene agent dispatch "status"
```

### Registered Agents
| Agent | Role | Action Keywords |
|-------|------|-----------------|
| `BuilderAgent` | Node provisioning | `spawn`, `create`, `provision` |
| `MonitorAgent` | Telemetry & alerts | `status`, `monitor`, `list` |
| `OptimizerAgent` | Quantum routing | `optimize`, `route`, `quantum` |
| `NetworkAgent` | P2P mesh topology | `network`, `topology`, `connect` |
| `EvolutionAgent` | Fitness & mutation | `evolve`, `mutate`, `heal` |

---

## 4. ⚛️ `quantum` — QAOA & SQA Route Optimizer

Quantum-inspired optimization engine (QAOA + Simulated Quantum Annealing).

```bash
# Run full quantum route optimization on a demo network graph
netgene quantum optimize --nodes 8 --layers 3
netgene quantum optimize -n 16 -l 5

# Run QAOA on a random QUBO problem (n variables)
netgene quantum qaoa --size 6
netgene quantum qaoa -s 12

# Run Simulated Quantum Annealing (SQA) on a QUBO problem
netgene quantum anneal --size 8
netgene quantum anneal -s 20

# Show quantum module info & algorithm details
netgene quantum info
```

### Algorithm Parameters
| Param | Default | Description |
|-------|---------|-------------|
| `--nodes` / `-n` | `8` | Number of nodes in the network graph |
| `--layers` / `-l` | `3` | QAOA circuit depth (p-value) |
| `--size` / `-s` | `6–8` | Number of binary variables in the QUBO problem |

---

## 5. 🏗️ `build` — Organic Node Provisioner

Intent-driven node provisioning engine that parses natural language to create nodes.

```bash
# Provision nodes from a natural language intent string
netgene build spawn "spawn 3 edge nodes with quantum routing"
netgene build spawn "create 2 gateway nodes for the mesh"
netgene build spawn "provision 5 quantum nodes with HA"

# List all provisioned nodes in the current session
netgene build list
```

### Node Templates
| Template | Description |
|----------|-------------|
| `edge` | Edge/leaf node (default) |
| `core` | Core infrastructure node |
| `gateway` | Network gateway node |
| `quantum` | Quantum-enhanced compute node |
| `custom:<name>` | User-defined template |

---

## 6. 📂 `store` — Sled DB & CRDT State Store

Persistent key-value store backed by Sled DB at `~/.netgene/db`.

```bash
# Show database status and stats
netgene store status

# List all stored nodes
netgene store list
netgene store list --json

# Get a node record by ID
netgene store get --id "node-01"

# Get agent memory value
netgene store memory --agent "BuilderAgent" --key "last_provisioned"

# List recent events
netgene store events --limit 20
```

---

## 7. 🧠 `llm` — Ollama Local LLM & Intent Engine 2.0

Integrates with a local **Ollama** instance (`llama3` / `qwen`) for NL intent parsing.  
Falls back to a rule-based engine when Ollama is offline.

```bash
# Check Ollama connectivity
netgene llm status

# Parse a natural language command into a structured intent (JSON)
netgene llm parse "cria 3 nós quânticos com alta disponibilidade"
netgene llm parse "optimize routing for 10 nodes"
netgene llm parse "spawn a monitoring agent"

# Chat with the local LLM
netgene llm chat --prompt "Qual o estado da rede Mesh?"
netgene llm chat --prompt "Explain the current network topology"
```

### Supported Intent Actions
| Action | Example Trigger |
|--------|----------------|
| `provision_nodes` | "spawn", "create", "provision" |
| `optimize_network` | "optimize", "route", "quantum" |
| `trigger_anomaly_scan` | "heal", "scan", "safeguard" |
| `system_status` | anything else |

---

## 8. 🌐 `p2p` — libp2p Mesh Network

Manages the distributed **libp2p** P2P mesh (Gossipsub + mDNS + Kademlia DHT).

```bash
# Show P2P node status and PeerId
netgene p2p status

# Start a P2P listener node
netgene p2p start --port 7700

# List known mesh peers
netgene p2p peers

# Publish a message on the mesh topic
netgene p2p publish --topic "netgene-mesh-v1" --message "HEALTH_OK"

# Dial a specific remote peer
netgene p2p dial --addr "/ip4/192.168.1.10/tcp/7700"
```

### Mesh Protocol Stack
```
TCP  →  Noise (XX handshake)  →  Yamux  →  Gossipsub / Kademlia / mDNS / Identify
```

---

## 9. 🧠 `neural` — BCI Neural Intent Adapter

Processes EEG signal streams from Brain-Computer Interfaces and translates them into kernel actions.

```bash
# Show neural adapter status
netgene neural status

# Process a single EEG signal reading
netgene neural stream --target "node-01" --beta 0.85 --gamma 0.75
netgene neural stream -t "node-03" -b 0.95 -g 0.90

# Start continuous BCI stream monitor
netgene neural monitor --duration 60
```

### Signal → Action Mapping
| Beta Power | Gamma Power | Action |
|-----------|-------------|--------|
| > 0.90 | > 0.85 | `EMERGENCY_HEAL` (Critical focus) |
| > 0.75 | > 0.70 | `OPTIMIZE_ROUTE` (High focus) |
| ≤ 0.75 | ≤ 0.70 | `MONITOR` (Relaxed state) |

---

## 10. ⚙️ `wasm` — WebAssembly Sandbox

Executes cryptographically signed WASM modules in an isolated sandbox.

```bash
# Show WASM sandbox status
netgene wasm status

# Execute a named WASM module (must be signed)
netgene wasm run --name "routing-optimizer-v2"
netgene wasm run -n "anomaly-detector-v1"

# Validate a WASM binary (checks 0x00asm magic bytes + 10 MB limit)
netgene wasm validate --path "./modules/optimizer.wasm"
```

### Security Constraints
- **Magic Header**: Every module must start with `\0asm` (`0x00 0x61 0x73 0x6d`)
- **Size Limit**: Maximum **10 MB** per module
- **Heap Limit**: **1 MB** sandbox memory
- **Signature**: Must pass Ed25519 signature verification

---

## 11. 🛡️ `ebpf` — Kernel Network Security Telemetry

eBPF probes for real-time kernel-level network telemetry and security monitoring.

```bash
# Show eBPF probe status
netgene ebpf status

# Sample network telemetry from a network interface
netgene ebpf sample --interface eth0
netgene ebpf sample -i lo

# Attach an eBPF probe
netgene ebpf attach --program "tcp_rtt_tracker"

# Detach a running eBPF probe
netgene ebpf detach --program "tcp_rtt_tracker"
```

### Telemetry Metrics
| Metric | Unit | Description |
|--------|------|-------------|
| RTT | microseconds | Per-packet round-trip time |
| Packet Entropy | bits | Shannon entropy of packet payload |
| TCP Stats | count | SYN/SYN-ACK/ACK packet counters |

---

## 12. ☸️ `k8s` — Kubernetes Operator & CRD Generator

Generates and manages NetGene Custom Resource Definitions (CRDs) for Kubernetes.

```bash
# Show Kubernetes operator status
netgene k8s status

# Generate a CRD manifest (YAML)
netgene k8s manifest --name "gene-node-alpha" --namespace "netgene-system"
netgene k8s manifest -n "edge-cluster-01" --namespace "production"

# Apply a CRD manifest to the cluster (via kubectl)
netgene k8s apply --name "gene-node-alpha" --namespace "netgene-system"

# Reconcile a CRD resource state
netgene k8s reconcile --name "gene-node-alpha"
```

### Supported CRD Types
| CRD | API Group | Description |
|-----|-----------|-------------|
| `GeneNode` | `netgene.io/v1alpha1` | Single network node resource |
| `GeneMeshCluster` | `netgene.io/v1alpha1` | Full mesh cluster configuration |
| `QuantumRoutePolicy` | `netgene.io/v1alpha1` | Quantum routing policy |

---

## 13. ⚛️ `qpu` — Physical QPU & OpenQASM 3.0 Transpiler

Connects to real quantum hardware via REST APIs and generates OpenQASM 3.0 circuits.

```bash
# Show QPU connector status
netgene qpu status

# Generate an OpenQASM 3.0 circuit (local transpile)
netgene qpu qasm --qubits 4 --layers 2
netgene qpu qasm -q 8 -l 3

# Submit a QUBO problem to a real QPU backend
netgene qpu submit --backend "ibm_brisbane" --shots 1000
netgene qpu submit --backend "aws_braket" --shots 2000
netgene qpu submit --backend "rigetti" --shots 500
```

### Supported QPU Backends
| Backend | Provider | Notes |
|---------|---------|-------|
| `ibm_brisbane` | IBM Quantum | REST via IBM Cloud API |
| `aws_braket` | Amazon Web Services | Managed quantum via Braket |
| `rigetti` | Rigetti Computing | QCS API |
| `ionq` | IonQ | Cloud ion-trap QPU |

---

## 14. 📱 `mobile` — Mobile PWA & WebAuthn Passkey Engine

Biometric authentication engine using **WebAuthn Passkeys** (Face ID / Touch ID).

```bash
# Show mobile bridge status
netgene mobile status

# Generate a WebAuthn authentication challenge for a user gene
netgene mobile challenge --user "gene-master-01"
netgene mobile challenge -u "gene-master-01" --json

# Start the WebSocket bridge for mobile PWA telemetry
netgene mobile bridge --port 8080
netgene mobile bridge -p 9090
```

### Authentication Flow
```
Mobile App → Challenge Request → Ed25519 Signed Response → Gene Verification → Access Granted
```

---

## 15. 🏛️ `dao` — P2P Autonomous Governance & DAO Tokenomics

Decentralized governance engine with **Proof-of-Utility** token rewards.

```bash
# Show DAO status
netgene dao status

# Submit a governance proposal (requires Master Gene)
netgene dao propose --title "Upgrade QAOA Layers to 5" \
                    --description "Improve quantum routing accuracy by 20%" \
                    --proposer "gene-master-01"

# Vote on an active proposal
netgene dao vote --proposal-id <UUID> --voter "gene-node-02" --approve --weight 100
netgene dao vote --proposal-id <UUID> --voter "gene-node-03" --reject --weight 50

# Calculate and display Proof-of-Utility token reward
netgene dao reward --node "node-alpha-01" --qpu-shots 1000 --packets 50000
```

### Proposal Types
| Type | Description | Required Quorum |
|------|-------------|-----------------|
| `KernelMutation` | Modify kernel agent behaviour | 66% weighted vote |
| `QuantumWeightUpdate` | Change QAOA layer depth | 66% weighted vote |
| `NodePolicyUpdate` | Change max node count | 66% weighted vote |

### Proof-of-Utility Formula
```
GENE Tokens = (qpu_shots × 0.05) + (packets_inspected × 0.001)
```

---

## 16. 📟 `tui` — Interactive Terminal Dashboard

Launches the full-screen **Ratatui** terminal dashboard with live system metrics.

```bash
netgene tui
```

### TUI Keyboard Controls
| Key | Action |
|-----|--------|
| `Tab` / `→` | Next tab |
| `Shift+Tab` / `←` | Previous tab |
| `↑` / `↓` | Scroll up/down |
| `r` | Force refresh |
| `q` / `Esc` | Quit |

### Dashboard Tabs
| Tab | Contents |
|-----|----------|
| **Dashboard** | System health, uptime, gene fingerprint, quick stats |
| **Agents** | 5 BDI agent status, message bus activity |
| **Network** | Node topology table, load, latency, connections |
| **Quantum** | QAOA improvement %, annealing results, QPU status |
| **Logs** | Live scrolling event log |

---

## Example Workflows

### Bootstrap a New NetGene Node
```bash
# 1. Create root identity
netgene gene init --name "Apex-Node-01"

# 2. Spawn a network sub-gene
netgene gene spawn --parent <MASTER_ID> --name "Net-01" --role node

# 3. Launch the kernel and provision nodes
netgene agent dispatch "spawn 5 edge nodes"

# 4. Run quantum route optimization
netgene quantum optimize --nodes 16 --layers 4

# 5. Monitor in TUI
netgene tui
```

### Quantum Benchmarking Pipeline
```bash
netgene quantum qaoa --size 12
netgene quantum anneal --size 12
netgene quantum optimize --nodes 20 --layers 5
netgene qpu qasm --qubits 8 --layers 3
```

### Natural Language Workflow (with Ollama)
```bash
netgene llm parse "provision 3 quantum gateway nodes with redundancy"
netgene agent dispatch "optimize mesh routing for 10 nodes"
netgene agent dispatch "heal any degraded nodes"
```
