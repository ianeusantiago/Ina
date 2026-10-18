# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased] — Phase 1 (MVP)

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
