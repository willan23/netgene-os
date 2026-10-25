# NetGene OS 🧬

### A camada de confiança para agentes de IA: identidade criptográfica, permissões verificáveis e execução isolada, numa malha privada entre os seus dispositivos.

[![Website](https://img.shields.io/badge/Website-mnemosyne--one.com-cyan.svg)](https://mnemosyne-one.com)
[![License](https://img.shields.io/badge/License-MIT%20%2F%20Apache--2.0-green.svg)](#licença)
[![Status](https://img.shields.io/badge/status-alpha-yellow.svg)](#estado-real-de-cada-módulo)

> **Alpha (v0.5.0).** As falhas críticas da auditoria foram fechadas. Agora o LLM só **propõe**: um motor de políticas decide, e cada decisão fica num log de auditoria assinado. As identidades são **híbridas pós-quânticas**. O NetGene OS ainda não passou por auditoria externa. A tabela [Estado real](#estado-real-de-cada-módulo) diz exatamente o que funciona e quais limites continuam.

---

## Por que o NetGene existe

Agentes de IA estão ganhando acesso a arquivos, redes, chaves e APIs. Hoje quase nenhum deles tem uma identidade própria, um limite claro do que pode fazer ou um registro auditável do que fez. Esse é o problema que o NetGene quer resolver:

| Quem | O que ganha |
|---|---|
| **Usuário final** | Um assistente de IA local que roda nos seus dispositivos (PC, notebook e, no futuro, celular), sem nuvem de terceiros. Cada agente só faz o que você autorizou. |
| **Desenvolvedores** | Um runtime para publicar agentes e plugins WASM assinados, com permissões declaradas, que qualquer pessoa pode instalar com segurança. |
| **Empresas e big techs** | Uma camada auto-hospedada de identidade e política para frotas de agentes: cada ação é atribuída a uma identidade, limitada por capacidades e registrada, sem depender de um fornecedor. |

**O diferencial:** a proposta não é ser mais uma VPN mesh (NetBird, Tailscale e Nebula já fazem isso bem), nem mais um framework de agentes. O NetGene é a peça que falta entre os dois. É a camada que diz **quem** é cada agente, **o que** ele pode fazer e **onde** ele pode rodar.

---

## Arquitetura

```mermaid
graph TD
    UI["🖥️ Desktop (Tauri 2 + React)"] --> K
    CLI["💻 CLI (netgene)"] --> K
    K["⚡ Kernel: agentes + barramento de mensagens"] --> LLM["🧠 LLM local (Ollama): só propõe"]
    K --> POL["⚖️ Policy: decide + log de auditoria assinado"]
    K --> W["⚙️ Sandbox WASM"]
    K --> S["💾 Store (sled + CRDT)"]
    K --> G["🔑 Gene: identidade Ed25519"]
    P["🌐 P2P (libp2p)"] --> G
    V["📦 Vault (AES-256-GCM)"] --> G
    SG["🛡️ Safeguard: detecção de anomalias"] --> S
```

Mais detalhes em [netgene-core/docs/ARCHITECTURE.md](netgene-core/docs/ARCHITECTURE.md).

---

## Estado real de cada módulo

✅ funciona · 🧪 experimental (funciona em parte, com limitações conhecidas) · ⛔ não use para dados reais

| Crate | Estado | O que faz de verdade | Limitação conhecida |
|---|---|---|---|
| `netgene-gene` | ✅ | Identidades **híbridas Ed25519 + ML-DSA-65** (FIPS 204), fingerprint SHA3-256 e hierarquia Master → sub-genes com **linhagem assinada pelas duas chaves** (retirar a assinatura pós-quântica não serve como downgrade). As chaves privadas ficam no **cofre do sistema** (Windows Credential Manager, macOS Keychain, Linux Secret Service), nunca em disco. | A implementação ML-DSA (`ml-dsa` da RustCrypto) ainda não tem auditoria independente, e por isso é usada em modo híbrido. Genes antigas continuam só Ed25519. |
| `netgene-policy` | ✅ | **O LLM propõe, a política decide.** Catálogo de ações com capacidade e risco, motor de políticas (`allow` / `confirm` / `deny`, regras em `~/.netgene/policy.json`, limites de parâmetros) e **log de auditoria append-only**, encadeado por SHA3 e assinado, em `~/.netgene/audit.jsonl`. | O catálogo ainda é pequeno (4 ações). |
| `netgene-p2p` | ✅ | Nó libp2p 0.57: TCP + Noise + Yamux, Gossipsub assinado com validação estrita, mDNS e Kademlia. O **PeerId é derivado da Gene**, e só entram nós da mesma linhagem (allowlist). | O transporte (Noise) ainda é clássico, sem troca de chaves pós-quântica. Não há NAT traversal. |
| `netgene-store` | ✅ | Banco local (sled) para nós, eventos e memória de agentes, com registrador CRDT LWW. | `sled` está sem manutenção ativa. Migração prevista para `redb`. |
| `netgene-llm` | ✅ | Cliente Ollama (chat/generate) e parser de intenções com fallback por regras. | Modelo e URL fixos (`llama3`, `localhost:11434`). |
| `netgene-kernel` | 🧪 | 5 agentes em tasks Tokio e barramento de mensagens. Cada intenção passa pelo motor de políticas: leituras rodam na hora, mudanças esperam confirmação humana e o que está fora da política é negado. Inclui o marketplace local com **manifestos assinados**. | Os agentes ainda são simples (o "provisionamento" é só uma mensagem para os agentes, sem infraestrutura real por trás). |
| `netgene-safeguard` | 🧪 | Detecção de anomalias por z-score em janela deslizante. | As ações de "self-healing" são apenas registradas, não executadas. |
| `netgene-wasm` | ✅ | **Executa** módulos assinados (Ed25519) por um autor confiável, com wasmtime 36 LTS, limite de CPU (fuel) e de memória, e sem imports de host (sem WASI, rede ou arquivos). | A ABI v1 é mínima (`run() -> i32`) e ainda não recebe entrada. |
| `netgene-vault` | 🧪 | Cifra arquivos em pedaços com AES-256-GCM. A chave fica no cofre do sistema e **sobrevive a reinícios**. Cada pedaço é ligado ao arquivo, à posição e ao total (AAD). | Só local: ainda não há replicação entre dispositivos. Arquivos de versões ≤ 0.3 não podem ser lidos (a chave antiga se perdia). |
| `netgene-desktop` | 🧪 | Painel de nós, eventos, agentes, marketplace (assina com uma Gene local), vault e terminal de intenções com botões **Confirmar/Rejeitar**. Age como a Master Gene local. | Sem autenticação local (passkeys reais estão previstos). |
| `netgene-cli` | ✅ | Comandos `gene` (init, spawn, verify, export, import), `agent` (dispatch `--gene` [`--confirm`], audit, policy, publish, market), `store`, `llm`, `p2p`, `wasm` (sign, run) e `vault`. | — |

**Removido na versão 0.3.0:** os módulos quântico/QAOA, QPU, BCI neural, DAO, TPM, eBPF, Kubernetes, IoT lite, cloud mesh, builder, TUI e o "FHE". Eles eram simulados (valores fixos ou verificações que sempre aceitavam) e davam garantias que não existiam. O histórico do git preserva o código. Alguns podem voltar quando houver uma implementação real (por exemplo, eBPF com `aya` ou TPM com `tss-esapi`). O motivo de cada remoção está em [AUDITORIA_2026-10.md](AUDITORIA_2026-10.md).

---

## Como executar

**Pré-requisitos:** [Rust](https://rustup.rs/) estável (testado com 1.94), [Node.js](https://nodejs.org/) 20+ para o desktop e, opcionalmente, [Ollama](https://ollama.com/) com o modelo `llama3`.

```bash
cd netgene-core
cargo test --workspace
```

```bash
cargo run -p netgene-cli -- gene init --name minha-identidade
```

```bash
cargo run -p netgene-cli -- p2p listen --gene <ID-da-gene> --port 7777
```

O ID aparece em `gene init`. Outros nós entram se forem sub-genes da mesma Master (`gene spawn`, depois `gene export` e `gene import` no outro dispositivo).

```bash
cd netgene-desktop && npm install && npm run tauri dev
```

**Docker** (nó P2P headless):

```bash
docker build -t netgene netgene-core
```

---

## Roadmap

O plano completo está em [ROADMAP.md](ROADMAP.md). Resumo:

1. **Fase 0 (concluída):** higiene do repositório, CI funcional, licença e documentação honesta.
2. **Fase 1 (concluída, v0.4.0):** chaves no cofre do sistema, vault persistente e autenticado, WASM assinado e executado de fato, marketplace assinado, P2P com identidade Gene e allowlist.
3. **Fase 2 (em andamento, v0.5.0):** motor de políticas, log de auditoria assinado, identidade híbrida pós-quântica e libp2p 0.57 já estão feitos. Faltam capability tokens, runtime WASM com permissões, passkeys e `redb`.
4. **Fase 3:** produto. Marketplace assinado, app mobile, eBPF no Linux, SSO e integrações empresariais.

---

## Segurança

O NetGene OS ainda não passou por auditoria de segurança externa. Use com cautela antes disso. Para reportar uma vulnerabilidade, abra uma *security advisory* privada no GitHub em vez de uma issue pública.

## Licença

Distribuído sob [MIT](LICENSE-MIT) **ou** [Apache-2.0](LICENSE-APACHE), à sua escolha.
