# INA

[English](README.md)

App desktop (Windows 11 primeiro) que orquestra uma equipe de agentes de IA para código: um orquestrador mais N agentes, cada um rodando o Claude Code CLI de verdade no próprio terminal e no próprio git worktree.

> Status: Fase 1 (MVP). Por enquanto só Windows. Ainda há arestas.

## O que faz

1. **Descreva a equipe em linguagem natural.** O INA pede ao Claude Code (`claude -p` com schema JSON) um plano estruturado: agentes, papéis, escopo, tarefa, critérios de aceite, dependências, paralelo ou sequencial.
2. **Revise e aprove.** Nada roda antes da aprovação. Na aprovação o INA cria um git worktree e uma branch por agente (`.ina/worktrees/<run>-<agente>`, branch `ina/<run>/<agente>`).
3. **N+1 terminais reais.** Orquestrador na raiz do workspace (sem escrita de arquivos nem git), cada agente no seu worktree. Terminais são PTYs reais (ConPTY), não simulações.
4. **Registro de tarefas compartilhado.** Cada agente recebe `.ina/task.json` e reporta em `.ina/result.json` (`done`, `failed`, `blocked`). O INA guarda o status no SQLite e registra cada mudança em `.ina/events.jsonl`. O painel de tarefas mostra status, resumo e critérios de aceite.
5. **Integração controlada.** Status das branches, merge `--no-ff` na ordem das dependências, aborta em conflito (nada muda), limpeza de worktrees e branches integradas. Trabalho não commitado ou não integrado nunca é apagado; remover arquivos não rastreados pede uma segunda confirmação.
6. Equipes persistem entre reinícios e podem ser retomadas.

Interface em português (pt-BR) e inglês.

## Requisitos

- Windows 11
- [Claude Code](https://docs.claude.com/claude-code) CLI no `PATH`, configurado e logado (o INA verifica ao abrir)
- Git
- Rust (stable, MSVC) e Visual Studio Build Tools 2022
- Node.js (testado com a 24)
- Cada workspace precisa ser um repositório git com pelo menos um commit, e a pasta precisa ser confiável no Claude Code (rode `claude` nela uma vez e aceite)

## Rodar

```sh
npm install
npm run tauri dev
```

## Testes

```sh
npm test                      # Vitest
npm run lint
npx tsc --noEmit
cd src-tauri
cargo test                    # unitários + integração (usa git real)
cargo test -- --ignored       # também roda o Claude Code real (gasta tokens)
cargo clippy --all-targets
```

## Modelo de segurança

- O backend é a única autoridade sobre processos: programas vêm de uma allowlist (`claude`, `powershell`, `git`), argumentos são validados e passados como vetor (sem interpolar string em shell) e todo `cwd` precisa estar dentro do workspace.
- O frontend nunca escolhe programa nem prompt de agente; os prompts são montados a partir do plano aprovado e salvo.
- Variáveis de sessão do Claude Code pai (ids, tokens, sockets) são removidas dos processos filhos.
- O INA só lê a configuração do Claude Code (`~/.claude.json`), nunca escreve.

Veja [SECURITY.md](SECURITY.md).

## Estrutura

```
src/              UI React + TypeScript (xterm.js, i18next)
src-tauri/src/
  pty.rs          sessões PTY reais (portable-pty / ConPTY)
  policy.rs       allowlist de programas, validação de args/cwd, limpeza de env
  plan.rs         schema do plano, validação, planejador, prompts
  worktree.rs     git worktrees, status, merge, limpeza
  tasks.rs        protocolo .ina/ e registro de tarefas
  store.rs        SQLite (workspaces, equipes, tarefas)
  trust.rs        checagem somente-leitura de confiança de pasta do Claude Code
  setup.rs        checagem de primeira execução do Claude Code
docs/STATE.md     estado atual e próximos passos
```

## Licença

[MIT](LICENSE)
