# AgentBar

Native macOS menu bar monitor and launcher for AI coding agents, written in Rust (inspired by Omarchy's topbar agent menu).

## Features

- **Native macOS Menu Bar App (`NSStatusBar` / `NSMenu`)**: Runs as an accessory app (`LSUIElement`) with no Dock icon and a monochrome template icon that adapts automatically to Light and Dark mode.
- **Active Sessions Monitor**: Detects running agent CLI and IDE processes via `ps` + `lsof`, showing PID, project folder, CPU%, memory usage, and elapsed time, with actions to open the folder in Finder, open a terminal there, or send `SIGTERM`.
- **Accounts, Limits & Usage**: Reads local state (read-only) for each agent environment to display active accounts, plans/auth methods, token/cost/session usage, and recent sessions.
- **Quick Launch from the Menu**:
  - Launch interactive CLI sessions in **Ghostty**, **iTerm2**, **Warp**, or **Terminal.app** (auto-detected).
  - Pick a project folder via native macOS folder dialog (`Launch in Folder…`).
  - Enter a prompt via native dialog (`Launch with Prompt…`).
  - Open GUI agent apps (`Antigravity IDE`, `Cursor`, `Claude`).
- **Auto-Refresh**: Background polling every 30s (toggleable) plus manual `Refresh Now`.

## Supported Agents

| Agent | CLI / App | Local State Inspected |
|---|---|---|
| **Antigravity (`agy`)** | `agy`, `Antigravity IDE.app` | `~/.gemini/settings.json`, `~/.gemini/google_accounts.json`, `~/.gemini/antigravity/brain` |
| **Claude Code** | `claude`, `Claude.app` | `~/.claude.json`, `~/.claude/profiles`, `~/.claude/stats-cache.json`, `~/.claude/history.jsonl` |
| **Cursor** | `cursor`, `Cursor.app` | `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, `~/.cursor/projects` |
| **OpenAI Codex** | `codex` | `~/.codex/auth.json`, `~/.codex/state_5.sqlite` |
| **OpenCode** | `opencode` | `~/.local/share/opencode/auth.json`, `~/.config/opencode/opencode.jsonc`, `~/.local/share/opencode/opencode.db` |
| **GitHub Copilot** | `copilot`, `gh copilot` | `~/.copilot`, `~/.config/gh/hosts.yml` |

## Build & Run

```bash
# Run in development
cargo run

# Run tests and lints
cargo test
cargo clippy --all-targets -- -D warnings

# Build macOS .app bundle (outputs dist/AgentBar.app)
chmod +x scripts/build-app.sh
./scripts/build-app.sh
open dist/AgentBar.app
```

## Architecture

- `src/model.rs` — Core domain types (`AgentKind`, `AccountInfo`, `QuotaMetric`, `ActiveSession`, `BarSnapshot`)
- `src/scanner/` — macOS process/CWD discovery (`ps`, `lsof`) and safe JSON/JSONC/SQLite helpers
- `src/providers/` — Per-agent state & session collectors (`agy`, `claude`, `cursor`, `codex`, `opencode`, `copilot`)
- `src/launcher.rs` — Terminal & GUI launcher, folder picker, prompt dialog, process control
- `src/icon.rs` — Programmatic 22×22 monochrome template icon
- `src/menu.rs` — Dynamic `muda` `NSMenu` builder and action registry
- `src/app.rs` — `tao` accessory event loop and background refresh workers
