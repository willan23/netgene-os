# Arquitetura do NetGene OS (v0.3.0)

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
│ Ollama HTTP  │   │ sled + LWW   │   │ wasmtime (valida │
│ + fallback   │   │ CRDT         │   │ e compila)       │
└──────────────┘   └───────┬──────┘   └──────────────────┘
                           │
┌──────────────────────────▼────────────────────────────────┐
│  netgene-gene: identidade Ed25519, hierarquia de genes     │
│  netgene-p2p: libp2p (Noise/Yamux/Gossipsub/mDNS/Kad)      │
│  netgene-vault: chunks AES-256-GCM                         │
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
| `~/.netgene/genes/` | `<id>.gene.json` (público) e `<id>.key` (chave privada, **sem cifra**) |
| `~/.netgene/db/` | Banco sled (nós, eventos, memória de agentes) |
| `~/.netgene/vault/` | Chunks cifrados e `.meta` (veja as limitações no README) |
| `./.netgene/marketplace/` | Manifestos de agentes locais (diretório de trabalho atual) |
