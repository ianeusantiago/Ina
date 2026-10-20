# Histórico de mudanças

Formato: [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/). Versões seguem [SemVer](https://semver.org/lang/pt-BR/).

## [Não lançado]

## [0.1.1] — 2026-10-20

Revisão de código e correções; primeira versão entregue pela atualização automática.

### Alterado

- README, CONTRIBUTING, SECURITY, este histórico e os modelos de issue e PR em português e atualizados.
- Revisão de código: chamadas ao Claude sem ferramentas num módulo único (`claude_json`); `PATH`, pasta do usuário e variáveis removidas dos processos filhos centralizadas no `policy`; `pty_spawn_agent` dividido em funções menores; `TeamPanel`, `Sidebar` e `ReadyBanner` divididos em componentes menores; controle segmentado único.

### Corrigido

- Enviar uma mensagem pela Interface travava a tela por 80 ms (pausa na thread da interface).
- A busca por conversas para continuar rodava na thread da interface.
- Duas aprovações no mesmo segundo podiam gerar o mesmo nome de branch.
- O tick guardava para sempre dados de equipes já fechadas.
- Um arrasto do divisor interrompido pelo sistema deixava listeners pendurados.

### Acessibilidade

- Seletores de tema e de vista são grupos de opções navegáveis pelas setas, Home e End.

## [0.1.0] — 2026-10-19

Primeira versão pública: instalador do Windows com atualização automática.

### Adicionado

- **Tela Início:** conversa com o Claude que cria o workspace (em `INA Projects`), monta a equipe e oferece “Aprovar e começar”; a conversa fica salva; modelos de equipe (Site, API + testes, Corrigir bug, Melhorar código) com ícones Aqua.
- **Interface tipo chat** para orquestrador e agentes, lida dos logs das sessões; alterna com os terminais reais.
- **Permissões pela Interface:** cartão Permitir / Permitir sempre / Negar com “Ver detalhes”.
- **Barra lateral em árvore** (workspace → equipes → membros com status), filtro e arquivamento de equipes.
- **Fim guiado:** quando todos terminam, prévia da integração, juntar no projeto e limpar; **resumo final** em linguagem simples (“o que foi feito” e “como testar”) no banner e na Início; **revisor** por IA antes de juntar.
- **Painel de progresso** com etapas e porcentagem; **uso de tokens** por equipe.
- **Notificações do Windows** (terminou, falhou, travou, pede permissão).
- **Continuar conversa** depois de fechar o app (`--resume`).
- **Prévia do projeto** dentro do app (`index.html` ou `localhost`) em sandbox.
- **Seletor de pasta** nativo.
- Agentes podem rodar no **Codex** (escolha por agente na revisão do plano); rodapé com o uso do plano do Claude Code e do Codex.
- **Instalador NSIS** e **atualização automática assinada**; workflow de release por tag.
- **CI no GitHub** (build, lint, formato, Vitest, clippy e testes Rust) em todo PR.

### Alterado

- Visual **Frutiger Aero**: arte de fundo dia/noite, vidro, botões Aqua e ícone redesenhado; tema claro por padrão, escuro e sistema.
- Respostas dos agentes, resumo do plano e status em **linguagem simples**; detalhes técnicos ficam no terminal.

### Corrigido

- O orquestrador não abria (“too many arguments”) depois que a lista de bloqueios cresceu; agora há teste e o E2E falha se alguma sessão não abrir.
- A conversa da Início não some mais ao trocar de tela.

### Segurança

- Lista de bloqueios do orquestrador cobre também a ferramenta **PowerShell** e mais formas de git (`-C`, `-c`, `restore`, `stash`, `worktree`, `update-ref`, `branch -f/-D`).
- Planejador, Início, resumo e revisor rodam o Claude **sem ferramentas**.
- Prévia com acesso de leitura liberado só para a pasta da página e iframe sem same-origin; atualizações só com assinatura válida.

## Fases anteriores (antes da 0.1.0)

### Fase 4 — pendências

- Tarefas sem sessão finalizadas pelo tick; evento de mudança leva as tarefas; pré-requisitos vindos do backend; plano não aprovado volta à revisão depois de recarregar.

### Fase 3 — arquitetura

- Tick no backend (a cada 5 s) avança as equipes mesmo sem painel aberto; painéis atualizam por evento; timeouts pelo tick; travas de merge por worktree; cache de equipes aprovadas.
- Integração sempre exige agentes concluídos.

### Fase 2 — orquestração confiável

- Recuperação ao iniciar; agendador por dependências com início automático e limite de 60 minutos; mensagens estruturadas (outbox/inbox com aprovação); verificação real dos resultados; prévia de integração com `git merge-tree`; `previousAttempt`; E2E com Playwright.
- Orquestrador só grava propostas no outbox; verificações em checkout limpo sem variáveis `ANTHROPIC_*`.

### Fase 1 — MVP

- App Tauri 2 + React + TypeScript com terminais PTY reais (ConPTY) e xterm.js.
- Política de execução (allowlist, validação de argumentos e pastas, limpeza de variáveis `CLAUDE*`); workspaces em SQLite.
- Plano da equipe a partir de linguagem natural, revisão e aprovação; N+1 sessões reais do Claude Code; um git worktree e branch por agente; integração controlada e limpeza.
- Protocolo `.ina/` (`task.json`, `result.json`, `events.jsonl`) e registro de tarefas; interface em pt-BR e inglês.
