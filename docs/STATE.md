# INA — Estado

**Fase atual:** 2 (orquestração confiável) concluída em 2026-10-18; aguardando revisão do usuário e integração ao `main`. Próximo: planejar a Fase 3.

## Feito (verificado)
Fase 1 (MVP): passos 1–10 (PTY, policy, SQLite, plano → aprovação → N+1 sessões, worktrees, integração, protocolo `.ina/`, registro de tarefas, i18n, docs).
Fase 2: recuperação no startup; agendador por dependências com auto-start; mensagens estruturadas (outbox/inbox com aprovação); verificação de resultados (`verifying`); prévia de integração (merge-tree); `previousAttempt`; divisor arrastável; E2E Playwright via CDP; README principal em pt-BR.
Fechamento: code-review (6 corrigidos + outbox sem perda + limite em caracteres; achado sobre allow/deny do Edit era falso positivo — verificado com `claude -p`); simplify (helpers de processo, estilos compartilhados, `usePersistedState`); docs (CHANGELOG, SECURITY).

Testes: Rust 50 (+6 ignorados reais), clippy -D warnings, tsc/eslint/prettier, Vitest 4, Playwright 5 (inclui fluxo real de equipe com `INA_E2E_WORKSPACE=ws-todo`).

## Pendências conhecidas
- Orquestrador: deny list é guarda, não sandbox (SECURITY.md).
- Cópia TS de `prerequisites` espelha a Rust (poderia vir do backend).
- E2E do fluxo real falhou 1x (plano gravado no banco, UI não recebeu resposta; não reproduziu em 3 execuções seguintes).
- `team_tasks` não finaliza mais tarefas `running` sem sessão (só a recuperação no startup cobre corrida rara de 2 sessões saindo juntas).
- Arquitetura (do simplify, para a Fase 3): estado avança só com polling da UI (ideal: watcher no backend + eventos); `static RUNNING`/`MERGING` globais (ideal: lease no banco / trava por agente); um thread de timeout por sessão (ideal: reaper no `PtyManager`); `Registry::task` tipado; backfill de equipes antigas sem registro; cache do `Team` decodificado; git status em paralelo; listar só mensagens novas.

## Bloqueios
- RAM 7,3 GB: Claude Code mata `tauri dev` em background; fechar apps pesados ao testar.

## Decisões
- `portable-pty` fixado em `=0.9.0`: `kill` no Windows tem resultado invertido (contornado em `PtyManager::kill`).
- ConPTY envia `ESC[6n` no início e espera resposta (xterm.js responde; testes simulam).
- Arquivos `.cmd`/`.bat` nunca são executados (injeção via cmd.exe).
- Adicionar workspace por campo de caminho; seletor de pasta exigiria `tauri-plugin-dialog` (dependência nova, pedir aprovação).

## Como retomar
`RETOMAR INA`. Dev: `npm run tauri dev` (cargo em `~/.cargo/bin`). Verificação de UI: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
