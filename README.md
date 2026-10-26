# NetGene OS 🧬

### A camada de confiança para agentes de IA: identidade criptográfica, permissões verificáveis e execução isolada, numa malha privada entre os seus dispositivos.

[![Website](https://img.shields.io/badge/Website-mnemosyne--one.com-cyan.svg)](https://mnemosyne-one.com)
[![License](https://img.shields.io/badge/License-MIT%20%2F%20Apache--2.0-green.svg)](#licença)
[![Status](https://img.shields.io/badge/status-alpha-yellow.svg)](#estado-real-de-cada-módulo)

> **Alpha (v0.9.0).** O LLM só **propõe**: um motor de políticas decide, um executor faz o trabalho de verdade e cada passo fica num log de auditoria assinado. Identidades, manifestos, módulos WASM, tokens e o log são assinados de forma **híbrida pós-quântica**. Na malha, tanto as mensagens privadas quanto o **gossip** são confidenciais contra computadores quânticos (**X25519 + ML-KEM-768**), e nós atrás de NAT se alcançam por **relays da própria malha** com hole punching. As capacidades podem ser delegadas com **tokens que expiram e podem ser revogados**, inclusive a partir de **OIDC ou SAML**. As confirmações podem exigir **Windows Hello** ou uma **passkey** (até do celular). Os agentes são **WebAssembly assinado** (módulo ou **Component Model**) e só recebem as **permissões que declaram e que a política concede**. O NetGene OS ainda não passou por auditoria externa. A tabela [Estado real](#estado-real-de-cada-módulo) diz exatamente o que funciona e quais limites continuam.

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
    K["⚡ Kernel: intenção → política → executor"] --> LLM["🧠 LLM local (Ollama): só propõe"]
    K --> POL["⚖️ Policy: decide + log de auditoria assinado"]
    K --> PR["🖐️ Presence: Windows Hello / passkeys"]
    K --> W["⚙️ Agentes WASM (módulo ou componente)"]
    K --> S["💾 Store (redb)"]
    K --> SG["🛡️ Safeguard: métricas + anomalias"]
    K --> G["🔑 Gene: identidade Ed25519 + ML-DSA-65"]
    E["🏢 Enterprise: OIDC / SAML"] --> G
    P["🌐 P2P (libp2p, ML-KEM, relays)"] --> G
    V["📦 Vault (AES-256-GCM)"] --> G
```

Mais detalhes em [netgene-core/docs/ARCHITECTURE.md](netgene-core/docs/ARCHITECTURE.md).

---

## Estado real de cada módulo

✅ funciona · 🧪 experimental (funciona em parte, com limitações conhecidas) · ⛔ não use para dados reais

| Crate | Estado | O que faz de verdade | Limitação conhecida |
|---|---|---|---|
| `netgene-gene` | ✅ | Identidades **híbridas Ed25519 + ML-DSA-65** (FIPS 204) com **linhagem assinada pelas duas chaves**. Há uma primitiva de assinatura híbrida que liga a chave pública aos bytes assinados (remover a parte pós-quântica invalida também o Ed25519). **Capability tokens**: delegação de capacidades com janela de validade, assinada, revogável e sem escalada de privilégio. As chaves privadas ficam no **cofre do sistema**, nunca em disco. | A implementação ML-DSA (`ml-dsa` da RustCrypto) ainda não tem auditoria independente, e por isso é usada sempre em modo híbrido. Genes antigas continuam só Ed25519. |
| `netgene-policy` | ✅ | **O LLM propõe, a política decide.** Catálogo de ações com capacidade e risco (`system.status`, `safeguard.scan`, `node.provision`, `agent.list`, `agent.run`), motor de políticas (`allow` / `confirm` / `deny`, regras em `~/.netgene/policy.json`, limites de parâmetros, permissões que os agentes podem receber, capacidades vindas de tokens válidos) e **log de auditoria append-only**, encadeado por SHA3 e com assinatura híbrida, em `~/.netgene/audit.jsonl`. Ele pode ser **exportado para SIEM** em **OCSF** ou **CEF** (`agent audit --export`). | Ações novas exigem código no kernel (o catálogo é fechado de propósito). |
| `netgene-enterprise` | ✅ | **OIDC:** um ID token corporativo (Okta, Entra ID, Keycloak…) é verificado contra o JWKS do IdP (só RS256/ES256; `iss`, `aud` e `exp` conferidos), e os grupos viram capacidades num capability token com prazo (`gene grant-oidc`, mapeamento em `~/.netgene/enterprise.json`). O **JWKS fica em cache** (1 h; nova busca quando aparece um `kid` desconhecido, no máximo uma por minuto; cache de até 24 h se o IdP cair). **SAML 2.0** (`gene grant-saml`, recurso `saml`): resposta assinada verificada com `samael` + xmlsec (proteção contra *signature wrapping*), metadados sem certificado são recusados, só SHA-256 ou mais forte, e cada asserção só pode ser usada uma vez. As concessões ficam no log de auditoria. | O SAML precisa de libxml2, xmlsec1 e OpenSSL, por isso é opcional (servidores Linux). Respostas SAML cifradas ainda não são aceitas. |
| `netgene-ebpf` | ✅ (Linux) | **eBPF real:** um programa XDP em C, compilado com clang e carregado com `aya`, conta pacotes e bytes por IP de origem sem nunca derrubar tráfego. `ebpf watch` alimenta o detector do Safeguard. | Só Linux, como root (ou `CAP_BPF` + `CAP_NET_ADMIN`). Por enquanto só IPv4. |
| `netgene-presence` | ✅ | **Confirmação com hardware:** com `"require_user_presence": true`, uma ação só é confirmada com uma prova presa àquela proposta exata. **Windows Hello** (chave RSA-2048 no TPM, testado à mão) ou **passkeys WebAuthn** (`gene enroll-passkey`, `--confirm --passkey`): a cerimônia roda numa página `http://localhost` temporária, e o navegador oferece o autenticador do aparelho ou "usar um celular". A verificação é feita em Rust com `ring` (ES256, EdDSA, RS256; tipo, desafio, origem, RP, flags UP+UV e contador). | A atestação das passkeys não é verificada (a chave é confiável porque o próprio operador a cadastrou). macOS Touch ID nativo não foi implementado (no Mac, use passkeys). O desktop ainda confirma só com Windows Hello. |
| `netgene-p2p` | ✅ | Nó libp2p 0.57: TCP e **QUIC**, Noise + Yamux, nomes DNS, Gossipsub assinado com validação estrita, mDNS e Kademlia. O **PeerId é derivado da Gene**, e só entram nós da mesma linhagem (allowlist com chaves fixadas). As **mensagens privadas** passam por uma sessão **híbrida X25519 + ML-KEM-768**. O **gossip também é confidencial**: cada nó cifra os seus broadcasts com uma *sender key* própria, entregue só dentro dessas sessões, assinada pelo autor, repassada entre membros (malhas com vários saltos), trocada a cada hora e com proteção contra replay. **NAT traversal:** relays da própria malha (`--relay-server`, `--relay`), hole punching (DCUtR) e AutoNAT. Testado com 3 nós em Docker, em redes isoladas ligadas só por um relay. | Protocolo v2 (`/netgene/pq/2`, `netgene-mesh-v2`): não conversa com nós v0.8. Remover um membro exige reiniciar os nós (a allowlist é carregada na partida). |
| `netgene-store` | ✅ | Banco local **redb** (arquivo único, ACID, puro Rust) em `~/.netgene/store.redb`, para nós, eventos (em ordem cronológica), memória e KV dos agentes, com registrador CRDT LWW. O `sled` saiu do build padrão. | Para migrar uma base da v0.6 ou anterior, rode uma vez um build com `--features netgene-store/sled-migration` (o build padrão avisa quando encontra uma). |
| `netgene-llm` | ✅ | Cliente Ollama (chat/generate) e parser de intenções com fallback por regras. | Modelo e URL fixos (`llama3`, `localhost:11434`). |
| `netgene-kernel` | ✅ | Cada intenção passa pelo motor de políticas: leituras rodam na hora, mudanças esperam confirmação humana e o que está fora da política é negado. Toda ação aprovada tem um **executor real**, e o resultado volta para quem pediu e fica no log: `node.provision` cria sub-Genes de verdade (linhagem híbrida, chaves no cofre), `safeguard.scan` mede CPU, memória e processos do host, `system.status` mostra o estado real, e `agent.list` e `agent.run` executam **agentes WASM assinados** pela linhagem do operador (`agent install`, `agent run`), com o KV persistido entre execuções. Inclui o marketplace local com **manifestos assinados** e **reputação** resistente a Sybil. | Os agentes de demonstração antigos, que só simulavam trabalho, foram removidos. Um manifesto do marketplace ainda não carrega código (é descrição assinada). |
| `netgene-safeguard` | ✅ | Detecção de anomalias por z-score em janela deslizante, alimentada por `safeguard.scan`, pelo eBPF ou por `kernel.observe_metric`. | O "self-healing" simulado foi removido: hoje ele detecta e reporta, mas não corrige sozinho. |
| `netgene-wasm` | ✅ | **Executa** módulos com assinatura híbrida de um autor confiável (a chave tem de bater exatamente, incluindo a parte pós-quântica), com wasmtime 36 LTS, limite de CPU (fuel) e de memória. **Permissões declaradas e assinadas** (`log`, `io`, `kv`, `clock`): só são linkadas as funções de host das permissões que o módulo declara **e** que o sandbox concede, e qualquer outro import é recusado. Aceita também **componentes do Component Model** do mundo [`netgene:agent`](netgene-core/crates/netgene-wasm/wit/agent.wit) (uma interface WIT por permissão), escritos em qualquer linguagem. Não há WASI, rede nem sistema de arquivos. | Não há permissão de rede por design. |
| `netgene-vault` | 🧪 | Cifra arquivos em pedaços com AES-256-GCM. A chave fica no cofre do sistema e **sobrevive a reinícios**. Cada pedaço é ligado ao arquivo, à posição e ao total (AAD). | Só local: ainda não há replicação entre dispositivos. Arquivos de versões ≤ 0.3 não podem ser lidos (a chave antiga se perdia). |
| `netgene-desktop` | 🧪 | Painel de nós, eventos, agentes, marketplace (assina com uma Gene local), vault e terminal de intenções com botões **Confirmar/Rejeitar**, que mostra o resultado de cada ação. Age como a Master Gene local. **Também roda no Android** (Tauri mobile): as chaves ficam cifradas por uma chave do **Android Keystore**, e os dados ficam no diretório privado do app. | O APK ainda não foi publicado (falta a chave de assinatura e a conta na Play Store), e iOS não foi testado (precisa de um Mac). |
| `netgene-cli` | ✅ | Comandos `gene` (init, spawn, verify, export, import, grant, tokens, revoke-token, enroll-presence, enroll-passkey, grant-oidc, grant-saml), `agent` (dispatch e run com `--confirm` [`--passkey`], install, list, audit, policy, publish, market, review), `store`, `llm`, `p2p` (listen, connect, broadcast, peer-id; relays e NAT), `wasm` (sign, run), `ebpf` e `vault`. | — |

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

**Auto-hospedado** (nó headless, chaves no cofre cifrado em arquivo): veja [deploy/docker-compose.yml](deploy/docker-compose.yml) e [deploy/netgene-node.service](deploy/netgene-node.service). Defina `NETGENE_SECRET_STORE=file` e `NETGENE_SECRET_PASSPHRASE_FILE`. Um nó com IP público pode servir de relay para os outros membros (`p2p listen --relay-server --external-addr /ip4/<IP>/tcp/7777`). Os nós atrás de NAT usam `--relay /ip4/<IP>/tcp/7777/p2p/<PeerId do relay>`.

**Testes de malha:** `bash deploy/mesh-test/run.sh` sobe 3 nós em Docker, em duas redes isoladas ligadas só por um relay.

**Agentes:** compile um módulo ou componente WASM (o contrato está em [agent.wit](netgene-core/crates/netgene-wasm/wit/agent.wit)), assine com `wasm sign --gene <id> --permissions io,kv`, instale com `agent install` e rode com `agent run <nome> --gene <id> --confirm`.

**Fuzzing:** `cd netgene-core/fuzz && cargo +nightly fuzz run p2p_messages` (alvos: `webauthn`, `p2p_messages`, `oidc_token`, `signed_records`).

---

## Roadmap

O plano completo está em [ROADMAP.md](ROADMAP.md). Resumo:

1. **Fase 0 (concluída):** higiene do repositório, CI funcional, licença e documentação honesta.
2. **Fase 1 (concluída, v0.4.0):** chaves no cofre do sistema, vault persistente e autenticado, WASM assinado e executado de fato, marketplace assinado, P2P com identidade Gene e allowlist.
3. **Fase 2 (concluída, v0.7.0):** motor de políticas, log de auditoria, identidade e assinaturas híbridas, capability tokens, Windows Hello, runtime WASM com permissões, sessões ML-KEM no P2P e `redb`.
4. **Fase 3 (concluída, v0.8.0):** reputação no marketplace, OIDC, exportação SIEM, deploy auto-hospedado, eBPF real e app **Android** com as chaves no Android Keystore.
5. **Fase 4 (concluída, v0.9.0):** gossip confidencial pós-quântico, NAT traversal, executores e agentes WASM reais (também Component Model), passkeys, SAML, cache de JWKS, `clippy`, `cargo deny`, testes com vários nós em Docker e fuzzing.

---

## Segurança

O NetGene OS ainda não passou por auditoria de segurança externa. Use com cautela antes disso. Para reportar uma vulnerabilidade, abra uma *security advisory* privada no GitHub em vez de uma issue pública.

## Licença

Distribuído sob [MIT](LICENSE-MIT) **ou** [Apache-2.0](LICENSE-APACHE), à sua escolha.
