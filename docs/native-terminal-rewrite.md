# Native Terminal Rewrite

This document is the implementation spec for replacing Lantern's Tauri + xterm terminal stack with a Linux-native Rust app shell built on `gtk4-rs`, `libadwaita`, and `VTE`.

## Why

- Supacode is not using xterm or a webview terminal. It is a native macOS app built directly on GhosttyKit.
- The current Tauri/xterm path still shows trailing-character lag in agent UIs, which points to architectural overhead rather than a single remaining hot-path bug.
- Linux has a mature native terminal widget in VTE. We should use it.

## Rewrite Chunks

### Chunk 1: Workspace and shared core

- Create a Cargo workspace at the repo root.
- Introduce `lantern-core` for shared Rust models, config, DB access, layout persistence, and selection normalization.
- Add tests for stale repo/tab restore and layout roundtrips.

### Chunk 2: Native Linux app bootstrap

- Add `lantern-native-linux` using `libadwaita`, `gtk4`, and `vte4`.
- Boot a native app window and load repo/tab/layout state from `lantern-core`.
- Render sidebar, tab bar, status bar, and VTE-backed terminal stack.
- Spawn shells directly in VTE with repo cwd and config-derived shell/font settings.

### Chunk 3: Terminal host abstraction

- Define a native terminal-host interface around create/focus/close surface operations and runtime events.
- Implement it with VTE only for Linux.
- Keep the interface small enough that macOS and Windows backends can be added later without dragging GTK types into shared state.

### Chunk 4: Parity features

- Tab creation/close.
- Split panes.
- Search.
- Notifications and prompt/task lifecycle.
- Persisted layout and selection restore to a fresh prompt.

### Chunk 5: Cutover

- Keep the existing Tauri app working while the native client reaches parity.
- Move the main product path to the native client once the terminal experience is clearly better than the current stack.

## Initial Commands

```bash
cargo test -p lantern-core
cargo check -p lantern-native-linux
npm run dev
npm run build
npm run frontend:dev
npm run frontend:build
npm run tauri:dev
npm run tauri:build
npm run native:dev
npm run native:package
npm run native:install
```

## Current Status

- Linux cutover is implemented in the repo: the native GTK4/libadwaita/VTE client is the default Linux desktop path.
- `Chunk 1` is implemented in the repo.
- `Chunk 2` is implemented as a working native Linux app with real VTE terminals.
- `Chunk 3` is implemented for the Linux backend.
- `Chunk 4` is implemented for the Linux app: restore, tabs, tab reordering, search, settings, grouped repos/worktrees, git state, multi-pane native splits, native shortcuts, active process detection, shell lifecycle integration, and toast-based terminal feedback.
- Native Linux packaging now includes install/uninstall scripts, bundled app icons, dependency preflight checks, release checksums, a desktop entry template, AppStream metadata, a release bundling script, and a `lantern` launcher symlink under `apps/lantern-native-linux/packaging`.
- The staged tarball bundle now installs from its own bundled assets instead of assuming the source checkout is present.
- Top-level desktop wrapper scripts now route Linux to the native client and non-Linux to Tauri from `scripts/`.
- Top-level `npm run dev` and `npm run build` now follow that desktop routing as the default app entrypoints, while `frontend:*` and `tauri:*` scripts keep the old web/Tauri paths explicit.
- GitHub Actions now builds, tests, packages, and uploads the native Linux bundle from `.github/workflows/native-linux.yml`.
- Remaining work is maintenance, bug fixing, and any future non-Linux native work rather than Linux viability.
