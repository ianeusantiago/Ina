# Segurança

## Como reportar

Não abra issue pública para vulnerabilidades. Use o **reporte privado** do GitHub: aba **Security → Report a vulnerability** deste repositório.

## Modelo

O INA abre processos de verdade no seu computador, então o backend (Rust) é a única autoridade sobre eles.

**Processos e comandos**

- **Allowlist de programas:** só `claude`, `codex`, `powershell` e `git`, resolvidos no `PATH` pelo backend. A interface nunca escolhe programa.
- **Sem interpolação de shell:** argumentos vão como lista; quantidade e tamanho são limitados e bytes NUL são recusados.
- **Limite do workspace:** toda pasta de trabalho é canonizada e precisa estar dentro do workspace.
- **Só pastas confiáveis:** planejador e sessões não começam numa pasta que o Claude Code ainda não confia (senão configurações e hooks do projeto seriam carregados).
- **Prompts só de planos aprovados:** os prompts do orquestrador e dos agentes são montados no backend a partir do plano que você aprovou e o INA salvou; nunca vêm da interface.

**Orquestrador**

- Roda no modo de permissão padrão do Claude Code com `Edit`, `NotebookEdit` e os comandos git merge/rebase/reset/commit/push/cherry-pick/checkout/switch/restore/stash/worktree/update-ref, `branch -f/-D` e as opções globais `-C`/`-c` **bloqueados no Bash e no PowerShell**.
- Só pode gravar propostas em `.ina/outbox/`; gravações nos worktrees dos agentes, em `.git/` e em `.ina/events.jsonl` são negadas (regras conferidas com `claude -p`). Propostas só chegam a um agente depois que você aprova cada uma. A integração acontece só pelo INA, com sua confirmação.

**Claude sem ferramentas**

- Planejador, tela Início (`home_intent`), resumo final (`team_report`) e revisor (`team_review`) rodam `claude -p` com `--tools ""` e `--strict-mcp-config`: só leem o que o INA manda e respondem num JSON validado por schema.
- O revisor recebe o diff de cada agente limitado a ~20 mil caracteres (limite da linha de comando do Windows) e avisa quando corta.

**Verificação**

- Comandos de verificação vêm do plano que você aprova e se limitam a `cargo`, `node`, `python` e `pytest` (só `.exe`; atalhos `.cmd` como o npm são recusados).
- Rodam sem shell, com argumentos validados, num checkout descartável da branch do agente, com limite de 10 minutos e sem variáveis `ANTHROPIC_*`. Eles executam código escrito pelo agente: aprovar uma verificação é confiar nesse código.

**Interface e terminais**

- **Permissões pela Interface:** o INA lê o menu de permissão na tela do terminal e, **só quando você clica**, digita a tecla da opção escolhida. Nada é respondido sozinho.
- **Mensagens:** arquivos do outbox têm tamanho limitado e schema validado, com tipos e destinatários permitidos; inválidos vão para `rejected/`. O aviso no terminal de um agente é uma linha fixa, sem dígitos nem texto da mensagem, para não responder a um prompt de permissão.
- **Leitura de conversas:** só sessões abertas pelo INA (id conhecido); nenhum caminho vem da interface.

**Arquivos e pastas**

- **Workspaces criados pela tela Início** ficam só em `%USERPROFILE%\INA Projects\<nome>`; o nome é validado (sem `..`, barras, nomes reservados do Windows) e pastas já com arquivos são recusadas.
- **Seletor de pasta:** só a permissão de abrir pasta (`dialog:allow-open`); o caminho escolhido passa pela mesma validação de workspace.
- **Prévia do projeto:** o acesso de leitura é liberado em tempo de execução **só para a pasta da página** (`index.html`) do workspace aberto; o iframe usa sandbox **sem `allow-same-origin`**, então a página não alcança o INA. Endereços digitados precisam ser `localhost` ou `127.0.0.1`.
- **Sem perda de dados:** limpeza recusa worktrees com mudanças e branches não integradas. Apagar arquivos não rastreados exige uma segunda confirmação e só vale com a branch integrada. **Arquivar** uma equipe só a esconde; worktrees e branches ficam no disco.
- **Resultados dos agentes não são confiáveis:** `result.json` tem tamanho limitado e schema validado; arquivo inválido nunca conta como sucesso.

**Segredos e atualizações**

- Variáveis de sessão do Claude Code pai (`CLAUDECODE`, ids de sessão, tokens, sockets) são removidas dos processos filhos. O INA lê `~/.claude.json` só para indicadores (pasta confiável, onboarding, logado), nunca escreve e nunca devolve dados da conta à interface.
- **Atualizações assinadas:** o app só instala versões com assinatura válida (minisign). A chave pública está no `tauri.conf.json`; a privada fica fora do repositório (nos secrets do GitHub, para o workflow de release).
- **Content Security Policy:** scripts só do próprio app; IPC só com o Tauri; frames só do protocolo de arquivos do app e de `localhost`.

## Limites conhecidos

- Os agentes rodam o Claude Code com as permissões normais dentro do worktree deles; o INA não os isola além disso.
- A lista de bloqueios do orquestrador é uma proteção, não um sandbox: regras por prefixo como `Bash(git merge:*)` ainda podem ser contornadas por outras formas de comando (aliases, scripts, redirecionamento). O orquestrador roda na raiz do workspace; confira o que ele faz no terminal.
- A detecção de permissões lê a tela do terminal: uma mudança no visual do Claude Code pode fazer o cartão não aparecer (o terminal continua funcionando normalmente).
- O instalador não tem certificado de assinatura de código; o Windows mostra um aviso na primeira instalação.
