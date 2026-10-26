# NetGene OS — Roadmap

Base: [AUDITORIA_2026-10.md](AUDITORIA_2026-10.md). Cada fase só começa quando a anterior estiver com CI verde.

## Fase 0: Honestidade e higiene ✅ (v0.3.0)

- [x] Remover os módulos simulados: quântico/QAOA, QPU, BCI, DAO, TPM, eBPF, K8s, lite, cloud, builder, TUI e "FHE".
- [x] Remover a tela de bloqueio que aceitava qualquer passkey.
- [x] Mover a CI para `.github/workflows/` na raiz (core em Linux e Windows, desktop em Windows).
- [x] Corrigir o Dockerfile (Rust 1.94, usuário sem root, binário `netgene`).
- [x] Adicionar `LICENSE-MIT` e `LICENSE-APACHE`.
- [x] Remover os logs de build, os planos antigos e o `SECURITY_AUDIT.md` incorreto.
- [x] Fazer o README mostrar o estado real de cada módulo.
- [x] Desktop: CSP ativada, telemetria sem números inventados e o estado do vault corrigido (tipo de `Mutex` divergente).

## Fase 1: Fechar as falhas críticas ✅ (v0.4.0)

- [x] **Gene:** a chave privada sai do disco e vai para o cofre do sistema (crate `keyring`), com migração automática dos arquivos `.key` antigos. A assinatura de linhagem agora é guardada e verificável (`verify_lineage`).
- [x] **Vault:** a chave de 256 bits é criada uma vez e fica no cofre do sistema. Cada pedaço usa AAD (`file_id` + índice + total) e um byte de versão do formato. Tamanho e metadados são conferidos na leitura.
- [x] **WASM:** os módulos são assinados por uma Gene e só rodam se o autor estiver na lista de confiança. Executam de fato com fuel, limite de memória e nenhum import de host. O fallback que inventava bytes foi removido. O wasmtime passou de 18 (que **abortava o processo** em qualquer trap) para 36 LTS.
- [x] **Marketplace:** ids validados (`[a-z0-9-]`, sem path traversal), manifestos assinados e verificados ao publicar, listar e instalar. Os agentes de demonstração falsos foram removidos.
- [x] **P2P:** `ValidationMode::Strict` com aceitação no nível da aplicação, PeerId derivado da Gene e allowlist formada pela Master e por suas sub-genes com linhagem válida.
- [x] **CLI:** logs vão para stderr (o `--json` fica limpo) e há um teste de `debug_assert` do clap.
- [x] **CI:** `cargo audit` adicionado.
- [ ] **Passkeys (adiado):** `webauthn-rs` exige uma origem HTTPS ou `localhost` no WebView do Tauri. Fica junto do motor de políticas na Fase 2 (a tela de bloqueio falsa já foi removida na Fase 0).
- [x] **libp2p 0.54 → 0.57:** feito na v0.5.0. O `cargo audit` agora reporta 0 vulnerabilidades no core e no desktop.
- [x] **CI:** `cargo deny` e `clippy -D warnings` (v0.9.0).
- [x] **Linux/Docker:** build validado no WSL e no Docker. O fluxo do `deploy/docker-compose.yml` foi testado de ponta a ponta: `gene init` no volume, `secrets.enc` cifrado com permissão 0600, senha errada recusada, nó no ar com rootfs só leitura, sem capabilities e sem root.
- [ ] **Linux:** testar o cofre Secret Service num desktop real e rodar o CI (bloqueado pela cobrança do GitHub Actions).

## Fase 2: Núcleo real ✅ (v0.5.0 – v0.7.0)

