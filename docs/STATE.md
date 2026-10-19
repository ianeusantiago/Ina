# INA — Estado

**Fase atual:** 3 (arquitetura) concluída em 2026-10-18, branch `ianeusantiago/fase3-arquitetura` (PR #4), aguardando revisão/merge. Fase 2 integrada ao `main` (PR #3).

## Fase 4 — pendências (pausada; branch `worktree-fase4-pendencias`)
- Feito (verificado: Rust 51 + clippy, tsc/eslint/prettier, Vitest 2): tarefa `running` sem sessão finalizada pelo tick (2 ticks seguidos); evento `team-changed` leva as tarefas (sem sync repetido); `prerequisites` vem do backend (cópia TS removida); `Registry::task` lê 1 linha; travas por worktree sem uso são descartadas.
- Restauração de rascunho: plano não aprovado (< 30 min, mais novo que a última equipe aprovada) volta à revisão após reload (`plan_draft`). Rust 52.
- E2E Playwright 5/5 (inclui fluxo real `ws-todo`) após as mudanças; code-review (medium) sem achados.
- Próximo: PR.

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
- Orquestrador: deny list é guarda, não sandbox (SECURITY.md).
- E2E do fluxo real falhou 1x (plano gravado no banco, UI não recebeu resposta; não reproduziu em 3 execuções seguintes). Backend grava e retorna em sequência; hipótese: WebView recarregou (recompilação do `tauri dev`) durante o planejamento ou planner > 5 min. Mitigado na Fase 4 (restauração de rascunho); confirmar no E2E.

## Bloqueios
- RAM 7,3 GB: Claude Code mata `tauri dev` em background; fechar apps pesados ao testar.

## Decisões
- `portable-pty` fixado em `=0.9.0`: `kill` no Windows tem resultado invertido (contornado em `PtyManager::kill`).
- ConPTY envia `ESC[6n` no início e espera resposta (xterm.js responde; testes simulam).
- Arquivos `.cmd`/`.bat` nunca são executados (injeção via cmd.exe).
- Adicionar workspace por campo de caminho; seletor de pasta exigiria `tauri-plugin-dialog` (dependência nova, pedir aprovação).

## Como retomar
`RETOMAR INA`. Dev: `npm run tauri dev` (cargo em `~/.cargo/bin`). Verificação de UI: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
