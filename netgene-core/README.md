# netgene-core

Workspace Rust do NetGene OS. A visão geral, o estado de cada módulo e o roadmap estão no [README da raiz](../README.md).

| Crate | Função |
|---|---|
| `netgene-gene` | Identidade híbrida Ed25519 + ML-DSA-65, linhagem assinada, cofre de segredos |
| `netgene-kernel` | Agentes, barramento de mensagens, intents e marketplace local |
| `netgene-llm` | Cliente Ollama e parser de intenções |
| `netgene-p2p` | Nó libp2p (Noise, Gossipsub, mDNS, Kademlia) e sessões X25519 + ML-KEM-768 |
| `netgene-store` | Persistência local (redb) e CRDT LWW |
| `netgene-vault` | Cofre local AES-256-GCM, chave no cofre do sistema |
| `netgene-wasm` | Sandbox de módulos WASM assinados, com permissões declaradas |
| `netgene-safeguard` | Detecção de anomalias por z-score |
| `netgene-policy` | Catálogo de ações, motor de políticas e log de auditoria assinado |
| `netgene-presence` | Confirmação com hardware (Windows Hello) |
| `netgene-cli` | Binário `netgene` |

```bash
cargo test --workspace
```

```bash
cargo run -p netgene-cli -- --help
```

Arquitetura: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
