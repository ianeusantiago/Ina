# INA

[Português](README.pt-BR.md)

Desktop app (Windows 11 first) that orchestrates a team of AI coding agents: one orchestrator plus N agents, each running the real Claude Code CLI in its own terminal and its own git worktree.

> Status: Phase 2 (reliable orchestration). Windows only so far. Expect rough edges.

## What it does

1. **Describe the team in natural language.** INA asks Claude Code (`claude -p` with a JSON schema) for a structured plan: agents, roles, scope, task, acceptance criteria, dependencies, parallel or sequential.
2. **Review and approve.** Nothing runs before approval. On approval INA creates one git worktree and branch per agent (`.ina/worktrees/<run>-<agent>`, branch `ina/<run>/<agent>`).
3. **N+1 real terminals.** Orchestrator in the workspace root (no file edits, no git writes), each agent in its worktree. Terminals are real PTYs (ConPTY), not simulations.
4. **Dependency-aware scheduling.** An agent starts only when its prerequisites are done (sequential mode: everyone before it). Before it starts, INA merges the prerequisites' branches into its worktree and lists them in its `task.json`. Dependents of a failed agent become blocked and are released when it recovers. Optional auto-start; 60-minute limit per agent.
5. **Shared task registry.** Each agent gets `.ina/task.json` and reports in `.ina/result.json`. INA tracks status in SQLite (`pending → running → verifying → done`, or `failed` / `blocked` / `cancelled`) and appends every change to `.ina/events.jsonl`.
6. **Result verification.** A reported "done" is only a claim: INA runs the agent's verification commands (approved with the plan; `cargo`, `node`, `python` or `pytest` only) on a clean checkout of what it committed and compares the files it declared with what its branch changed. Without checks a task is marked "unverified".
7. **Structured messages.** Agents write questions, progress, blockers and handoffs to `.ina/outbox/`; you answer from the Messages panel. The orchestrator proposes instructions the same way, and you approve them before they are delivered. Delivered messages land in the agent's `.ina/inbox/`; on your request INA types one fixed notice line into its terminal.
8. **Controlled integration.** A dry run (`git merge-tree`) shows clean merges, predicted conflicts and files touched by more than one agent before you confirm. Only done (verified or explicitly unverified) agents are merged, with `--no-ff` in dependency order; abort on conflict; cleanup of worktrees and merged branches. Uncommitted or unmerged work is never deleted.
9. **Recovery.** Teams persist; after a restart, tasks whose session was lost are closed with whatever result they left. Re-running a task tells the agent how the previous attempt ended (`previousAttempt`).

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
cd ..
# E2E: start the app with CDP, then run Playwright against its WebView2
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run tauri dev
npm run e2e                   # UI, workspaces, real shell
INA_E2E_WORKSPACE=<workspace name> npm run e2e   # + full team flow with real agents (costs tokens)
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
  tasks.rs        .ina/ protocol, task registry, dependency blocking
  messages.rs     outbox/inbox messages
  verify.rs       verification checks, declared vs changed files
  store.rs        SQLite (workspaces, teams, tasks)
  trust.rs        read-only Claude Code folder-trust check
  setup.rs        first-run check of Claude Code
docs/STATE.md     current state and next steps
docs/PHASE2.md    Phase 2 plan and decisions
e2e/              Playwright tests (attach to the running app over CDP)
```

## License

[MIT](LICENSE)
