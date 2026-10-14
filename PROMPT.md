# INA — AI Agent Orchestration Desktop Application

Act as a senior software architect, desktop engineer, AI agent orchestration specialist, UI/UX designer and security engineer.

Build **INA**: a real, installable desktop app (target: **Windows 11 first**) that orchestrates multiple AI coding agents (Claude Code CLI) through independent, real terminal processes. UX reference: **Orca** (agent orchestrator). Not a mockup.

Project rules live in `CLAUDE.md` and take precedence over this prompt.

## 1. Operating rules
- Concise, direct, execution-focused. Minimize tokens and context.
- Phase 0 plan requires my approval before any file change or install.
- After approval: small verifiable increments; ask approval only at checkpoints (new module, new dependency, destructive action), not trivial details.
- Never claim it works/passes/builds without running it and showing evidence.
- No unnecessary dependencies or abstractions. Sensible defaults for non-critical decisions; one concise question if a critical one is ambiguous.

## 2. Product
Dark desktop control center. User can:
1. Manage workspaces linked to local directories.
2. Create an orchestration session from a natural-language task.
3. Define **1 orchestrator + N agents** in one natural-language prompt.
4. Orchestrator interprets the task, splits work, assigns roles, coordinates.
5. After approval, INA opens **N+1 real terminal sessions** (one orchestrator, one per agent), each an independently managed process running Claude Code.
6. Monitor output, status, errors and progress in one UI.
7. Stop/restart/inspect individual sessions.
8. Review proposed actions/changes before allowing them.
9. See summary: done, failed, pending, verification results.

## 3. Stack (validate versions against official docs first)
Tauri 2 · React + TypeScript (strict) + Vite · Tailwind · xterm.js · Rust backend with real PTY (`portable-pty` / ConPTY on Windows) · Vitest + Rust tests · Playwright E2E · native installers (MSI/NSIS).
Recommend alternatives only if materially better; state trade-off in one line.
INA orchestrates local CLI sessions; it does not include model access or bypass Claude Code authentication.

## 4. Architecture (modules)
Shell & workspaces · Process/PTY lifecycle · Agent config & roles · Orchestrator & planning · Agent coordination (message bus) · Session state & events · Permissions/approvals · Persistence & recovery · Logging/diagnostics.
Typed interfaces, explicit state machines. UI separate from process and orchestration logic. Backend is the sole authority for process execution and permissions. Persist metadata/config only; no terminal output or secrets by default.

## 5. Orchestrator and agents
**Orchestrator:** interprets task; extracts requirements, constraints, dependencies, acceptance criteria; splits into assignments; defines roles; marks parallel vs sequential; assigns tasks and keeps shared task state; gives each agent role, context, deliverables, restrictions; monitors progress, failures and conflicts; asks for info when needed; validates and integrates; reports honestly (including unverified work). Opening a terminal ≠ agent started or finished.

**Each agent:** unique id, name, role, scope, dedicated terminal, assignment + acceptance criteria, minimal context, visible status + history, structured progress/result reporting, failure/cancel/recovery path.

Example roles (not mandatory): architecture, frontend/UI, backend/integrations, tests/QA, security, review/integration.

**Dynamic team:** e.g. "1 orchestrator and 3 agents: frontend, backend, tests+security" → INA shows structured plan (roles, responsibilities, dependencies, session count) → approval → launches 4 sessions.

**Communication:** explicit mechanism — shared task records (file/SQLite), structured messages (JSON), status events, result reports. Do not rely on agents reading each other's terminals. Define how instructions are injected into each Claude Code session (e.g. initial prompt + per-agent `CLAUDE.md`/task file) and how results are read back.

## 6. Parallel safety
One **git worktree per agent**; controlled review/test/merge by orchestrator; sequential fallback if isolation unavailable. Implement task ownership, dependency tracking, cancel/timeout, error propagation, conflict detection, verification before success. No agent touches another's files or runs privileged ops without authorization.

