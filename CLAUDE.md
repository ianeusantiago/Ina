# INA — Regras do Projeto

App desktop (Windows 11 primeiro) para orquestrar agentes de IA (estilo Orca): 1 orquestrador + N agentes, cada um com terminal/processo real (Claude Code CLI).

## Estilo
- Respostas curtas, objetivas, em português. Sem preâmbulo nem recap.
- Responder perguntas diretamente. Parar quando o usuário pedir.
- Economizar tokens: grep antes de ler, ler só trechos, não reler arquivo editado, filtrar logs (head/tail).

## Fluxo
- Fase 0 (plano) precisa de aprovação antes de qualquer arquivo/instalação.
- Depois: incrementos pequenos; pedir aprovação só em checkpoints relevantes (novo módulo, dependência nova, ação destrutiva).
- Nunca dizer que funciona/passa sem rodar e ver a saída.
- Commit/push só quando o usuário pedir (exceto no comando PAUSAR, abaixo).

## Stack (default)
Tauri 2 + React + TypeScript (strict) + Vite + Tailwind + xterm.js; Rust no backend com PTY real (portable-pty). Sem terminais simulados.

## Orquestração (obrigatório)
- Orquestrador: interpreta tarefa, quebra em subtarefas, define papéis, dependências, paralelo vs sequencial, monitora, valida, integra, reporta honestamente.
- Agentes: id, nome, papel, escopo, terminal próprio, critérios de aceite, status, histórico, falha/cancelamento/recuperação.
- Equipe definida em linguagem natural → plano estruturado → aprovação → abre N+1 sessões reais.
- Comunicação via mensagens estruturadas + registro de tarefas compartilhado (não só ler terminal alheio).
- Isolamento por git worktree por agente; integração controlada; fallback sequencial.

## Segurança
- Backend é autoridade sobre processos. Sem shell arbitrário vindo do frontend.
- Sem interpolação de string em comandos; validar executável, args, cwd e limites do workspace.
- Segredos fora de logs/código/estado do frontend.
- Aprovação explícita para ações destrutivas/privilegiadas.

## Ferramentas liberadas (usar quando pertinente, sem pedir)
- Superpowers: brainstorming, writing-plans, executing-plans, test-driven-development, systematic-debugging, using-git-worktrees, dispatching-parallel-agents, subagent-driven-development, requesting-code-review, verification-before-completion.
- Playwright: testes E2E da UI (Vite dev server; app Tauri via CDP do WebView2).
- frontend-design / ui-ux-pro-max / ui-styling: UI estilo macOS, minimalista, clara por padrão (modo escuro e sistema opcionais).
- code-review, security-review, simplify: antes de fechar cada fase.
- Instalar plugin/MCP/dependência NOVA: mostrar propósito + comando e pedir aprovação.

## Idiomas e documentação
- UI bilíngue: pt-BR e inglês (i18n, sem strings fixas).
- Um agente/papel dono da documentação: README (EN + pt-BR), docs/, CONTRIBUTING, CHANGELOG, LICENSE, SECURITY, templates. Atualizar a cada fase; só documentar o que foi verificado.

## Qualidade
TS strict, ESLint, Prettier, testes unitários + integração (ciclo de processo, atribuição de tarefa, autorização, cancelamento). Sem funcionalidade falsa. Sem erros engolidos.

## Comando PAUSAR
Quando o usuário disser **"PAUSAR INA"**:
1. Parar o que estiver fazendo (encerrar processos em background).
2. Atualizar `docs/STATE.md`: fase atual, feito, em andamento, próximos passos, decisões, bloqueios.
3. Salvar resumo na memória do Claude (memória `ina-progress`).
4. `git add -A && git commit -m "wip: pausa"`; se houver remote, `git push` (nuvem).
5. Responder em 1 linha: o que foi salvo + como retomar.

## Comando RETOMAR
Quando o usuário disser **"RETOMAR INA"**: ler `docs/STATE.md` + memória `ina-progress`, `git status`, e dizer em 3 linhas onde parou e o próximo passo.
