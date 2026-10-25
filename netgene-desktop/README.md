# netgene-desktop

Aplicativo desktop do NetGene OS (Tauri 2 + React 19 + Vite + Tailwind). Ele usa os crates `netgene-kernel`, `netgene-store`, `netgene-llm` e `netgene-vault` de [`../netgene-core`](../netgene-core).

```bash
npm install
```

```bash
npm run tauri dev
```

Telas: Dashboard (nós conhecidos, eventos, terminal de intenções), Safeguard, Topologia, Agentes, Marketplace e Vault. O estado de cada módulo está no [README da raiz](../README.md#estado-real-de-cada-módulo).
