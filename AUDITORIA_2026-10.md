# Auditoria Técnica Completa — NetGene OS

**Data:** 25/10/2026 · **Commit auditado:** `0b3bc3a` (branch `main`) · **Escopo:** `netgene-core` (21 crates Rust, ~8.300 linhas), `netgene-desktop` (Tauri 2 + React), Dockerfiles, CI, documentação.

---

## 1. Veredito em uma frase

**Vale a pena continuar, mas não do jeito que está.** Hoje o NetGene OS é um **protótipo de demonstração bem organizado**, não um sistema do qual outros possam depender. Boa parte do que o README e o `SECURITY_AUDIT.md` descrevem como pronto (pós-quântico, TPM, eBPF, QPU, passkeys, FHE, sandbox WASM) é **simulado, hardcoded ou ausente**. Para virar uma fundação confiável, ele precisa encolher o escopo, trocar simulações por bibliotecas reais e ser honesto sobre o que existe.

---

## 2. O que está bom (o que vale aproveitar)

| Área | Avaliação |
|---|---|
| Estrutura do workspace | Boa separação em crates, workspace dependencies centralizadas, Rust 2021. Fácil de evoluir. |
| Identidade (`netgene-gene`) | Ed25519 via `ring`, fingerprint SHA3-256, `Zeroizing` na chave privada. Base correta. |
| P2P (`netgene-p2p`) | libp2p real (TCP + Noise + Yamux, Gossipsub assinado, mDNS, Kademlia, Identify). É a parte mais "de verdade" do sistema. |
| Vault (`netgene-vault`) | AES-256-GCM via `ring` com nonce aleatório por chunk. A primitiva está correta (o gerenciamento da chave não, ver abaixo). |
| CRDT LWW (`netgene-store`) | Merge com desempate determinístico por `writer_id`. Simples e correto. |
| Kernel multi-agente | Agentes em tasks Tokio com message bus. Um bom esqueleto para orquestração. |
| Desktop | Tauri 2 + React 19 + Zustand + Three.js. Stack moderna e leve. |

---

## 3. Achados críticos (bloqueiam qualquer uso real)

| # | Severidade | Onde | Problema |
|---|---|---|---|
| C1 | 🔴 Crítico | `netgene-mobile/src/passkey.rs` `verify_response` | **Sempre retorna `Ok(true)`**. Nenhuma assinatura WebAuthn é verificada. O mesmo vale para `verify_passkey` no desktop (`src-tauri/src/lib.rs`). Qualquer pessoa "passa" na biometria. |
| C2 | 🔴 Crítico | `netgene-kernel/src/crypto.rs` | O "FHE" usa **primos fixos no código** (`1000000007`, `1000000009`). Qualquer pessoa consegue fatorar `n` e decifrar tudo. Além disso, Paillier é **parcialmente** homomórfico (só soma), não FHE. |
| C3 | 🔴 Crítico | `netgene-tpm/src/lib.rs` | "Selar no TPM" só **prefixa `TPM2_SEALED:` no texto puro**. Não existe nenhuma proteção. O segredo continua legível. |
| C4 | 🔴 Crítico | `netgene-vault/src/lib.rs` | A chave AES é **gerada aleatoriamente a cada `new()` e nunca é salva**. Depois de reiniciar o processo, **todos os arquivos do vault ficam irrecuperáveis**. Também não há AAD ligando o chunk ao arquivo, então os chunks podem ser trocados ou reordenados sem detecção. |
| C5 | 🔴 Crítico | `netgene-wasm/src/sandbox.rs` | A "verificação de assinatura" só confere se a string **não está vazia**. O módulo **nunca é executado de fato**: só é compilado, e o resultado devolvido é JSON fabricado com `"status": "EXECUTED"`. Bytes base64 inválidos viram um cabeçalho WASM válido por fallback. |
| C6 | 🔴 Crítico | `netgene-kernel/src/marketplace.rs` `publish_agent` | **Path traversal**: `manifest.id` entra direto no nome do arquivo. Um `id` como `../../x` grava fora do diretório. Também não há assinatura nem verificação de origem dos agentes. |
| C7 | 🟠 Alto | `netgene-gene/src/storage.rs` | A chave privada é salva em **base64 em texto puro**. No Windows, nem as permissões restritas são aplicadas (`#[cfg(unix)]`). |
| C8 | 🟠 Alto | `src-tauri/tauri.conf.json` | `"csp": null`. Sem Content Security Policy no app desktop, qualquer XSS no frontend chega aos comandos Tauri (FHE, vault, intents). |
| C9 | 🟠 Alto | `netgene-p2p` | `ValidationMode::Permissive` no Gossipsub e nenhuma autorização de peer: qualquer nó na LAN (mDNS) entra na malha e publica `MeshMessage`. Não há vínculo entre o PeerId libp2p e a identidade Gene. |
| C10 | 🟠 Alto | `netgene-kernel/src/kernel.rs` | A saída do LLM vira ação de infraestrutura sem política, sem confirmação e sem escopo. Isso abre espaço para *prompt injection* virar comando. O cliente Ollama é recriado a cada intent, com URL e modelo hardcoded. |

