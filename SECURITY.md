# Security / Segurança

## Reporting / Como reportar

Please do not open a public issue for vulnerabilities. Use GitHub's private vulnerability reporting ("Report a vulnerability" in the Security tab) on this repository.

Não abra issue pública para vulnerabilidades. Use o reporte privado do GitHub ("Report a vulnerability" na aba Security) deste repositório.

## Model / Modelo

INA starts real processes on your machine, so the backend (Rust) is the only authority over them:

- **Program allowlist:** only `claude`, `powershell` and `git`, resolved from `PATH` by the backend. The frontend never names a program.
- **Trusted folders only:** the planner and every session refuse to start in a folder Claude Code has not trusted (project settings and hooks would otherwise load).
- **No shell interpolation:** arguments are passed as a vector; their count and size are limited and NUL bytes are rejected.
- **Workspace boundary:** every working directory is canonicalized and must be inside the workspace.
- **Prompts from approved plans only:** agent and orchestrator prompts are built in the backend from the plan the user approved and INA stored, never sent by the frontend.
- **Orchestrator limits:** it starts in Claude Code's default permission mode (every write asks you) with `Edit`, `NotebookEdit` and git merge/rebase/reset/commit/push/cherry-pick/checkout/switch denied. Writes to `.ina/outbox/` (proposals) are pre-allowed and writes to agent worktrees, `.git/` and `.ina/events.jsonl` are denied (path rules checked with `claude -p`); INA delivers proposals to an agent only after you approve each one. Integration happens only through INA after user confirmation.
- **Verification commands:** come from the plan you approve, are limited to `cargo`, `node`, `python` and `pytest` (resolved `.exe` only; `.cmd` shims such as npm are refused), run without a shell, with validated args, on a throwaway checkout of the agent's committed branch, with a 10-minute limit and without `ANTHROPIC_*` variables. They run code the agent wrote (tests, build scripts): approving a check means trusting that code to run as you.
- **Messages:** outbox files are size-limited and schema-validated, with the sender's allowed types and known recipients enforced; invalid files are moved to `rejected/`. INA types into an agent's terminal only when you click "Notify terminal", and only one fixed line without digits or message text, so it cannot answer a permission prompt by itself.
- **Secrets:** Session variables of the parent Claude Code (`CLAUDECODE`, session ids, tokens, sockets) are removed from child processes; user configuration such as `CLAUDE_CONFIG_DIR` is kept. INA reads `~/.claude.json` only for presence flags (trusted folder, onboarding, logged in) and never writes it or returns account data to the UI.
- **No data loss by default:** cleanup refuses dirty worktrees and unmerged branches. Deleting untracked files requires a second explicit confirmation and is refused unless the branch is merged and no tracked file is modified.
- **Agent results are untrusted input:** `result.json` is size-limited and schema-validated; an invalid file never counts as success.
- **Content Security Policy:** scripts only from the app itself; IPC only to Tauri.

Known limits (Phase 1):
- Agents run Claude Code with its normal permissions inside their worktree; INA does not sandbox them beyond that.
- The orchestrator's deny list is a guardrail, not a sandbox: prefix rules like `Bash(git merge:*)` can be bypassed by other command forms (`git -C . merge`, shell redirection). The orchestrator still runs in the workspace root, so review what it does in its terminal.
