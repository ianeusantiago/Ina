# INA — Estado

**Fase atual:** 5 (pendências finais), branch `ianeusantiago/fase5-pendencias`. Fases 1–4 no `main` (PR #3, #4, #5).

## Fase 5 — pendências finais (branch `ianeusantiago/fase5-pendencias`)
- Orquestrador: deny list cobre também a ferramenta PowerShell e mais formas de git (`-C`, `-c`, restore, stash, worktree, update-ref, branch -f/-D). Verificado com `claude -p` (PowerShell deny e `git -C` bloqueados; controle sem deny commitou). Rust 53, clippy, fmt.
- E2E Playwright 5/5 em 3 execuções seguidas (fluxo real `ws-todo`): falha intermitente não reproduziu.
- Próximo: PR.

## Fase 4 — pendências (PR #5 merged, 85ca40e)
- Tick finaliza tarefas órfãs; evento leva tarefas; `prerequisites` do backend; restauração de rascunho. E2E 5/5, code-review sem achados.

## Fase 3 — feito (verificado)
- Tick no backend (5 s) avança equipes sem UI (provado: mensagem no outbox ingerida com painel fechado); recuperação fora do startup.
- `Registry::tasks/task` tipados.
- git status das worktrees em paralelo.
- Evento `team-changed` do tick → painéis atualizam (provado: mensagem no painel em ~6 s; polling de reserva 15 s).
- Timeouts pelo tick (`Deadlines`); claim RAII de verificação; trava de merge por worktree; cache de equipes aprovadas; backfill do registro (integração sempre exige done).
- code-review da Fase 3: 7 achados corrigidos (recuperação antes do tick, corrida do cache, pânico no tick, tick durante limpeza, cache ao remover workspace, docs).

## Feito (verificado)
Fase 1 (MVP): passos 1–10 (PTY, policy, SQLite, plano → aprovação → N+1 sessões, worktrees, integração, protocolo `.ina/`, registro de tarefas, i18n, docs).
Fase 2: recuperação no startup; agendador por dependências com auto-start; mensagens estruturadas (outbox/inbox com aprovação); verificação de resultados (`verifying`); prévia de integração (merge-tree); `previousAttempt`; divisor arrastável; E2E Playwright via CDP; README principal em pt-BR.
Fechamento: code-review (6 corrigidos + outbox sem perda + limite em caracteres; achado sobre allow/deny do Edit era falso positivo — verificado com `claude -p`); simplify (helpers de processo, estilos compartilhados, `usePersistedState`); docs (CHANGELOG, SECURITY).

Testes: Rust 50 (+6 ignorados reais), clippy -D warnings, tsc/eslint/prettier, Vitest 4, Playwright 5 (inclui fluxo real de equipe com `INA_E2E_WORKSPACE=ws-todo`).

## Pendências conhecidas
- Orquestrador: deny list é guarda, não sandbox (aliases/scripts ainda contornam; SECURITY.md).
- E2E do fluxo real falhou 1x na Fase 2; após a restauração de rascunho (Fase 4) não reproduziu em 4 execuções completas. Considerado mitigado; reabrir se voltar.

## Bloqueios
- RAM 7,3 GB: Claude Code mata `tauri dev` em background; fechar apps pesados ao testar.

## Decisões
- `portable-pty` fixado em `=0.9.0`: `kill` no Windows tem resultado invertido (contornado em `PtyManager::kill`).
- ConPTY envia `ESC[6n` no início e espera resposta (xterm.js responde; testes simulam).
- Arquivos `.cmd`/`.bat` nunca são executados (injeção via cmd.exe).
- Adicionar workspace por campo de caminho; seletor de pasta exigiria `tauri-plugin-dialog` (dependência nova, pedir aprovação).

## Como retomar
`RETOMAR INA`. Dev: `npm run tauri dev` (cargo em `~/.cargo/bin`). Verificação de UI: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
