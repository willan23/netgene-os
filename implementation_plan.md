# Integração do NetGene Desktop com netgene-store

Este plano descreve como vamos conectar a interface desktop persistente ao nosso banco de dados local (Sled DB) do `netgene-store`, bem como as atualizações necessárias na documentação do sistema.

## User Review Required

> [!IMPORTANT]
> A integração com o `netgene-store` vai requerer o carregamento da base de dados local sempre que o Desktop for aberto. Valide se prefere que a app recupere automaticamente o histórico de logs/nós da base de dados no arranque.

## Open Questions

1. **Gestão de Estado Inicial:** Quando a App Desktop iniciar, devemos preencher o Dashboard imediatamente com os nós históricos guardados em `netgene-store` ou prefere que a app inicie com o ecrã limpo e só guarde o que acontece durante a sessão atual?
2. **Atualização da Arquitetura:** Vou modificar o `ARCHITECTURE.md` para incluir a Camada de Desktop e adicionar o estado do Desktop App no `ROADMAP_PHASES.md`. Está de acordo?

## Proposed Changes

### Documentação (Atualizar Primeiro)

#### [MODIFY] [ROADMAP_PHASES.md](file:///w:/NetGene%20OS/netgene-core/docs/ROADMAP_PHASES.md)
Adicionar a nova aplicação Desktop na listagem de conquistas (talvez agrupada na Fase 7 juntamente com a PWA ou como um milestone bónus da Megastructure v1.0.0).

#### [MODIFY] [ARCHITECTURE.md](file:///w:/NetGene%20OS/netgene-core/docs/ARCHITECTURE.md)
Atualizar o diagrama Mermaid principal para incluir o bloco de UI: `Tauri Frontend -> IPC Commands -> Kernel / Store`.

### Código

#### [MODIFY] [Cargo.toml](file:///w:/NetGene%20OS/netgene-desktop/src-tauri/Cargo.toml)
Adicionar a dependência para o `netgene-store`:
```toml
netgene-store = { path = "../../netgene-core/crates/netgene-store" }
```

#### [MODIFY] [lib.rs](file:///w:/NetGene%20OS/netgene-desktop/src-tauri/src/lib.rs)
- Inicializar `let store = netgene_store::NetGeneStore::open(None).unwrap();` no momento do setup do Tauri.
- Injetar a `store` no estado gerido do Tauri (`app.manage(store)`).
- Modificar os Comandos (ex: `dispatch_intent`) para registarem os eventos (`StoredEvent`) na persistência.
- Criar novos comandos Tauri para permitir ao Frontend carregar o histórico: `get_stored_events()` e `get_stored_nodes()`.

## Verification Plan

1. Vou alterar primeiro os 2 documentos oficiais do core e guardá-los.
2. Em seguida, ligo as dependências e o código do Rust backend.
3. Finalmente, vou compilar o backend `cargo check` para garantir que o acesso ao Store via Tauri Managed State está sem erros.
