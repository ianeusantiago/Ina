# Como contribuir

Obrigado pelo interesse no INA! Para preparar o ambiente, veja [Desenvolvimento](README.md#desenvolvimento) no README.

## Regras do projeto

- **TypeScript strict**, ESLint e Prettier; no Rust, `cargo fmt` e `cargo clippy --all-targets -- -D warnings` limpos.
- **Todo texto da interface passa pelo i18n**, em `src/locales/pt-BR.json` e `src/locales/en.json` (um teste confere se as chaves batem).
- **Nada de funcionalidade de mentira:** terminais são PTYs reais e os status vêm de processos e arquivos reais.
- **Não engula erros:** devolva ou registre.
- As regras de [SECURITY.md](SECURITY.md) não são negociáveis: nada de shell vindo da interface, nada de comando montado como texto, validar programa, argumentos e pasta, e nenhum segredo em log ou na interface.
- **Dependência nova** precisa de um motivo claro no PR.
- Na interface, prefira **linguagem simples**: detalhes técnicos ficam no terminal.

## Antes de abrir um PR

```sh
npx tsc --noEmit && npm run lint && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

O [CI](.github/workflows/ci.yml) roda essas mesmas checagens em todo PR.

Na descrição, conte **o que você verificou e como** (testes, teste manual no app). Documente só o que foi verificado.

## Commits

Mensagens curtas, em português, dizendo o que mudou e por quê (ex.: `Home: conversa persiste ao trocar de tela`).
