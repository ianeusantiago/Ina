# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased] — Phase 3 (architecture)

### Changed
- A background tick (every 5 s) applies results, starts verifications and ingests outboxes for every open team, so work moves on even with no team panel open; startup recovery moved off the window-opening path.
- Task registry exposes typed statuses (`Registry::tasks`/`task`); no more string comparisons.
- Team branch status runs git for each worktree in parallel.

## Phase 2 (reliable orchestration)

### Added
- Startup recovery: running tasks whose session was lost are finished with their result or as lost.
- Dependency-aware scheduler: prerequisites (and sequential mode) enforced, prerequisite branches merged into the dependent's worktree, blocked/unblocked dependents, optional auto-start, 60-minute agent timeout.
- Structured messages: `.ina/outbox/` → validated, stored and logged; answers and approved orchestrator instructions delivered to `.ina/inbox/` with a fixed terminal notice; Messages panel.
- Result verification: per-agent checks approved with the plan (`cargo`/`node`/`python`/`pytest`), new `verifying` status, declared vs changed files.
- Integration preview with `git merge-tree` (no refs or files touched) and overlapping-file warnings.
- `previousAttempt` in `task.json` when a task is re-run.
- Playwright E2E attached to the app over CDP; opt-in full team flow.
- Draggable (and keyboard-resizable) divider between the task/message panels and the terminals, remembered per machine.

### Security
- Orchestrator writes: outbox pre-allowed; agent worktrees, `.git/` and the event log denied.
- Checks run on a clean checkout of the committed branch, without `ANTHROPIC_*` variables, and cannot hang on inherited pipes.
- Terminal notices are user-triggered and contain no digits; integration only merges done agents.

### Changed
- The orchestrator starts in Claude Code's default permission mode; its only file writes are proposals in `.ina/outbox/` (Edit and git write commands stay denied).
- Main README is now in Portuguese (`README.md`); English moved to `README.en.md`.

### Fixed
- Agent timeout no longer kills a session whose task already finished or is being verified.
- A verification error marks the task failed instead of leaving it stuck in `verifying`.
- Prerequisites are not merged into a worktree while another session of that agent is running.
- Outbox messages are restored to the outbox if storing them fails (no silent loss).
- Message length limit counts characters, not bytes (accented text was rejected early).

## Phase 1 (MVP)

### Added
- Tauri 2 + React + TypeScript app with real PTY terminals (ConPTY via portable-pty) and xterm.js.
- Execution policy: program allowlist, argument and working-directory validation, `CLAUDE*` env stripping.
- Workspaces stored in SQLite.
- Team plan from natural language (`claude -p` with JSON schema), validation (ids, dependencies, cycles), review and approval screen.
- N+1 real Claude Code sessions: orchestrator (no file edits or git writes) and one agent per worktree.
- Read-only Claude Code folder-trust check with instructions when the folder is not trusted.
- One git worktree and branch per agent; branch status, controlled merge in dependency order with abort on conflict, cleanup (with optional, double-confirmed removal of untracked files).
- Teams persisted and resumable after restart; cleanup stops the team's sessions.
- `.ina/` protocol (`task.json`, `result.json`, `events.jsonl`) and shared task registry with a task panel.
- First-run check of Claude Code (installed, onboarded, logged in), language selector (pt-BR / English), maximize/restore terminal.

### Security
- Content Security Policy enabled; planner refuses untrusted folders; only session-specific `CLAUDE*` variables are stripped.

### Fixed
- Sequential mode and dependencies are enforced: an agent starts only after its prerequisites are done.
- Integration reports merges already done when a later merge fails for a non-conflict reason.
- A blocking terminal write no longer freezes all sessions.
- Duplicate dependencies no longer reported as a cycle; `.git/info/exclude` is never overwritten after a read error.
- Orphaned `claude` process when a terminal remounted before its session was registered.
