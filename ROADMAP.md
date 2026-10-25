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

## Fase 1: Fechar as falhas críticas

- [ ] **Vault:** derivar a chave da identidade Gene e persistir selada no keystore do sistema operacional. Usar AAD por chunk (`file_id` + índice).
- [ ] **WASM:** verificar a assinatura Ed25519 do módulo com a chave do autor. Executar de fato com fuel, `StoreLimits` e timeout. Remover o fallback que inventa bytes.
- [ ] **Marketplace:** validar `id` (só `[a-z0-9-]`), exigir manifesto assinado e verificar a assinatura na instalação.
- [ ] **Gene:** cifrar a chave privada em repouso (keystore do sistema operacional, ou senha com Argon2id).
- [ ] **P2P:** `ValidationMode::Strict`, allowlist de PeerId assinada pelo Master e vínculo PeerId ↔ Gene.
- [ ] **Passkeys:** `webauthn-rs` para confirmar ações destrutivas.
- [ ] **CI:** `cargo audit`, `cargo deny` e `clippy -D warnings`.

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
