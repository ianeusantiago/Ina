<p align="center">
  <img src="src/assets/ina-icon.svg" alt="Ícone do INA" width="120" />
</p>

<h1 align="center">INA</h1>

<p align="center">
  <strong>Uma equipe de agentes de IA trabalhando no seu projeto — com você no comando.</strong>
</p>

<p align="center">
  <a href="https://github.com/ianeusantiago/Ina/releases/latest"><img alt="Última versão" src="https://img.shields.io/github/v/release/ianeusantiago/Ina?label=vers%C3%A3o&color=1a8fe0" /></a>
  <a href="https://github.com/ianeusantiago/Ina/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/ianeusantiago/Ina/actions/workflows/ci.yml/badge.svg" /></a>
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-0a78d8" />
  <a href="LICENSE"><img alt="Licença MIT" src="https://img.shields.io/badge/licen%C3%A7a-MIT-3aa83a" /></a>
</p>

<p align="center">
  <a href="#baixar">Baixar</a> ·
  <a href="#como-funciona">Como funciona</a> ·
  <a href="#recursos">Recursos</a> ·
  <a href="#segurança">Segurança</a> ·
  <a href="#desenvolvimento">Desenvolvimento</a> ·
  <a href="README.en.md">English</a>
</p>

---

O INA é um app desktop para Windows que monta e coordena uma **equipe de agentes de IA**: um **orquestrador** e vários **agentes**, cada um rodando o [Claude Code](https://docs.claude.com/claude-code) de verdade, no próprio terminal e na própria cópia do projeto (git worktree). Você descreve o que quer em português, aprova o plano e acompanha tudo numa interface de conversa — sem precisar abrir o terminal, a menos que queira.

## Baixar

1. Baixe o **`INA_x.y.z_x64-setup.exe`** em **[Releases](https://github.com/ianeusantiago/Ina/releases/latest)** e instale.
2. Se o Windows mostrar “O Windows protegeu o computador”, clique em **Mais informações → Executar assim mesmo** (o instalador ainda não tem certificado de assinatura de código).
3. Pronto. Quando sair uma versão nova, o próprio INA avisa: **Nova versão disponível → Atualizar e reiniciar**.

**Você precisa ter:**

- Windows 11
- [Claude Code](https://docs.claude.com/claude-code) instalado e com login feito (o INA confere ao abrir)
- [Git](https://git-scm.com/)

> Os agentes usam a **sua** conta do Claude: o uso sai do seu plano. O INA mostra quanto cada equipe gastou.

## Como funciona

1. **Peça na tela Início.** Ex.: _“Crie um workspace chamado Loja e faça uma página de produtos com 2 agentes.”_ O INA cria a pasta do projeto, monta a equipe e mostra o plano. Também dá para começar por um **modelo** (Site, API + testes, Corrigir bug, Melhorar código).
2. **Aprove.** Nada roda antes da sua aprovação. Cada agente ganha sua própria cópia do projeto, então ninguém atropela ninguém.
3. **Acompanhe na Interface.** Converse com o orquestrador e com cada agente como num chat. Quando um agente pede permissão para rodar algo, aparece um cartão com **Permitir / Permitir sempre / Negar** (e “Ver detalhes”).
4. **Receba o resultado.** Quando todos terminam, o INA mostra um resumo simples (“o que foi feito” e “como testar”), oferece uma **revisão** por IA e junta o trabalho no seu projeto com um clique — depois de mostrar uma prévia sem conflitos.

## Recursos

**Conversa e acompanhamento**

- Tela **Início** que conversa com o Claude, cria workspaces e monta equipes; a conversa fica salva.
- **Interface tipo chat** para cada membro da equipe, em linguagem simples; detalhes técnicos ficam no terminal.
- **Terminais reais** (ConPTY) a um clique, para quem quiser ver tudo.
- Barra lateral em **árvore**: workspaces → equipes → orquestrador e agentes, com status ao vivo; filtro e arquivamento de equipes antigas.
- **Painel de progresso** (montada → trabalhando → verificado → juntado) e **uso de tokens** por equipe.
- **Notificações do Windows** quando um agente termina, falha, trava ou pede permissão.
- **Continuar conversa** depois de fechar o app (retoma a sessão do Claude).

**Qualidade e controle**

- Plano estruturado: papéis, escopo, tarefa, critérios de aceite, dependências, paralelo ou sequencial.
- Agentes começam só quando suas dependências terminam; limite de tempo por agente.
- **Verificação real:** “concluído” é só uma alegação — o INA roda os testes aprovados no plano num checkout limpo.
- **Revisor:** um Claude sem ferramentas lê as mudanças de cada agente e diz se é seguro juntar.
- **Integração controlada:** simulação de merge antes, merge na ordem das dependências, nada é apagado sem estar integrado.
- **Prévia do projeto** dentro do app (página `index.html` ou servidor `localhost`), isolada em sandbox.

**Visual**

- Estilo **Frutiger Aero**: vidro, céu, bolhas e brilho — tema claro (padrão), escuro ou do sistema.
- Interface em **português** e **inglês**.

## Segurança

- O backend é a única autoridade sobre processos: só `claude`, `codex`, `powershell` e `git` (mais `cargo`, `node`, `python` e `pytest` para os testes aprovados no plano), com argumentos validados e passados como lista (nada de comandos montados como texto) e pastas sempre dentro do workspace.
- A interface nunca escolhe programa nem prompt: os prompts saem do plano que você aprovou.
- O orquestrador não edita arquivos nem faz commit/merge/push (bloqueado no Bash e no PowerShell); só você integra, pelo INA.
- Planejador, resumo e revisor rodam o Claude **sem ferramentas** — só leem e respondem.
- A prévia roda em iframe com sandbox, sem acesso ao app; atualizações só são aceitas se **assinadas** com a chave do projeto.

Detalhes e limites conhecidos em [SECURITY.md](SECURITY.md).

## Desenvolvimento

**Requisitos extras:** Rust (stable, MSVC), Visual Studio Build Tools 2022 e Node.js 22+.

```sh
npm install
npm run tauri dev
```

**Testes**

```sh
npm test                 # Vitest
npm run lint             # ESLint
npx tsc --noEmit         # TypeScript
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test               # unitários + integração (git real)
cargo test -- --ignored  # também chama o Claude Code real (gasta tokens)
```

**E2E** (Playwright conectado ao WebView2 do app):

```sh
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run tauri dev
npm run e2e
INA_E2E_WORKSPACE=<workspace> npm run e2e   # + fluxo com agentes reais (gasta tokens)
```

**CI e versões**

- Todo PR roda o [CI](.github/workflows/ci.yml) no Windows: build, lint, formato, Vitest, clippy e testes Rust.
- Para publicar, crie uma tag (`git tag v0.2.0 && git push origin v0.2.0`): o [workflow de release](.github/workflows/release.yml) gera o instalador, assina a atualização e publica em Releases. Lembre de subir a versão em `package.json`, `src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json` antes.

<details>
<summary><strong>Estrutura do código</strong></summary>

```
src/                    interface (React + TypeScript, xterm.js, i18next)
  components/           Home, Interface, barra lateral, terminais, prévia, banners
  lib/                  chamadas ao backend, tema, notificações, resumo
  locales/              textos em pt-BR e inglês
src-tauri/src/          backend (Rust, Tauri 2)
  pty.rs                terminais reais (ConPTY)
  policy.rs             allowlist de programas, validação de argumentos e pastas
  plan.rs               plano da equipe, planejador e prompts
  worktree.rs           git worktrees, merge, diff e limpeza
  tasks.rs / verify.rs  registro de tarefas e verificação dos resultados
  messages.rs           mensagens entre você, orquestrador e agentes
  activity.rs           leitura dos logs das sessões (Interface)
  home.rs               tela Início: entende o pedido e cria o workspace
  report.rs / review.rs resumo final e revisor
  teamusage.rs          tokens usados por equipe
  store.rs              banco SQLite
  trust.rs / setup.rs   confiança de pasta e checagem do Claude Code
docs/STATE.md           estado atual do projeto
e2e/                    testes Playwright
```

</details>

## Contribuir

Veja [CONTRIBUTING.md](CONTRIBUTING.md). Ideias e bugs: [abra uma issue](https://github.com/ianeusantiago/Ina/issues/new/choose).

## Licença

[MIT](LICENSE) © 2026 Ian Santiago