---

## 4. Simulação apresentada como real (risco de credibilidade)

Isso não é bug de código, mas é o maior risco para o projeto: **quem depender dele vai confiar em garantias que não existem.**

| Afirmação no README / docs | Realidade no código |
|---|---|
| "Kyber/Dilithium post-quantum cryptography" | Só existe um `enum` com os nomes `MlKem768` e `MlDsa65`. **Nenhuma linha de PQC.** Tudo é Ed25519. |
| "Linux eBPF XDP zero-copy" | `sample_kernel_probe()` devolve **constantes fixas** (IP `10.42.0.1`, RTT 1450, entropia 7.82). `aya` é dependência, mas nenhum programa eBPF é carregado. |
| "QAOA / quantum optimization" | `compute_cost()` **ignora a matriz QUBO** e usa um custo fixo `Σ b·(i+1)`. A "evolução" do estado não é unitária. Na prática é amostragem aleatória com best-of, comparada a uma média aleatória para inventar o "% de melhoria". |
| "IBM Quantum / AWS Braket / IonQ" | `submit_openqasm` devolve **contagens hardcoded** (`0011: 42%`, `1100: 45%`). `submit_qubo` devolve `energy: -128.45` fixo. Nenhuma chamada HTTP. |
| "TPM 2.0 / Secure Enclave" | Ver C3. |
| "WebAuthn biometrics" | Ver C1. |
| "Fully Homomorphic Encryption" | Ver C2. |
| "Self-healing" | `healing.rs` só faz `sleep` e registra "Simulated healing". |
| "Neural BCI", "DAO", "K8s Operator" | Mocks e estruturas de dados. Não há leitura de EEG, nem consenso distribuído real, nem operador `kube-rs` funcional (cai em "simulation mode"). |
| `SECURITY_AUDIT.md`: "APROVADO — ZERO VULNERABILIDADES CRÍTICAS" | **Incorreto.** Cita "parsing obrigatório de ML-DSA-65" (não existe), "Ed25519 signature check before exec" no WASM (não existe) e "RTT dinâmico" no eBPF (é constante). Recomendo retirar ou reescrever esse documento. |
| Badge "License MIT/Apache-2.0 — see LICENSE" | **Não há arquivo `LICENSE`** no repositório. |

Há 54 ocorrências de `simulate`/`mock`/`In a real…` no código. Sozinho isso não é problema: o problema é o README tratar esses módulos como prontos.

---

## 5. Engenharia, build e operação

| Item | Situação |
|---|---|
| CI | O workflow está em `netgene-core/.github/workflows/ci.yml`. **O GitHub só lê `.github/` na raiz do repositório**, então o CI nunca roda. |
| `Dockerfile` da raiz | Roda `cargo build` na raiz, onde **não existe `Cargo.toml`**, e o build falha. Ele também usa `rust:1.80` com dependências que exigem um compilador mais novo. |
| Arquivos de lixo versionados | `build_errors.txt`, `build_full.txt`, `test_results*.txt`, `implementation_plan2` e `.netgene/marketplace/*.json` dentro de `src-tauri`. |
| Testes | 101 testes (2 falham, ver seção 9), quase todos unitários e verificando o próprio mock (ex.: o teste do eBPF confere que o tamanho é 1420, a constante). Não há testes de integração multi-nó, fuzzing nem property tests no CRDT. |
| `unwrap()` fora de testes | 63 ocorrências. No desktop, `status.lock().unwrap()` derruba o app se o mutex for envenenado. |
| Versões inconsistentes | README "1.0.0 Apex", workspace `0.2.0`, desktop `0.1.0`, identify `/netgene/0.2.0`. |
| Dependências | `sled 0.34` está sem manutenção ativa desde 2021 (alternativas: `redb`, `fjall`). `bincode 1` foi substituído pelo 2.x. Rodar `cargo audit` e `cargo deny` na CI. |
| Identidade do app | `identifier: com.user.netgene-desktop` (placeholder). A janela tem título `netgene-desktop` em 800x600. |

