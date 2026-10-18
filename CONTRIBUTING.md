# Contributing / Como contribuir

## Setup

See [README.md](README.md#requirements) (English) or [README.pt-BR.md](README.pt-BR.md#requisitos) (português).

## Rules / Regras

- TypeScript strict, ESLint, Prettier; Rust with `cargo fmt` and `cargo clippy --all-targets` clean.
- Every user-facing string goes through i18n, in both `src/locales/pt-BR.json` and `src/locales/en.json` (a test checks the keys match).
- No fake features: terminals are real PTYs, statuses come from real processes and files.
- Do not swallow errors: return them or report them.
- Security rules in [SECURITY.md](SECURITY.md) are non-negotiable: no shell from the frontend, no string-interpolated commands, validate program/args/cwd, keep secrets out of logs and UI.
- New dependencies need a clear purpose in the PR.

## Before opening a PR / Antes de abrir um PR

```sh
npx tsc --noEmit && npm run lint && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets && cargo test
```

Describe what you verified and how (tests, manual check in the app). Only document what was verified.

Descreva o que você verificou e como (testes, checagem manual no app). Só documente o que foi verificado.
