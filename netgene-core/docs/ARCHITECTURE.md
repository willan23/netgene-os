# Arquitetura do NetGene OS (v0.9.0)

## Camadas

```text
┌───────────────────────────────────────────────────────────┐
│  Interfaces: netgene-cli · netgene-desktop (Tauri 2)       │
└──────────────────────────────┬────────────────────────────┘
                               │ comandos / IPC Tauri
┌──────────────────────────────▼────────────────────────────┐
│  netgene-kernel                                            │
│   • propose_intent → netgene-llm (Ollama ou regras)        │
│   • PolicyEngine + AuditLog (netgene-policy)               │
│   • confirmação: humano, Windows Hello ou passkey          │
│   • Executor: genes, métricas do host, agentes WASM        │
│   • Marketplace local + reputação                          │
└───────┬──────────────────┬──────────────────┬─────────────┘
        │                  │                  │
┌───────▼──────┐   ┌───────▼──────┐   ┌───────▼──────────┐
│ netgene-llm  │   │ netgene-store│   │ netgene-wasm     │
│ Ollama HTTP  │   │ redb + LWW   │   │ wasmtime 36:     │
│ + fallback   │   │ CRDT, KV dos │   │ módulos e compo- │
│              │   │ agentes      │   │ nentes assinados │
└──────────────┘   └──────────────┘   └──────────────────┘
┌───────────────────────────────────────────────────────────┐
│  netgene-gene: Ed25519 + ML-DSA-65, linhagem, tokens,      │
│                SecretStore (cofre do SO / arquivo cifrado) │
│  netgene-p2p: libp2p, PeerId = Gene, allowlist, sessões    │
│               X25519 + ML-KEM-768, gossip com sender keys, │
│               QUIC, relays, DCUtR, AutoNAT                 │
│  netgene-presence: Windows Hello, passkeys WebAuthn        │
│  netgene-enterprise: OIDC (JWKS em cache), SAML (opcional) │
│  netgene-vault: chunks AES-256-GCM + AAD, chave no cofre   │
│  netgene-safeguard: anomalias por z-score                  │
│  netgene-ebpf: contador XDP (Linux)                        │
└───────────────────────────────────────────────────────────┘
```

## Fluxo de uma intenção

1. O usuário escreve uma intenção na CLI (`netgene agent dispatch "..."`) ou no terminal do desktop. A CLI também aceita ações estruturadas (`agent run`), que seguem o mesmo caminho.
2. O `NetSphereKernel::propose_intent` pede ao `LlmIntentEngine` uma ação estruturada. Se o Ollama não responder, ou responder algo fora do formato, entra o parser por regras. O resultado é só uma **proposta**.
3. A proposta é mapeada para uma `Action` do catálogo (`netgene-policy`). Se não estiver no catálogo, ela é negada.
4. O `PolicyEngine` avalia a ação para a Gene operadora (status, capacidade, limites de parâmetros, regras):
   - `allow`: executa.
   - `confirm`: fica pendente até `confirm`, `confirm_with_presence` (Windows Hello) ou `confirm_with_passkey`, ou é descartada com `reject`. As provas de presença assinam um desafio que liga o id da proposta à ação exata.
   - `deny`: é recusada.
5. O `Executor` faz o trabalho e devolve o resultado em `DispatchOutcome::Executed { result }`.
6. Cada passo (proposta, decisão, confirmação, execução ou falha, com um resumo do resultado) entra no `AuditLog`, encadeado por SHA3 e assinado pela Gene operadora.

| Ação | Risco | Executor |
|---|---|---|
| `system.status` | leitura | Estado do operador, das genes, dos agentes, do store e do kernel |
| `safeguard.scan` | leitura | CPU, memória e processos do host (sysinfo) no detector de anomalias |
| `node.provision` | escrita | Sub-genes do operador (linhagem híbrida, chave no SecretStore, nó no store) |
| `agent.list` | leitura | Agentes WASM em `~/.netgene/agents` e se o autor é confiável |
| `agent.run` | escrita | Sandbox com as permissões que o agente declarou e a política concede, com o KV persistido |

## Malha P2P

