# Repository Guidelines

## Project Structure & Module Organization

`apps/lantern-native-linux/` is the primary Linux desktop app; keep GTK/libadwaita/VTE UI code there. `crates/lantern-core/` holds shared Rust models, config loading, SQLite persistence, workspace restore logic, and git/worktree helpers. `scripts/` contains the top-level desktop wrappers and Linux packaging entrypoints. `src/` and `src-tauri/` remain the legacy React/Tauri fallback path; keep UI in `src/components`, shared state in `src/stores`, reusable hooks in `src/hooks`, Tauri bridge/helpers in `src/lib`, shared types in `src/types`, and global/theme styles in `src/styles`. Design notes live in `docs/DESIGN.md`. Treat `dist/`, `target/`, and `src-tauri/target/` as generated output.

## Build, Test, and Development Commands

- `npm run dev`: start the default desktop app for the current platform. On Linux this launches the native GTK/libadwaita/VTE client.
- `npm run build`: build the default desktop app for the current platform. On Linux this stages the native release bundle.
- `npm run frontend:dev`: start the Vite frontend for local UI work.
- `npm run frontend:build`: run `tsc` and produce the production frontend bundle.
- `npm run preview`: serve the built frontend locally.
- `npm test`: run the Vitest suite once in `jsdom`.
- `npm run test:watch`: rerun frontend tests during development.
- `npm run native:package`: build the native Linux tarball plus checksum.
- `npm run native:install`: install the native Linux app into `~/.local`.
- `cargo test -p lantern-core`: run Rust unit tests for the shared native/core crate.
- `cargo test -p lantern-native-linux`: run Rust unit tests for the native Linux desktop app.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust unit tests for the Tauri backend.

## Coding Style & Naming Conventions

Use 2-space indentation and double quotes in TypeScript and TSX, matching the current codebase. React components use PascalCase filenames and exports such as `Sidebar.tsx`; hooks use `useX.ts`; stores and utilities use descriptive camelCase names. Keep component styles in colocated `*.module.css` files. Rust modules should stay snake_case. No dedicated ESLint or Prettier config is checked in, so match surrounding style and keep edits surgical.

## Testing Guidelines

Frontend tests use Vitest with Testing Library and are colocated as `*.test.ts` or `*.test.tsx` next to the source they cover, for example `src/components/Sidebar/Sidebar.test.tsx`. Rust tests live inside the owning module with `#[cfg(test)]`. Prefer behavior-focused tests around rendering, store transitions, terminal/runtime state, and command boundaries. Run `npm test` before every PR; if you touch `apps/lantern-native-linux/` or `crates/lantern-core/`, run the matching `cargo test -p ...` commands; if you touch `src-tauri/`, also run the Tauri Rust tests.

## Commit & Pull Request Guidelines

Recent history uses short imperative subjects such as `Add theme system...` and `Fix WebKitGTK compositing crash on Wayland`. Follow that pattern: start with `Add`, `Fix`, `Refactor`, or similar, then describe the concrete change. Keep pull requests focused, include linked issues when relevant, summarize user-visible impact, call out Linux or Tauri-specific behavior, and attach screenshots or GIFs for UI changes.
