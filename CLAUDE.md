# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Lantern

Lantern is a lightweight Linux desktop app for organizing terminal sessions grouped by repository. Sidebar of repos on the left, terminal tabs per repo on the right. The default Linux desktop path is now the native GTK/libadwaita/VTE client, with the older Tauri v2 + React 19 + xterm.js app kept as an explicit fallback path. Not an IDE.

## Commands

```bash
# Default desktop app for the current platform
npm run dev

# Default production desktop build for the current platform
npm run build

# Frontend dev server (Vite on port 1420)
npm run frontend:dev

# Frontend production bundle
npm run frontend:build

# Legacy Tauri app (frontend + Rust backend)
npm run tauri:dev
npm run tauri:build

# Native Linux app directly
npm run native:dev
npm run native:build

# Frontend tests (vitest, jsdom)
npm test                    # run once
npm run test:watch          # watch mode
npx vitest run src/stores/appStore.test.ts  # single test file

# Rust tests
cargo test -p lantern-core
cargo test -p lantern-native-linux
cd src-tauri && cargo test

# Type check frontend
npx tsc --noEmit

# Rust check
cargo check -p lantern-native-linux
cd src-tauri && cargo check
```

## Architecture

### Current desktop paths

**Native Linux app** (`apps/lantern-native-linux/`) — GTK4/libadwaita shell with VTE-backed terminals. This is the default desktop path on Linux through the top-level wrapper scripts.

**Shared core** (`crates/lantern-core/`) — shared Rust models, config loading, SQLite persistence, workspace restore/normalization, and git/worktree utilities used by the native Linux client.

**Legacy Tauri app** (`src-tauri/` + `src/`) — Rust backend plus React/xterm.js frontend kept for non-Linux and fallback work.

### Primary Linux-native modules

| Module | Purpose |
|--------|---------|
| `apps/lantern-native-linux/src/app.rs` | Main GTK/libadwaita window, sidebar, tabs, splits, search, settings, and runtime state wiring |
| `apps/lantern-native-linux/src/terminal_host.rs` | VTE-backed terminal surface host and shell spawning |
| `apps/lantern-native-linux/src/theme.rs` | Native theme normalization and VTE palette application |
| `crates/lantern-core/src/db.rs` | SQLite schema, migrations, layout/session persistence, repo ordering, native split persistence |
| `crates/lantern-core/src/workspace.rs` | Workspace restore normalization and repo/tab ordering logic |
| `crates/lantern-core/src/git.rs` | Branch/dirty/divergence info, worktree discovery, foreground process classification |
| `apps/lantern-native-linux/packaging/` | Native Linux install/package/uninstall scripts, desktop metadata, dependency preflight |
| `scripts/desktop-*.sh` | Top-level desktop dev/build/install wrappers that route Linux to the native client |

### Native runtime model

- GTK4/libadwaita renders the window chrome, sidebar, tab strip, settings, and search UI.
- VTE owns terminal rendering, PTY lifecycle, clipboard, search integration, and shell lifecycle signals.
- `lantern-core` owns persisted state and restore logic so the native UI stays thin.
- Active process detection still uses lightweight `/proc` inspection, but shell prompt/task transitions come from VTE signals instead of frontend echo hacks.

### Legacy Tauri architecture

**Rust backend** (`src-tauri/src/`) — PTY management, SQLite persistence, git status polling, TOML config. No HTTP server; communicates with frontend exclusively through Tauri commands and events.

**React frontend** (`src/`) — UI rendering, xterm.js terminal instances, state management. Runs inside native webview.

### Backend modules

| Module | Purpose |
|--------|---------|
| `pty.rs` | `PtyManager` — spawns PTY sessions via `portable-pty`, one reader thread per session, 4KB read chunks |
| `db.rs` | SQLite with WAL mode. Tables: `repo`, `terminal_session`, `app_state`, `active_tab`, `schema_version`. `DbConn` = `Arc<Mutex<Connection>>` |
| `git.rs` | Uses `git2` crate for branch/dirty/ahead/behind info. Also reads `/proc/{pid}/stat` for foreground process detection (agent detection) |
| `config.rs` | `UserConfig` from `~/.config/lantern/config.toml`. JSON patch updates via `merge_patch()` |
| `state.rs` | `AppState` — holds PtyManager, DbConn, UserConfig, sidebar_width, git_poll_interval |
| `error.rs` | `LanternError` enum with Serialize impl for Tauri error responses |
| `commands/` | Tauri command handlers: `repo.rs`, `terminal.rs`, `pty_io.rs`, `config.rs`, `layout.rs` |
| `main.rs` | App init, git polling background thread (configurable interval), window close handler |

