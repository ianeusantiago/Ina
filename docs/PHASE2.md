# Fase 2 — Orquestração confiável (aprovada em 2026-10-18)

1. Recuperação no startup: tarefas `running` órfãs → `result.json` ou "sessão perdida"; `team_tasks` só leitura; resumo limpo ao rodar de novo.
2. Agendador: auto-start de dependentes (opção liga/desliga); dependentes de falha/cancelada → `blocked`; merge das branches dos pré-requisitos no worktree do dependente antes de iniciar; `task.json` com resumo/branch dos pré-requisitos; timeout por agente.
3. Mensagens: `.ina/outbox/*.json` (progress, question, blocked, handoff) validadas → tabela `messages` + `events.jsonl`; respostas/instruções em `.ina/inbox/`; aviso no PTY com linha fixa.
4. Verificação: comandos `verify` por agente no plano, aprovados na tela do plano, só programas permitidos (npm, cargo, pytest, node, python), no worktree; `done` → `verified`/falha; arquivos declarados vs `git diff`.
5. Pré-integração: `git merge-tree` (sem tocar em nada) + arquivos sobrepostos entre agentes, mostrado antes de confirmar.
6. Reexecutar após falha com o motivo anterior no `task.json`.
7. E2E Playwright (via CDP), code-review, security-review, simplify, docs.

Decisões: (A) auto-start com opção de desligar; (B) só comandos de verificação aprovados e na allowlist; (C) INA digita no PTY só uma linha fixa de aviso; (D) instruções do orquestrador são propostas e o usuário aprova.
