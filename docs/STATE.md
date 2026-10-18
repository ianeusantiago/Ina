# INA — Estado

**Fase atual:** 2 em andamento (pausa 2026-10-18).

## Fase 2 — feito (commitado até b0a00cb)
Recuperação no startup, agendador por dependências, mensagens estruturadas (outbox/inbox), verificação de resultados, prévia de integração (merge-tree) e reexecução com previousAttempt.

## Em andamento (commit "wip: pausa", NÃO verificado)
23 arquivos alterados (backend: lib/messages/plan/pty/store/tasks/verify/worktree; UI: painéis, i18n) + novos: e2e/ (Playwright), playwright.config.ts, ui.ts, usePolled.ts.

## Próximos passos
1. Rodar testes (cargo test, clippy, tsc/eslint, Vitest, Playwright) sobre o wip e corrigir.
2. code-review/simplify da Fase 2; atualizar docs/CHANGELOG.
3. Integrar branch ao main (decisão do usuário).

## Feito (verificado)
1-6. Toolchain, scaffold, PTY, policy, SQLite/workspaces.
7. Plano → aprovação → N+1 sessões; trust só leitura; worktree por agente; integração controlada; equipes persistidas; limpeza (inclui forçada com 2ª confirmação).
8. Protocolo `.ina/` (task.json, result.json, events.jsonl) + registro de tarefas (SQLite) + painel. E2E real ok.
9. Checagem de primeira execução do Claude Code, seletor de idioma, maximizar terminal.
10. code-review (10 achados; corrigidos: trust no planner, env só de sessão, exclude sem sobrescrever, integração parcial reportada, write do PTY sem lock global, CSP, dependências/sequencial impostos, corrida no spawn, dependência duplicada); simplify (helpers, consultas, kill sinalizado pelo PTY, kill-antes-do-spawn recusado no backend, team_tasks fora da thread principal); docs (README EN/pt-BR, LICENSE MIT, SECURITY, CONTRIBUTING, CHANGELOG, templates).

Testes: Rust 37 (+5 ignorados reais), clippy, tsc/eslint/prettier, Vitest 4.

## Pendências conhecidas
- Orquestrador: deny list é guarda, não sandbox (documentado em SECURITY.md).
- Agente com dependência precisa ser aberto manualmente quando os pré-requisitos concluem.
- Ao rodar de novo uma tarefa, o resumo antigo aparece até o novo resultado.
- `team_tasks` ainda corrige tarefas "running" sem sessão (melhor: varredura no startup).
- Cópia TS de `prerequisites` espelha a Rust (poderia vir do backend).
- Redimensionar painéis por arrasto não implementado (só maximizar).

## Bloqueios
- RAM 7,3 GB: Claude Code mata `tauri dev` em background; fechar apps pesados ao testar.

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