### Frontend modules

| Module | Purpose |
|--------|---------|
| `stores/appStore.ts` | Single Zustand store + Immer middleware. Holds repos (with nested tabs + git info), activeRepoId, config, UI state. `hydrate()` loads everything on startup |
| `lib/terminalManager.ts` | Singleton managing xterm.js instances (Map<tabId, ManagedTerminal>). Handles creation, addon loading (WebGL with fallback), resize via ResizeObserver + rAF, PTY subscription |
| `lib/tauriCommands.ts` | Type-safe wrappers around `invoke()` for all backend commands |
| `lib/themes/` | 10 theme families (nord, tokyo-night, catppuccin, etc.), each with dark/light variants. ThemeVariant has UI colors + ANSI terminal colors |
| `lib/theme.ts` | `applyTheme()` sets CSS custom properties, `getTerminalTheme()` builds xterm.js ITheme |
| `types/index.ts` | Shared TypeScript types matching Rust serialized structs |

### Frontend-backend communication

- **Tauri commands** (`invoke()`) — request/response for CRUD operations
- **Tauri events** — backend emits `git-status-update` every N seconds with `Vec<(String, GitInfo)>`
- **Tauri channels** — PTY output streaming: `terminal_subscribe` creates a `Channel<TerminalOutputData>` for real-time data flow

### Key data flow: terminal lifecycle

1. User clicks "+" tab → store calls `addTab()` → backend creates DB row
2. `TerminalViewport` renders `TerminalInstance` (lazy: only created when first visible)
3. `TerminalInstance` calls `terminalManager.create()` → creates xterm.js Terminal, loads addons, calls `terminalSubscribe()` on backend
4. Backend spawns PTY process, starts reader thread → output flows through Channel to xterm.js
5. User types → xterm.js `onData` → `terminalWrite()` command → backend writes to PTY

### Component hierarchy

```
App → AppShell
  ├── TitleBar (custom, decorations: false)
  ├── Sidebar (repo list + add/settings buttons, drag-resizable)
  ├── TabBar (terminal tabs for active repo, inline rename)
  ├── TerminalViewport (renders all TerminalInstances, shows only active)
  │   └── TerminalInstance (xterm.js per tab, lazy creation)
  └── StatusBar (branch, dirty, ahead/behind, agent detection)
  └── SettingsDialog (modal, live preview, cancel reverts)
```

### Styling

CSS Modules + CSS custom properties for theming. No Tailwind. Theme colors applied as `--bg-primary`, `--accent`, etc. on document root. UI scale via `--ui-scale` custom property.

### Hooks

- `useTheme` — watches config.theme, applies CSS vars + terminal colors
- `useGitPoller` — listens to `git-status-update` Tauri event
- `useShortcuts` — keyboard shortcuts (Ctrl+T new tab, Ctrl+W close, etc.)
- `useUiScale` — applies `--ui-scale` CSS variable from config
- `useSidebarResize` — drag-to-resize sidebar
- `useAgentDetector` — polls foreground process to detect Claude Code/codex/aider

## Conventions

- Rust error handling via `thiserror` + serializable `LanternError` enum
- All PTY sessions identified by UUID string (same as terminal_session.id in DB)
- Frontend state mutations use Immer (via Zustand middleware) — mutate draft directly
- Theme IDs are `"family-mode"` strings, e.g. `"nord-dark"`, `"catppuccin-light"`
- Tests mock Tauri's `invoke` in `appStore.test.ts` — pattern: mock command responses, call store action, assert state
- Rust tests use `tempfile` crate for isolated DB/config testing
- The design doc at `docs/DESIGN.md` contains detailed specs and decisions
