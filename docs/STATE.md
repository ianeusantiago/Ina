# INA — Estado

**Fase atual:** 7 — Interface tipo chat, branch `ianeusantiago/interface` (worktree `.claude/worktrees/ina-view`). Pausado em 2026-10-20. No `main`: Aero (PR #9), Codex por agente (PR #10), rodapé de uso (PR #11).

## O que fazer ao retomar (usuário)
1. Abrir o INA, criar uma equipe NOVA (equipes antigas não têm `--session-id`).
2. Na Interface: mandar "hello" a um agente e conferir: mensagem aparece na hora, "Pensando…/Editando X…" enquanto trabalha, resposta em balão.
3. Trocar Interface ↔ Terminais: deve manter agente selecionado e rascunho.
4. Dizer o que ainda está errado na UX; se ok, pedir commit + merge.
5. Opcional: testar um agente Codex (nunca rodado de verdade).

## Fase 7 — em andamento (não commitado em main; WIP na branch)
- Interface (padrão) ↔ Terminais (botão ao lado da barra de integração); ambos ficam montados.
- `activity.rs`: lê o log do próprio CLI (Claude `~/.claude/projects/<cwd>/<session>.jsonl`; Codex `~/.codex/sessions/**/rollout-*.jsonl` por cwd). Sessões Claude do INA recebem `--session-id` fixo (`PlanStore::set_log/log`) — corrige o bug de ler o log errado.
- `agent_send`: digita uma linha no terminal vivo do agente (sem caracteres de controle, Enter separado).
- Chat: cartão "Tarefa" (texto do plano no lugar do prompt do INA), balões quadrados (`aero-card`), mensagem otimista, indicador "Pensando…/Editando X…", aviso após 45 s parado.
- Removido: resumo por Haiku (`digest`) e "detalhes técnicos" (usuário quer a conversa real).
- Verificado: Rust 66, clippy, tsc/eslint, Vitest 4. NÃO verificado: E2E (app caiu por RAM; última rodada inválida — CDP desligado). E2E novo testa enviar "pong" e receber resposta.
- Pendente: teste de redimensionar falhou 1x na rodada completa (passou sozinho); investigar se repetir.

## Fase 6 — design estilo macOS
- Tema claro padrão + Escuro/Sistema (persistido, aplicado antes do primeiro paint); tokens semânticos em `index.css`; barra de título própria com semáforo (janela sem decorações, permissões `core:window:allow-*`); sidebar translúcida; terminal segue o tema.
- Verificado: tsc/eslint/prettier, Vitest 2, Playwright 5/5 (fluxo real), capturas claro/escuro conferidas.
- Não verificado: clique real em fechar/minimizar/maximizar (E2E não fecha a janela).

## Fase 5 — pendências finais (PR #6 merged, 54fa0b8)
- Orquestrador: deny list cobre também a ferramenta PowerShell e mais formas de git (`-C`, `-c`, restore, stash, worktree, update-ref, branch -f/-D). Verificado com `claude -p` (PowerShell deny e `git -C` bloqueados; controle sem deny commitou). Rust 53, clippy, fmt.
- E2E Playwright 5/5 em 3 execuções seguidas (fluxo real `ws-todo`): falha intermitente não reproduziu.

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