- **Transporte:** TCP e QUIC, Noise + Yamux, DNS. O cliente de relay é sempre ativo, e o servidor de relay é opcional (`--relay-server`).
- **Admissão:** o PeerId é derivado da chave Ed25519 da Gene. A allowlist é a Master mais as genes com linhagem válida, e quem está fora dela é desconectado (também nos relays).
- **Sessões privadas (`/netgene/pq/2`):** handshake X25519 + ML-KEM-768 assinado pelas duas Genes. Os frames usam ChaCha20-Poly1305 com contador, e o conteúdo é um `PrivatePayload` (mensagem ou sender keys).
- **Gossip (`netgene-mesh-v2`):** assinado pelo autor (Gossipsub estrito) e cifrado com a sender key dele. A chave chega só pelas sessões pós-quânticas, como um anúncio com assinatura híbrida, e os membros a repassam a quem não está conectado ao autor. Ela troca a cada hora e expira em 3 h. Mensagens que chegam antes da chave ficam num buffer.
- **NAT:** um nó atrás de NAT reserva uma vaga num relay da malha. Os outros o alcançam por `…/p2p/<relay>/p2p-circuit/p2p/<nó>`, e o DCUtR tenta a conexão direta.

## Agentes WASM

Um agente é um `GeneModule`: bytes WASM (módulo ou componente), nome, versão e permissões, com assinatura híbrida do autor. O sandbox detecta o tipo pelos bytes.

- **Módulo (ABI v2):** imports `netgene.log`, `netgene.input_read`… por permissão.
- **Componente:** o mundo [`netgene:agent@0.1.0`](../crates/netgene-wasm/wit/agent.wit), com uma interface WIT por permissão (`log`, `io`, `kv`, `clock`) e o export `run: func() -> s32`.

Só são linkadas as interfaces das permissões concedidas. Fuel, memória e cotas valem para os dois tipos.

## Dados locais

| Caminho | Conteúdo |
|---|---|
| `~/.netgene/genes/` | `<id>.gene.json` (identidade pública) |
| Cofre do SO, serviço `netgene-os` (ou `secrets.enc`, 0600, com `NETGENE_SECRET_STORE=file`) | `gene:<id>` (PKCS#8 Ed25519), `gene-pq:<id>` (semente ML-DSA-65) e `vault:<fp>` (chave AES do vault) |
| `~/.netgene/policy.json` | Política (opcional; sem ela vale a padrão: leituras permitidas, escritas com confirmação, agentes com `log`/`io`/`kv`/`clock`) |
| `~/.netgene/audit.jsonl` | Log de auditoria encadeado e com assinatura híbrida |
| `~/.netgene/tokens/` | Capability tokens (`<token_id>.json`) |
| `~/.netgene/revoked-tokens.json` | Ids de tokens revogados |
| `~/.netgene/presence/` | Chave pública do Windows Hello (`<gene>.json`) e passkeys (`<gene>.passkeys.json`) por Gene |
| `~/.netgene/agents/` | Agentes WASM instalados (`<nome>.gene-module.json`) |
| `~/.netgene/store.redb` | Banco redb (nós, eventos, memória e KV dos agentes) |
| `~/.netgene/enterprise.json`, `jwks-cache.json` | Configuração OIDC e cache do JWKS |
| `~/.netgene/saml.json`, `saml-replay.json` | Configuração SAML e ids de asserções já usadas |
| `~/.netgene/vault/` | Chunks cifrados (formato v1) e `.meta` |
| `~/.netgene/marketplace/` | Manifestos de agentes assinados |

## Modelo de confiança

- **Gene Master:** a raiz. As sub-genes guardam `lineage_signature_b64`, a assinatura da Master sobre a chave pública delas.
- **Malha P2P:** cada nó roda como uma Gene, e o PeerId libp2p é derivado da mesma chave. A allowlist é a Master mais as genes cuja linhagem é válida. Identidades de outros dispositivos entram com `gene import` (só a parte pública). Um relay ou um membro que repassa a sender key de outro pode repassá-la, mas não forjá-la.
- **WASM e marketplace:** o autor assina, e o sandbox e o marketplace verificam a assinatura sobre os bytes exatos. O kernel só instala e executa agentes de autores da linhagem do operador.
- **Identidade corporativa:** os ID tokens OIDC e as respostas SAML viram capability tokens com prazo, emitidos pela Gene pai e registrados no log de auditoria.

## Verificação

- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` e `cargo deny check`.
- `cargo test -p netgene-enterprise --features saml` (Linux, com xmlsec).
- `bash deploy/mesh-test/run.sh`: 3 nós em Docker, em redes isoladas ligadas por um relay.
- `netgene-core/fuzz`: alvos de libFuzzer para WebAuthn, mensagens P2P, tokens OIDC e registros assinados.
