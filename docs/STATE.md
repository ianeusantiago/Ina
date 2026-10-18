# INA — Estado

**Fase atual:** 1 (MVP), passo 7 concluído (plano, aprovação, worktrees, integração, persistência).

## Feito (verificado)
1-6. Toolchain, scaffold, PTY, policy, SQLite/workspaces (ver git log).
7. Plano via `claude -p --json-schema` → aprovação → N+1 sessões; env `CLAUDE*` removido dos filhos; trust de pasta só leitura (`trust.rs`); git worktree por agente (`worktree.rs`, branch `ina/<run>/<agente>`); integração `merge --no-ff` em ordem topológica, aborta em conflito; cleanup mata sessões e remove worktrees; equipes persistidas (migração 2 `teams`) e retomáveis. E2E no app via CDP ok.

Testes: Rust 28/28 (+4 ignorados reais), clippy, tsc/eslint/prettier, Vitest 2/2.

## Em aberto
- (a) "Forçar remoção" (2ª confirmação) p/ worktree integrado com arquivos não rastreados.
- Possível sessão claude duplicada ao abrir 1 terminal (StrictMode? kill x spawn).

## Próximos passos
8. Protocolo `.ina/` (task.json, result.json, events.jsonl) + task_registry + painel de tarefas.
9. i18n/layout final, checagem de primeira execução do Claude Code.
10. code-review, security-review, simplify; docs da Fase 1.

## Bloqueios
- RAM 7,3 GB (~92% em uso): Claude Code mata `tauri dev` em background. Fechar apps pesados ao testar.

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
