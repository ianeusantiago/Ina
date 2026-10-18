# INA

[English](README.en.md)

App desktop (Windows 11 primeiro) que orquestra uma equipe de agentes de IA para código: um orquestrador mais N agentes, cada um rodando o Claude Code CLI de verdade no próprio terminal e no próprio git worktree.

> Status: Fase 2 (orquestração confiável). Por enquanto só Windows. Ainda há arestas.

## O que faz

1. **Descreva a equipe em linguagem natural.** O INA pede ao Claude Code (`claude -p` com schema JSON) um plano estruturado: agentes, papéis, escopo, tarefa, critérios de aceite, dependências, paralelo ou sequencial.
2. **Revise e aprove.** Nada roda antes da aprovação. Na aprovação o INA cria um git worktree e uma branch por agente (`.ina/worktrees/<run>-<agente>`, branch `ina/<run>/<agente>`).
3. **N+1 terminais reais.** Orquestrador na raiz do workspace (sem editar arquivos nem escrever no git), cada agente no seu worktree. Terminais são PTYs reais (ConPTY), não simulações.
4. **Agendamento por dependências.** Um agente só começa quando seus pré-requisitos concluem (modo sequencial: todos antes dele). Antes de iniciar, o INA faz merge das branches dos pré-requisitos no worktree dele e as lista no `task.json`. Dependentes de um agente que falhou ficam bloqueados e são liberados quando ele se recupera. Início automático opcional; limite de 60 minutos por agente.
5. **Registro de tarefas compartilhado.** Cada agente recebe `.ina/task.json` e reporta em `.ina/result.json`. O INA guarda o status no SQLite (`pending → running → verifying → done`, ou `failed` / `blocked` / `cancelled`) e registra cada mudança em `.ina/events.jsonl`.
6. **Verificação de resultados.** "Concluído" informado pelo agente é só uma alegação: o INA roda os comandos de verificação do agente (aprovados com o plano; só `cargo`, `node`, `python` ou `pytest`) num checkout limpo do que ele commitou e compara os arquivos declarados com o que a branch mudou. Sem comandos, a tarefa fica "não verificada".
7. **Mensagens estruturadas.** Agentes gravam perguntas, progresso, bloqueios e entregas em `.ina/outbox/`; você responde pelo painel de Mensagens. O orquestrador propõe instruções do mesmo jeito, e você aprova antes da entrega. Mensagens entregues vão para `.ina/inbox/` do agente; quando você pede, o INA digita uma linha fixa de aviso no terminal dele.
8. **Integração controlada.** Uma simulação (`git merge-tree`) mostra merges limpos, conflitos previstos e arquivos mexidos por mais de um agente antes de você confirmar. Só agentes concluídos (verificados ou explicitamente não verificados) entram, com merge `--no-ff` na ordem das dependências; aborta em conflito; limpeza de worktrees e branches integradas. Trabalho não commitado ou não integrado nunca é apagado.
9. **Recuperação.** Equipes persistem; depois de reiniciar, tarefas cuja sessão se perdeu são fechadas com o resultado que deixaram. Ao rodar uma tarefa de novo, o agente recebe como terminou a tentativa anterior (`previousAttempt`).

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
cd ..
# E2E: abra o app com CDP e rode o Playwright contra o WebView2 dele
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run tauri dev
npm run e2e                   # UI, workspaces, shell real
INA_E2E_WORKSPACE=<nome do workspace> npm run e2e   # + fluxo completo com agentes reais (gasta tokens)
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
  tasks.rs        protocolo .ina/, registro de tarefas, bloqueio por dependência
  messages.rs     mensagens outbox/inbox
  verify.rs       comandos de verificação, arquivos declarados vs alterados
  store.rs        SQLite (workspaces, equipes, tarefas)
  trust.rs        checagem somente-leitura de confiança de pasta do Claude Code
  setup.rs        checagem de primeira execução do Claude Code
docs/STATE.md     estado atual e próximos passos
docs/PHASE2.md    plano e decisões da Fase 2
e2e/              testes Playwright (conectam ao app em execução via CDP)
```

## Licença

[MIT](LICENSE)
