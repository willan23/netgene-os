# Arquitetura do NetGene OS (v0.4.0)

## Camadas

```text
┌───────────────────────────────────────────────────────────┐
│  Interfaces: netgene-cli · netgene-desktop (Tauri 2)       │
└──────────────────────────────┬────────────────────────────┘
                               │ comandos / IPC Tauri
┌──────────────────────────────▼────────────────────────────┐
│  netgene-kernel                                            │
│   • 5 agentes (Builder, Monitor, Optimizer, Network,       │
│     Evolution) em tasks Tokio                              │
│   • MessageBus (mpsc) · KernelMemory · Marketplace local   │
│   • dispatch_intent → netgene-llm (Ollama ou regras)       │
└───────┬──────────────────┬──────────────────┬─────────────┘
        │                  │                  │
┌───────▼──────┐   ┌───────▼──────┐   ┌───────▼──────────┐
│ netgene-llm  │   │ netgene-store│   │ netgene-wasm     │
│ Ollama HTTP  │   │ sled + LWW   │   │ wasmtime 36: só  │
│ + fallback   │   │ CRDT         │   │ módulos assinados│
└──────────────┘   └───────┬──────┘   └──────────────────┘
                           │
┌──────────────────────────▼────────────────────────────────┐
│  netgene-gene: Ed25519, linhagem assinada, SecretStore     │
│  netgene-p2p: libp2p, PeerId = Gene, allowlist de linhagem │
│  netgene-vault: chunks AES-256-GCM + AAD, chave no cofre   │
│  netgene-safeguard: anomalias por z-score                  │
└───────────────────────────────────────────────────────────┘
```

## Fluxo de uma intenção

1. O usuário escreve uma intenção na CLI (`netgene agent dispatch "..."`) ou no terminal do desktop.
2. O `NetSphereKernel::dispatch_intent` pede ao `LlmIntentEngine` uma ação estruturada. Se o Ollama não responder, ou responder algo fora do formato, entra o parser por regras.
3. A ação vira uma `AgentMessage` no `MessageBus`, e os agentes a consomem.
4. O desktop grava um `StoredEvent` no `netgene-store` e emite `kernel-log` para a interface.

**Limite atual:** ainda não existe um motor de políticas entre os passos 2 e 3. Ele é o item central da Fase 2 do [roadmap](../../ROADMAP.md).

## Dados locais

| Caminho | Conteúdo |
|---|---|
| `~/.netgene/genes/` | `<id>.gene.json` (identidade pública) |
| Cofre do SO, serviço `netgene-os` | `gene:<id>` (chave PKCS#8 da Gene) e `vault:<fp>` (chave AES do vault) |
| `~/.netgene/db/` | Banco sled (nós, eventos, memória de agentes) |
| `~/.netgene/vault/` | Chunks cifrados (formato v1) e `.meta` |
| `~/.netgene/marketplace/` | Manifestos de agentes assinados |

## Modelo de confiança

- **Gene Master:** a raiz. As sub-genes guardam `lineage_signature_b64`, a assinatura da Master sobre a chave pública delas.
- **Malha P2P:** cada nó roda como uma Gene, e o PeerId libp2p é derivado da mesma chave. A allowlist é a Master mais as genes cuja linhagem é válida. Identidades de outros dispositivos entram com `gene import` (só a parte pública).
- **WASM e marketplace:** o autor assina, e o sandbox e o marketplace verificam a assinatura sobre os bytes exatos.
