# Lantern

Lantern is a Linux desktop app for organizing terminal sessions by repository.

The default Linux desktop path is now the native GTK4/libadwaita/VTE client in `apps/lantern-native-linux/`. The older Tauri + React + xterm app is still in the repo as a fallback path for non-Linux and legacy work.

## Quickstart

Requirements for the native Linux app:

- `cargo`
- `pkg-config`
- `libadwaita-1-dev`
- `libgtk-4-dev`
- `libsoup-3.0-dev`
- `libvte-2.91-gtk4-dev`

Run the default desktop app:

```bash
npm run dev
```

Build the default desktop app:

```bash
npm run build
```

Run the native Linux client directly:

```bash
npm run native:dev
```

Package the native Linux bundle:

```bash
npm run native:package
npm run native:verify-bundle
```

That produces:

- `apps/lantern-native-linux/dist/lantern-native-linux-x86_64.tar.gz`
- `apps/lantern-native-linux/dist/lantern-native-linux-x86_64.tar.gz.sha256`

The extracted tarball is self-installing through its bundled `install.sh`.

Install it to `~/.local`:

```bash
npm run native:install
```

## Repo Layout

- `apps/lantern-native-linux/`: native Linux GTK4/libadwaita/VTE app
- `crates/lantern-core/`: shared Rust config, DB, workspace, and git logic
- `scripts/`: top-level desktop wrappers
- `src/`: legacy React frontend
- `src-tauri/`: legacy Tauri backend
- `docs/`: design and rewrite notes

## Tests

Frontend tests:

```bash
npm test
```

Native/shared Rust tests:

```bash
cargo test -p lantern-core
cargo test -p lantern-native-linux
```

Legacy Tauri Rust tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## Legacy Paths

The legacy web/Tauri stack is still available explicitly:

```bash
npm run frontend:dev
npm run frontend:build
npm run tauri:dev
npm run tauri:build
```
