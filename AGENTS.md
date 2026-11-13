# AgentBar — Agent Instructions

## Project Overview

**AgentBar** is a native macOS menu bar application written in Rust (`edition = "2024"`, MSRV `1.85`) that monitors AI coding agent accounts, usage limits, and active sessions, and provides quick launching from the top menu bar.

## Architecture

- `src/main.rs` — Entry point and logger initialization.
- `src/app.rs` — `tao` event loop (`ActivationPolicy::Accessory`), `TrayIcon` lifecycle, background scanner thread, and 30s auto-refresh ticker.
- `src/icon.rs` — Programmatic 22×22 monochrome template icon for Light/Dark macOS menu bars.
- `src/menu.rs` — Dynamic `muda` `NSMenu` builder and `MenuId -> MenuAction` registry.
- `src/model.rs` — Core domain types (`AgentKind`, `InstallStatus`, `AccountInfo`, `QuotaMetric`, `ActiveSession`, `RecentSession`, `AgentSnapshot`, `BarSnapshot`).
- `src/scanner/` — macOS process/CWD discovery (`ps`, `lsof`) and safe JSON/JSONC/read-only SQLite helpers.
- `src/providers/` — `AgentProvider` implementations (`agy`, `claude`, `cursor`, `codex`, `opencode`, `copilot`).
- `src/launcher.rs` — Terminal (`Ghostty`, `iTerm2`, `Warp`, `Terminal.app`) and GUI app launcher, native folder picker (`rfd`), AppleScript prompt dialog, and process termination.
- `scripts/build-app.sh` — Release build and `dist/AgentBar.app` bundler (`LSUIElement = true`).

## Coding & Rust Standards

- **Edition & Lints**: Keep `edition = "2024"`, `rust-version = "1.85"`, `[lints.rust] unsafe_code = "warn"`, and `[lints.clippy] all` + `pedantic` clean.
- **Zero-warning gate**: Always run `cargo test` and `cargo clippy --all-targets -- -D warnings` before committing.
- **No `unsafe` unless strictly required** and always documented with `// SAFETY:`.
- **Non-blocking UI**: Never run `ps`, `lsof`, SQLite queries, or file dialogs on the main AppKit thread; offload blocking work to worker threads and notify the event loop via `EventLoopProxy`.

## Security & Privacy Rules

- **Read-only local inspection**: Open agent SQLite databases strictly with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`.
- **Never expose secrets**: Never read, log, display, or store OAuth tokens, refresh tokens, or API keys from agent auth files (`auth.json`, `.claude.json`, etc.). Extract only non-sensitive metadata (email, plan, auth type, provider name, usage counts).
- **Safe shell quoting**: Always quote file paths, prompts, and AppleScript strings when invoking terminals or `osascript`.

## Adding a New Agent Provider

1. Add a variant to `AgentKind` in `src/model.rs` (update `display_name`, `slug`, `supports_cli_launch`, `gui_app_name`).
2. Add process classification rules in `src/scanner/process.rs` (`classify_process`), filtering out system XPC services and Electron helper processes.
3. Add binary resolution in `src/launcher.rs` (`resolve_agent_binary`).
4. Create `src/providers/<agent>.rs` implementing `AgentProvider` and register it in `src/providers/mod.rs`.
5. Add unit tests for any new parsing or classification logic.

## Git & Commits

- Use Conventional Commits without scope: `feat: ...`, `fix: ...`, `refactor: ...`, `docs: ...`, `chore: ...`.
- Write code, comments, and commit messages in English.
