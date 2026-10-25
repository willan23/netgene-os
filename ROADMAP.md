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
- [ ] **libp2p 0.54 → 0.57:** remove os avisos do `cargo audit` em `hickory-proto`, `rustls-webpki` 0.101, `ring` 0.16 e `h2` 0.3 (transitivos).
- [ ] **CI:** `cargo deny` e `clippy -D warnings`.
- [ ] **Linux:** validar o build e o cofre (Secret Service) no Docker e no CI.

## Fase 2: Núcleo real

- [ ] Identidade híbrida pós-quântica: Ed25519 + ML-DSA-65 nas assinaturas e X25519 + ML-KEM-768 na troca de chaves (`aws-lc-rs` ou RustCrypto).
- [ ] Capability tokens assinados com expiração e revogação.
- [ ] Motor de políticas: o LLM só **propõe**, a política **decide** e as ações sensíveis exigem confirmação.
- [ ] Runtime de agentes em WASM Component Model com permissões declaradas (rede, arquivos, chaves).
- [ ] Log de auditoria append-only e assinado.
- [ ] Migrar `sled` para `redb`. Avaliar `automerge` ou `loro` para CRDTs.
- [ ] Avaliar `iroh` para NAT traversal.
- [ ] Testes de integração com 3+ nós em Docker e fuzzing com `cargo-fuzz`.

## Fase 3: Produto

- [ ] Marketplace de agentes assinados com reputação.
- [ ] App mobile (Tauri mobile).
- [ ] eBPF real no Linux (`aya`) para telemetria de rede.
- [ ] Integração empresarial: OIDC/SAML, exportação de auditoria e deploy auto-hospedado.
- [ ] Módulos avançados (TPM via `tss-esapi`, otimização) apenas quando houver um caso de uso concreto.
