# netgene-core

Workspace Rust do NetGene OS. A visão geral, o estado de cada módulo e o roadmap estão no [README da raiz](../README.md).

| Crate | Função |
|---|---|
| `netgene-gene` | Identidade Ed25519, linhagem assinada, cofre de segredos |
| `netgene-kernel` | Agentes, barramento de mensagens, intents e marketplace local |
| `netgene-llm` | Cliente Ollama e parser de intenções |
| `netgene-p2p` | Nó libp2p (Noise, Yamux, Gossipsub, mDNS, Kademlia) |
| `netgene-store` | Persistência local (sled) e CRDT LWW |
| `netgene-vault` | Cofre local AES-256-GCM, chave no cofre do sistema |
| `netgene-wasm` | Sandbox de módulos WASM assinados |
| `netgene-safeguard` | Detecção de anomalias por z-score |
| `netgene-cli` | Binário `netgene` |

```bash
cargo test --workspace
```

```bash
cargo run -p netgene-cli -- --help
```

Arquitetura: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
