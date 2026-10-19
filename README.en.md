<p align="center">
  <img src="src/assets/ina-icon.svg" alt="INA icon" width="120" />
</p>

<h1 align="center">INA</h1>

<p align="center"><strong>A team of AI agents working on your project — with you in charge.</strong></p>

<p align="center">
  <a href="https://github.com/ianeusantiago/Ina/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/ianeusantiago/Ina?color=1a8fe0" /></a>
  <a href="https://github.com/ianeusantiago/Ina/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/ianeusantiago/Ina/actions/workflows/ci.yml/badge.svg" /></a>
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-0a78d8" />
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-3aa83a" /></a>
</p>

<p align="center"><a href="README.md">Português</a> (main documentation)</p>

---

INA is a Windows desktop app that assembles and coordinates a **team of AI agents**: one **orchestrator** plus several **agents**, each running the real [Claude Code](https://docs.claude.com/claude-code) CLI in its own terminal and its own copy of the project (git worktree). Describe what you want, approve the plan, and follow everything in a chat-like interface.

## Download

1. Get **`INA_x.y.z_x64-setup.exe`** from **[Releases](https://github.com/ianeusantiago/Ina/releases/latest)** and install it.
2. If Windows shows "Windows protected your PC", click **More info → Run anyway** (the installer has no code-signing certificate yet).
3. New versions show up inside the app: **New version available → Update and restart**.

**Requirements:** Windows 11, [Claude Code](https://docs.claude.com/claude-code) installed and logged in, and [Git](https://git-scm.com/). Agents use **your** Claude plan; INA shows how much each team used.

## How it works

1. **Ask on the Home screen** — e.g. _"Create a workspace called Shop and build a product page with 2 agents"_ — or start from a template. INA creates the project folder, sets up the team and shows the plan.
2. **Approve.** Nothing runs before you approve. Each agent gets its own copy of the project.
3. **Follow along in the Interface.** Chat with the orchestrator and each agent. Permission requests show up as **Allow / Always allow / Deny** cards.
4. **Get the result.** When everyone is done, INA writes a plain summary (what was done, how to try it), offers an AI **review**, and merges the work after a conflict-free preview.

## Highlights

- Real terminals (ConPTY), dependency-aware scheduling, real verification of results, controlled integration that never deletes unmerged work.
- Tree sidebar with live status, progress panel, token usage per team, Windows notifications, resume conversations after restarting.
- In-app project preview (sandboxed), signed auto-updates, Frutiger Aero look with light/dark/system themes, Portuguese and English UI.

Security model and known limits: [SECURITY.md](SECURITY.md) (in Portuguese).

## Development

Extra requirements: Rust (stable, MSVC), Visual Studio Build Tools 2022, Node.js 22+.

```sh
npm install
npm run tauri dev
npm test && npm run lint && npx tsc --noEmit
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

CI runs these checks on every PR. Push a `v*` tag to build, sign and publish a release. See [README.md](README.md#desenvolvimento) for E2E tests and the code layout.

## License

[MIT](LICENSE) © 2026 Ian Santiago
