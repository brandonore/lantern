# Repository Guidelines

## Project Structure & Module Organization

`src/` contains the React 19 frontend. Keep UI in `src/components`, shared state in `src/stores`, reusable hooks in `src/hooks`, Tauri bridge/helpers in `src/lib`, shared types in `src/types`, and global/theme styles in `src/styles`. `src-tauri/` contains the Rust desktop backend; Tauri commands live in `src-tauri/src/commands`, with PTY, git, config, and SQLite support in sibling modules. Design notes live in `docs/DESIGN.md`. Treat `dist/` and `src-tauri/target/` as generated output.

## Build, Test, and Development Commands

- `npm run dev`: start the Vite frontend for local UI work.
- `npm run build`: run `tsc` and produce the production frontend bundle.
- `npm run preview`: serve the built frontend locally.
- `npm test`: run the Vitest suite once in `jsdom`.
- `npm run test:watch`: rerun frontend tests during development.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust unit tests for the Tauri backend.

## Coding Style & Naming Conventions

Use 2-space indentation and double quotes in TypeScript and TSX, matching the current codebase. React components use PascalCase filenames and exports such as `Sidebar.tsx`; hooks use `useX.ts`; stores and utilities use descriptive camelCase names. Keep component styles in colocated `*.module.css` files. Rust modules should stay snake_case. No dedicated ESLint or Prettier config is checked in, so match surrounding style and keep edits surgical.

## Testing Guidelines

Frontend tests use Vitest with Testing Library and are colocated as `*.test.ts` or `*.test.tsx` next to the source they cover, for example `src/components/Sidebar/Sidebar.test.tsx`. Rust tests live inside the owning module with `#[cfg(test)]`. Prefer behavior-focused tests around rendering, store transitions, and command boundaries. Run `npm test` before every PR; if you touch `src-tauri/`, also run the `cargo test` command above.

## Commit & Pull Request Guidelines

Recent history uses short imperative subjects such as `Add theme system...` and `Fix WebKitGTK compositing crash on Wayland`. Follow that pattern: start with `Add`, `Fix`, `Refactor`, or similar, then describe the concrete change. Keep pull requests focused, include linked issues when relevant, summarize user-visible impact, call out Linux or Tauri-specific behavior, and attach screenshots or GIFs for UI changes.