- [x] **Identidade híbrida pós-quântica:** cada Gene nova tem uma chave Ed25519 e uma ML-DSA-65 (`ml-dsa` da RustCrypto, FIPS 204). A linhagem é assinada pelas duas, e a assinatura PQ cobre as duas chaves do filho, então retirá-la ou trocá-la é detectado. A semente PQ fica no cofre do sistema.
- [x] **Sessões P2P pós-quânticas** (v0.7.0): o protocolo `/netgene/pq/1` faz um handshake X25519 + ML-KEM-768, assinado pelas duas Genes (chaves fixadas pela allowlist), e deriva as chaves com HKDF sobre o transcript. As mensagens privadas usam ChaCha20-Poly1305 com contador anti-replay (CLI: `p2p connect --private`).
- [x] **Gossip confidencial pós-quântico** (v0.9.0): sender keys por autor, entregues só dentro das sessões ML-KEM, assinadas, repassadas entre membros e trocadas a cada hora.
- [x] **Assinaturas híbridas** (v0.6.0) nos manifestos do marketplace, nos módulos WASM, nos tokens e no log de auditoria, todas por uma única primitiva que liga a chave pública aos bytes assinados. O sandbox WASM exige que a chave do autor bata exatamente com a confiável. Logs de auditoria da v0.5 são arquivados como `audit.jsonl.v1`.
- [x] **Capability tokens** (v0.6.0): a Gene mãe delega capacidades a uma filha por um tempo limitado (`gene grant`). Os tokens são assinados e revogáveis (`gene revoke-token`, `~/.netgene/revoked-tokens.json`), e quem emite só delega o que tem. A política conta só os tokens válidos naquele momento.
- [x] **Motor de políticas (`netgene-policy`):** o LLM só **propõe** e a política **decide**. Há um catálogo de ações (capacidade + risco), regras em `~/.netgene/policy.json` e limites de parâmetros. As ações de escrita exigem confirmação humana (CLI `--confirm`, botão no desktop).
- [x] **Runtime WASM com permissões** (v0.7.0): o manifesto assinado declara as permissões (`log`, `io`, `kv`, `clock`), o sandbox concede um subconjunto (`--allow`) e só essas funções de host são linkadas, com cotas por permissão e ponteiros verificados. O guest roda numa thread com 8 MB de pilha (evita estouro na thread principal do Windows).
- [x] **WASM Component Model** (v0.9.0): mundo `netgene:agent@0.1.0`, com uma interface WIT por permissão, ao lado da ABI v2. O KV de cada agente persiste no store.
- [x] **Log de auditoria append-only:** encadeado por SHA3-256 e assinado por entrada. O kernel recusa continuar um log adulterado. CLI: `agent audit`.
- [x] **Confirmação com hardware** (v0.6.0, `netgene-presence`): com `require_user_presence`, uma ação `confirm` só é aprovada com uma assinatura do Windows Hello (RSA-2048 no TPM, verificada com `ring`) sobre o desafio daquela proposta. A falha também vai para o log de auditoria.
- [x] **Passkeys WebAuthn** (v0.9.0): aprovação pelo celular ou por outro autenticador, verificada em Rust com `ring` (sem `webauthn-rs`/OpenSSL).
- [ ] Touch ID nativo no macOS (no Mac, as passkeys já cobrem esse caso) e aprovação por passkey no desktop.
- [x] **`sled` → `redb` 4.4** (v0.7.0), com migração automática e retomável (um marcador grava na mesma transação da importação). Os eventos agora são listados em ordem cronológica (antes vinham em ordem aleatória).
- [x] `sled` fora do build padrão (v0.9.0); a migração continua disponível com `--features netgene-store/sled-migration`.
- [ ] Avaliar `automerge` ou `loro` para CRDTs (quando houver sincronização de estado entre dispositivos).
- [x] **NAT traversal** (v0.9.0) com o próprio libp2p: QUIC, relays da malha, DCUtR e AutoNAT. O `iroh` não foi necessário.
- [x] **Testes com 3+ nós em Docker** (`deploy/mesh-test`, também no CI) e **fuzzing** com `cargo-fuzz` (`netgene-core/fuzz`) (v0.9.0).

## Fase 3: Produto ✅ (v0.8.0)

- [x] **Reputação no marketplace:** avaliações com assinatura híbrida, presas ao digest do manifesto. Só revisores da sua linhagem contam (resistente a Sybil), há uma avaliação por revisor, autoavaliações são descartadas e a nota é uma média bayesiana.
- [x] **App Android (Tauri mobile):** APK de debug para arm64 e x86_64. As chaves ficam no **Android Keystore** (`android-native-keyring-store`), e os dados no diretório privado do app. Testado no emulador (API 36): a chave do vault fica cifrada e sobrevive a reinícios.
- [x] **eBPF real (`aya` 0.14):** contador XDP em C. No WSL2, `ebpf watch` contou 6008 pkt/s e o Safeguard marcou Critical.
- [x] **Integração empresarial:** OIDC (`gene grant-oidc`), exportação de auditoria em OCSF e CEF, cofre cifrado em arquivo para servidores, Docker Compose e systemd.
- [x] **Cache de JWKS** e **SAML 2.0** (recurso opcional `saml`) (v0.9.0).
- [ ] Publicação do APK assinado na Play Store (depende da chave de assinatura e da conta de desenvolvedor) e iOS (depende de um Mac com Xcode).
- [ ] Módulos avançados (TPM via `tss-esapi`, otimização) apenas quando houver um caso de uso concreto.

## Fase 4: Completar o núcleo ✅ (v0.9.0)

- [x] **Agentes reais:** os cinco agentes que simulavam trabalho saíram. Cada ação do catálogo tem agora um executor que faz o trabalho e devolve o resultado: sub-genes de verdade, métricas do host e agentes WASM assinados (`agent.list`, `agent.run`).
- [x] Gossip pós-quântico, NAT traversal, Component Model, passkeys, SAML, cache de JWKS, `sled` opcional, `clippy`/`cargo deny`, testes com vários nós e fuzzing (detalhes acima).
- [x] Corrigidos durante os testes: um handshake PQ que ficava pendente para sempre quando a mensagem saía antes da conexão; `p2p connect --private` entregava a mensagem ao relay em vez do destino; `secrets.enc` era legível por todos (0644); o volume do Docker ficava com dono root.

## O que não depende de código

- [ ] **Auditoria de segurança externa.** É o que falta para outros dependerem do NetGene em produção.
- [ ] **CI no GitHub Actions** (bloqueado pela cobrança da conta; tudo é verificado localmente até lá).
- [ ] Publicação do APK assinado na Play Store e build/teste no iOS.
