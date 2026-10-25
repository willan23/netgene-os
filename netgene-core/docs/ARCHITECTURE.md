# Arquitetura do NetGene OS (v0.7.0)

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
2. O `NetSphereKernel::propose_intent` pede ao `LlmIntentEngine` uma ação estruturada. Se o Ollama não responder, ou responder algo fora do formato, entra o parser por regras. O resultado é só uma **proposta**.
3. A proposta é mapeada para uma `Action` do catálogo (`netgene-policy`). Se não estiver no catálogo, ela é negada.
4. O `PolicyEngine` avalia a ação para a Gene operadora (status, capacidade, limites de parâmetros, regras):
   - `allow`: executa.
   - `confirm`: fica pendente até `confirm(proposal_id)`, ou é descartada com `reject`.
   - `deny`: é recusada.
5. Cada passo (proposta, decisão, confirmação, execução) entra no `AuditLog`, encadeado por SHA3 e assinado pela Gene operadora.
6. Só então a ação vira uma `AgentMessage` no `MessageBus`.

## Dados locais

| Caminho | Conteúdo |
|---|---|
| `~/.netgene/genes/` | `<id>.gene.json` (identidade pública) |
| Cofre do SO, serviço `netgene-os` | `gene:<id>` (PKCS#8 Ed25519), `gene-pq:<id>` (semente ML-DSA-65) e `vault:<fp>` (chave AES do vault) |
| `~/.netgene/policy.json` | Política (opcional; sem ela vale a padrão: leituras permitidas, escritas com confirmação) |
| `~/.netgene/audit.jsonl` | Log de auditoria encadeado e com assinatura híbrida |
| `~/.netgene/tokens/` | Capability tokens (`<token_id>.json`) |
| `~/.netgene/revoked-tokens.json` | Ids de tokens revogados |
| `~/.netgene/presence/` | Chave pública do Windows Hello por Gene (a privada fica no TPM) |
| `~/.netgene/store.redb` | Banco redb (nós, eventos, memória de agentes); `db.sled-migrated/` é o backup da base antiga |
| `~/.netgene/vault/` | Chunks cifrados (formato v1) e `.meta` |
| `~/.netgene/marketplace/` | Manifestos de agentes assinados |

## Modelo de confiança

- **Gene Master:** a raiz. As sub-genes guardam `lineage_signature_b64`, a assinatura da Master sobre a chave pública delas.
- **Malha P2P:** cada nó roda como uma Gene, e o PeerId libp2p é derivado da mesma chave. A allowlist é a Master mais as genes cuja linhagem é válida. Identidades de outros dispositivos entram com `gene import` (só a parte pública).
- **WASM e marketplace:** o autor assina, e o sandbox e o marketplace verificam a assinatura sobre os bytes exatos.