## 7. UI
Name: INA. Dark, minimal, professional (black, charcoal, graphite, gray; subtle borders; restrained accent; no gradients/excess animation).
Layout: sidebar (workspaces/sessions) · central resizable terminal grid · orchestrator selector + agent role inspector · task panel (assignments, dependencies, progress, errors) · status bar · team/instructions config · approval dialogs.
Keyboard navigation and accessible controls. Function over effects.
**Languages:** UI fully bilingual — **Português (Brasil)** and **English**, switchable in settings, default from OS locale. Use i18n (e.g. `i18next`); no hardcoded UI strings.

## 8. Security
Validate executables, args, cwd and workspace boundaries · no shell-string interpolation · secrets out of logs/code/frontend state (use OS credential store if needed) · policy checks on anything agents request · explicit approval for destructive/privileged actions showing executable, cwd and command · always-available cancel/kill · least privilege · document isolation limits honestly.

## 9. Quality
Clear module boundaries, small functions, strict TS, ESLint + Prettier, structured logs, explicit error handling, unit + integration tests (process lifecycle, task assignment, authorization, cancellation), Playwright E2E for main flows. No fake features, no swallowed errors, no hardcoded credentials.

**Documentation (dedicated responsibility):** one agent/role owns docs and keeps them in sync with code every phase. Deliver, in **pt-BR and English**, GitHub-ready:
- `README.md` (EN) + `README.pt-BR.md`: overview, screenshots, features, prerequisites, install, quick start, usage, roadmap, license.
- `docs/`: architecture (with diagrams), orchestration/agent model, security model, setup, development, testing, packaging, troubleshooting, FAQ.
- `CONTRIBUTING.md`, `CHANGELOG.md`, `LICENSE`, `SECURITY.md`, issue/PR templates.
Docs must match real, verified behavior — never document unbuilt features as done.

## 10. Tooling (pre-approved — use freely when relevant)
- **Superpowers:** brainstorming, writing-plans, executing-plans, test-driven-development, systematic-debugging, using-git-worktrees, dispatching-parallel-agents, subagent-driven-development, requesting-code-review, verification-before-completion.
- **Playwright:** E2E of the UI (Vite dev server; Tauri app via WebView2 CDP).
- **Design:** frontend-design, ui-ux-pro-max, ui-styling.
- **Review:** code-review, security-review, simplify at the end of each phase.
- Installing a NEW plugin/MCP/dependency: show purpose, source and command, then ask.

## 11. Distribution
Reproducible dev setup · production build · Windows installer · first-run check for Claude Code presence and auth · graceful handling if missing · docs for build/install/update/troubleshoot. Claim support only for platforms actually tested.

## 12. Phases
- **Phase 0 — Plan:** inspect env (OS, tooling, Claude Code config). Propose stack, architecture, MVP vs deferred, orchestration design, PTY strategy, security boundaries, risks, steps, verification. Wait for approval.
- **Phase 1 — MVP:** shell, workspace selection, real PTY sessions, orchestrator + configurable agents, N+1 managed processes, task assignment/status, basic approvals, essential persistence. No placeholder screens.
- **Phase 2 — Reliable orchestration:** structured messaging, dependency-aware scheduling, worktrees, failure recovery, result verification.
- **Phase 3 — Production:** tests, security review, performance, accessibility, docs, installer.
Reprioritize if a smaller milestone delivers more value.

## 13. Pause / resume
- **"PAUSAR INA"**: stop, write `docs/STATE.md` (phase, done, in progress, next, decisions, blockers), update memory, commit, push if remote exists. One-line reply.
- **"RETOMAR INA"**: read `docs/STATE.md` + memory + `git status`; 3-line status and next step.

## 14. First response format
1. Assessment 2. Stack 3. Architecture 4. MVP scope 5. Risks 6. Plan 7. Tools to use 8. Exact commands to start. End asking for approval.
