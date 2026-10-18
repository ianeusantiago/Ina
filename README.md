# INA

[Português](README.pt-BR.md)

Desktop app (Windows 11 first) that orchestrates a team of AI coding agents: one orchestrator plus N agents, each running the real Claude Code CLI in its own terminal and its own git worktree.

> Status: Phase 1 (MVP). Windows only so far. Expect rough edges.

## What it does

1. **Describe the team in natural language.** INA asks Claude Code (`claude -p` with a JSON schema) for a structured plan: agents, roles, scope, task, acceptance criteria, dependencies, parallel or sequential.
2. **Review and approve.** Nothing runs before approval. On approval INA creates one git worktree and branch per agent (`.ina/worktrees/<run>-<agent>`, branch `ina/<run>/<agent>`).
3. **N+1 real terminals.** Orchestrator in the workspace root (read-only for git and files), each agent in its worktree. Terminals are real PTYs (ConPTY), not simulations.
4. **Shared task registry.** Each agent gets `.ina/task.json` and reports in `.ina/result.json` (`done`, `failed`, `blocked`). INA tracks status in SQLite and appends every change to `.ina/events.jsonl`. The task panel shows status, summary and acceptance criteria.
5. **Controlled integration.** Branch status, merge with `--no-ff` in dependency order, abort on conflict (nothing changes), cleanup of worktrees and merged branches. Uncommitted or unmerged work is never deleted; removing untracked files needs a second confirmation.
6. Teams persist across restarts and can be resumed.

UI in Portuguese (pt-BR) and English.

## Requirements

- Windows 11
- [Claude Code](https://docs.claude.com/claude-code) CLI on `PATH`, onboarded and logged in (INA checks this on start)
- Git
- Rust (stable, MSVC) and Visual Studio Build Tools 2022
- Node.js (tested with 24)
- Each workspace must be a git repository with at least one commit, and its folder trusted in Claude Code (run `claude` there once and accept)

## Run

```sh
npm install
npm run tauri dev
```

## Tests

```sh
npm test                      # Vitest
npm run lint
npx tsc --noEmit
cd src-tauri
cargo test                    # unit + integration (uses real git)
cargo test -- --ignored       # also runs real Claude Code (costs tokens)
cargo clippy --all-targets
```

## Security model

- The backend is the only authority over processes: programs come from an allowlist (`claude`, `powershell`, `git`), args are validated and passed as a vector (no shell string interpolation), and every `cwd` must be inside the workspace.
- The frontend never chooses a program or a prompt for an agent; prompts are built from the stored, approved plan.
- Session variables of the parent Claude Code (ids, tokens, sockets) are removed from child processes.
- INA only reads the Claude Code config (`~/.claude.json`) and never writes it.

See [SECURITY.md](SECURITY.md).

## Layout

```
src/              React + TypeScript UI (xterm.js, i18next)
src-tauri/src/
  pty.rs          real PTY sessions (portable-pty / ConPTY)
  policy.rs       program allowlist, args/cwd validation, env stripping
  plan.rs         plan schema, validation, planner, prompts
  worktree.rs     git worktrees, status, merge, cleanup
  tasks.rs        .ina/ protocol and task registry
  store.rs        SQLite (workspaces, teams, tasks)
  trust.rs        read-only Claude Code folder-trust check
  setup.rs        first-run check of Claude Code
docs/STATE.md     current state and next steps
```

## License

[MIT](LICENSE)