**Resultado de `cargo test --workspace` nesta auditoria (Rust 1.94.1):** ver seção 9.

---

## 6. Pesquisa: o que existe lá fora (e o que aproveitar)

**Concorrentes diretos na camada de malha e zero-trust:** NetBird, Headscale/Tailscale, Nebula, Netmaker, OpenZiti. Todos já entregam malha WireGuard com controle de acesso, SSO e políticas, e são maduros. **Competir com eles em "VPN mesh" é perder.** O NetGene só se diferencia se a malha for um meio para a camada de agentes e intents, não o produto.

**Camada de "agent OS" em Rust:** projetos como OpenFang (binário Rust único, mais de 16 mil estrelas) e agentOS (Rivet) mostram que existe demanda, mas também concorrência forte. O diferencial possível do NetGene: **agentes locais com identidade criptográfica, permissões verificáveis e execução sandboxada em uma malha P2P privada.**

**Bibliotecas que substituem os mocks:**

| Mock atual | Substituto real recomendado |
|---|---|
| PQC fake | `aws-lc-rs` (ML-KEM; ML-DSA ainda instável) ou RustCrypto `ml-kem` / `ml-dsa`. **Atenção:** nenhuma das implementações PQ em Rust tem auditoria independente concluída. Use **modo híbrido** (X25519 + ML-KEM-768; Ed25519 + ML-DSA-65), nunca PQ sozinho. |
| Passkey `true` | `webauthn-rs` (kanidm): `start/finish_passkey_registration` e `start/finish_passkey_authentication`. |
| TPM fake | `tss-esapi` no Linux. No Windows, o keystore via `windows` crate (CNG/NCrypt). Em Apple, Secure Enclave via `security-framework`. Sem hardware, **dizer isso** em vez de fingir. |
| eBPF fake | `aya` está ativo (FOSDEM 2026, rumo ao Tier 2 no Rust stable). Começar com um programa XDP/TC mínimo que só conta pacotes por IP. |
| Sandbox WASM | `wasmtime` com fuel, `StoreLimits` (memória), epoch interruption, WASI p2 / Component Model e assinatura Ed25519 **verificada** do módulo. |
| P2P | Avaliar **iroh 1.0** (lançado em junho de 2026: discagem por chave pública, relays, QUIC). Ele resolve NAT traversal muito melhor do que um setup libp2p cru. Ou manter libp2p e adicionar allowlist de PeerId ligada ao Gene. |
| QAOA fake | Implementação honesta: chamar de "otimização clássica (simulated annealing)" ou integrar um simulador real (Qiskit via Python ou API real de QPU com chave do usuário). Para roteamento, um solver clássico (OR-Tools, ILP) vence QAOA simulado em qualquer tamanho prático. |
| FHE fake | Se realmente for necessário: TFHE-rs (Zama). Na prática, provavelmente **não é** para o caso de uso, e o recomendado é remover. |
| `sled` | `redb` (estável, puro Rust) ou `fjall`. |

---

## 7. Minha visão

O nome, a estética (Blame!, "megastructure") e a ambição são ótimos para marca. O problema é técnico: **o projeto tentou ser 21 coisas e não é nenhuma de verdade ainda.** Um sistema do qual outros vão depender precisa de **poucas garantias, todas verdadeiras.**

### O núcleo que eu construiria (NetGene "Core 1.0 real")

1. **Identidade Gene**: chave Ed25519 + ML-DSA híbrida, guardada no keystore do sistema operacional, com hierarquia Master → Node → Agent e **capability tokens assinados** com expiração e revogação.
2. **Malha privada**: iroh ou libp2p, onde só entra quem tem um Gene assinado pelo Master. Mensagens tipadas, versionadas e autenticadas.
3. **Runtime de agentes sandboxado**: plugins WASM assinados, com limites de memória, CPU e tempo e **permissões declaradas** (rede, arquivos, chaves). O LLM só *propõe* ações; um motor de políticas decide, e ações destrutivas exigem confirmação via passkey real.
4. **Estado replicado**: `redb` + CRDTs (considerar `automerge` ou `loro` em vez do LWW feito à mão) com log de auditoria assinado e append-only.
5. **Uma interface**: CLI + Desktop. TUI, mobile, BCI, DAO, K8s e quântico ficam **fora** até o núcleo estar sólido, ou ficam marcados claramente como `experimental/`.

