# INA — Estado

**Fase atual:** 1 (MVP), passo 6 de 10 concluído (falta commit, feito na pausa).

## Feito (verificado)
1. Toolchain: Rust stable-msvc, VS Build Tools 2022, Smart App Control desligado pelo usuário.
2. Scaffold Tauri 2 + React 19 + TS strict + Vite + Tailwind 4 + xterm.js + i18next (pt-BR/EN); ESLint, Prettier, Vitest.
3. `src-tauri/src/pty.rs`: PtyManager (ConPTY) spawn/write/resize/kill + código de saída. Testes ok.
4. Terminal xterm no app (status, Reiniciar, Parar), verificado via CDP do WebView2.
5. `policy.rs`: allowlist de `.exe` (claude, powershell), cwd dentro do workspace, limites de args/sessões, `simplify` remove `\\?\`.
6. `store.rs`: SQLite (rusqlite bundled) com migrações; workspaces add/list/remove; sidebar na UI; terminal abre no diretório do workspace (verificado no app).

Testes: Rust 12/12, Vitest 2/2, tsc/eslint/prettier limpos.

## Próximos passos
7. Plano do orquestrador: `claude -p` com saída JSON (schema do plano) → tela de aprovação → spawn N+1 sessões `claude`.
8. Protocolo `.ina/` (task.json, result.json, events.jsonl) + task_registry + painel de tarefas.
9. i18n/layout final (grid redimensionável), checagem de primeira execução do Claude Code.
10. code-review, security-review, simplify; docs da Fase 1.

## Decisões
- `portable-pty` fixado em `=0.9.0`: `kill` no Windows tem resultado invertido (contornado em `PtyManager::kill`).
- ConPTY envia `ESC[6n` no início e espera resposta (xterm.js responde; testes simulam).
- Arquivos `.cmd`/`.bat` nunca são executados (injeção via cmd.exe).
- Adicionar workspace por campo de caminho; seletor de pasta exigiria `tauri-plugin-dialog` (dependência nova, pedir aprovação).

## Pendências / riscos
- Checagem do limite de sessões tem corrida (count + spawn não atômicos) — corrigir no passo 7.
- Avisos de código não usado (`is_running`, `Program::Claude`) — somem no passo 7.

## Como retomar
`RETOMAR INA`. Dev: `npm run tauri dev` (cargo em `~/.cargo/bin`). Verificação de UI: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