O diferencial vendável deixa de ser "quântico + BCI" e passa a ser: **"seu enxame privado de agentes de IA, com identidade criptográfica e permissões verificáveis, rodando nos seus dispositivos."** Isso é defensável, útil hoje e não compete de frente com Tailscale ou NetBird.

### Roadmap sugerido

| Fase | Duração estimada | Entregas |
|---|---|---|
| **0. Honestidade e higiene** | 1 semana | Mover a CI para a raiz, corrigir o Dockerfile, adicionar `LICENSE`, remover lixo do repo, reescrever o README com status real por módulo (✅ real / 🧪 experimental / 🎭 mock), retirar o `SECURITY_AUDIT.md` atual. |
| **1. Fechar os furos críticos** | 2–3 semanas | C1 a C10: passkey real (`webauthn-rs`), remover ou isolar o "FHE", persistir a chave do vault (derivada do Gene, selada no keystore), AAD por chunk, assinatura WASM verificada, saneamento do `id` do marketplace, CSP no Tauri, allowlist de peers. |
| **2. Núcleo real** | 1–2 meses | Identidade híbrida PQ, malha autenticada, runtime WASM com permissões, motor de políticas para intents do LLM, `redb`. Testes de integração com 3+ nós em Docker, fuzzing (`cargo-fuzz`) em parsers, `cargo audit` e `cargo deny` na CI. |
| **3. Produto** | depois | Marketplace com assinatura e reputação, mobile, eBPF real no Linux, integrações. O quântico entra só se houver um problema concreto que o justifique. |

---

## 8. Resposta direta: vale a pena terminar?

**Sim, se você aceitar recomeçar o núcleo com escopo menor.** A estrutura em Rust, a identidade Ed25519, o libp2p e o desktop Tauri são uma boa base, e cerca de 30% do código é aproveitável. **Não**, se a ideia for continuar adicionando módulos simulados: cada mock a mais aumenta o risco de alguém confiar numa garantia falsa. Para "um sistema do qual todos os outros vão depender", isso é o pior cenário possível.

---

## 9. Resultado dos testes nesta auditoria

`cargo test --workspace --no-fail-fast` com Rust 1.94.1 no Windows 11 (Ollama ativo, mas sem o modelo `llama3`):

- **101 testes: 99 passaram e 2 falharam.** Os 2 falham em `netgene-kernel`: `test_kernel_dispatch_spawn_intent` e `test_kernel_dispatch_unknown_intent`.
- **Causa (corrigida após investigação):** os testes estavam desatualizados. Eles procuravam o texto "Intent dispatched", mas o kernel responde "Action dispatched". O fallback por regras do `netgene-llm` funciona. Os testes foram atualizados na Fase 0.
- Sem o `--no-fail-fast`, o `cargo test` para no `netgene-kernel`, então os crates depois dele nem rodam. A CI atual (se rodasse) ficaria vermelha.
- A maioria dos testes que passam verifica o comportamento do próprio mock (ex.: tamanho de pacote 1420 no eBPF, `is_hardware_backed == false` no TPM). Eles não dão garantia funcional.

---

### Fontes da pesquisa

- [Project Eleven — The state of post-quantum cryptography in Rust](https://www.projecteleven.com/blog/the-state-of-post-quantum-cryptography-in-rust-the-belt-is-vacant)
- [RustCrypto ml-dsa (docs)](https://doc.servo.org/ml_dsa/index.html)
- [bc-rust 0.1.2alpha](https://github.com/bcgit/bc-rust/wiki/0.1.2alpha-Is-Released!)
- [Iroh 1.0: Dial Keys, Not IPs](https://pinggy.io/blog/iroh_1_0_dial_keys_not_ips/)
- [The Deceptive Complexity of P2P Connections (libp2p vs iroh)](https://ark-builders.dev/blog/p2p-networking-webrtc-libp2p-iroh/)
- [FOSDEM 2026 — Aya: what's new in Rust for eBPF](https://fosdem.org/2026/schedule/event/aya)
- [webauthn-rs (docs.rs)](https://docs.rs/webauthn-rs/)
- [Mesh VPN comparison 2026: NetBird, Tailscale, Headscale, ZeroTier, Netmaker, Nebula](https://wz-it.com/en/blog/mesh-vpn-comparison-2026-overview/)
- [OSS Insight — Rust AI agent infrastructure 2026](https://ossinsight.io/blog/rust-ai-agent-infrastructure-2026)
