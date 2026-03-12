# Lantern — Terminal Workspace Manager

## Status

The current default Linux desktop path is the native GTK4/libadwaita/VTE client in `apps/lantern-native-linux/`, backed by shared Rust state in `crates/lantern-core/`. The Tauri + React + xterm architecture described below remains in the repo as the legacy fallback path and historical design context, not the primary Linux runtime.

## Context

No good Linux GUI exists for organizing multiple terminal sessions grouped by repository. Existing tools are either macOS-only (Conductor, Supacode), require tmux, or are full IDEs. Lantern is a lightweight desktop app: a sidebar of repos on the left, terminal tabs per repo on the right. Inspired by Conductor's layout. Clean, modern, dark theme. Not an IDE.

## Tech Stack

### Current default Linux stack

| Layer | Choice | Why |
|-------|--------|-----|
| App framework | **GTK4 + libadwaita** | Native Linux app shell, native widgets, no webview terminal overhead |
| Terminal rendering | **VTE** | Native terminal widget and PTY integration for Linux |
| Shared state/core | **Rust workspace crates** | Reuse config, DB, restore, and git logic without UI coupling |
| Database | **rusqlite (SQLite)** | Embedded, zero-config, persists repos/tabs/layout |
| Config | **TOML** | Human-editable, Rust-native |
| Git info | **git2** crate | No subprocess overhead, structured output |
| Packaging | **shell scripts + tarball bundle** | Simple Linux packaging/install path with CI artifacts |

### Legacy Tauri stack

| Layer | Choice | Why |
|-------|--------|-----|
| App framework | **Tauri v2** | Rust backend, small binary (~15MB), native webview, ~60MB RAM |
| Frontend | **React 19 + TypeScript** | Familiar, largest ecosystem, best xterm.js docs |
| Terminal rendering | **xterm.js + WebGL addon** | De facto web terminal standard, GPU-accelerated |
| PTY management | **portable-pty** crate | Battle-tested (from WezTerm), cross-platform |
| State management | **Zustand + immer** | Simple, flat store, handles nested tab arrays |
| Styling | **CSS Modules + CSS custom properties** | Full control, clean theming, no Tailwind bloat |
| Database | **rusqlite (SQLite)** | Embedded, zero-config, persists repos/tabs/layout |
| Config | **TOML** | Human-editable, Rust-native |
| Git info | **git2** crate | No subprocess overhead, structured output |
| Build | **Vite** | Fast dev server, Tauri default |

No backend API, no Postgres, no Docker. Pure local desktop app.

### Why Tauri Over Alternatives

| Option | Binary Size | Memory | Terminal Support | Verdict |
|--------|------------|--------|-----------------|---------|
| **Tauri v2** | ~15MB | ~60MB | xterm.js (excellent) | **Best balance** |
| Electron | ~150-300MB | ~200-400MB | xterm.js (same) | Works but bloated |
| Flutter | ~20MB | ~100MB | No mature terminal widget | Dealbreaker |
| Qt/GTK (C++) | ~30MB | ~60MB | QTermWidget (decent) | Painful to style, no React |
| Iced (pure Rust) | ~15MB | ~40MB | iced_term (unstable) | Terminal widget not ready |

### AI Agent Auth

No special handling. Terminals are real shell sessions inheriting the user's full environment (`$HOME`, `$PATH`, env vars, dotfiles). When a user runs `claude` or `codex` in a Lantern terminal:

- Claude Code reads auth from `~/.claude/` (OAuth tokens, API keys)
- Codex reads its auth from its config dir
- Any `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in the user's env is inherited

If you've already authenticated once on your machine, opening two tabs and typing `claude` in one and `codex` in the other will work immediately with no login prompts. Lantern is just the terminal host.

### Parallel Agents

Supported by design. Each tab is an independent PTY process. Run Claude Code in tab 1, Codex in tab 2, another agent in tab 3 — all running simultaneously with zero coordination overhead.

### Future Worktrees (v2)

Additive, not a rewrite. `git2` already supports worktree operations (`Repository::worktrees()`, `Repository::worktree()`). Would add:
- A `worktree` table in SQLite (parent_repo_id, branch, path)
- Expand repo items in sidebar to show sub-worktrees
- Each worktree gets its own terminal tabs (cwd set to worktree path)
- Existing PtyManager/TerminalManager work unchanged

---

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                   Tauri Window                   │
│  ┌──────────┐  ┌────────────────────────────┐   │
│  │ Sidebar  │  │ Tab Bar: [T1] [T2] [+]     │   │
│  │          │  ├────────────────────────────┤   │
│  │ repo-1   │  │                            │   │
│  │  main *  │  │   xterm.js (WebGL)         │   │
│  │          │  │                            │   │
│  │ repo-2   │  │   ← Tauri Channel →       │   │
│  │  feat/x  │  │     (binary PTY I/O)       │   │
│  │          │  │                            │   │
│  │ [+Add]   │  ├────────────────────────────┤   │
│  └──────────┘  │ Status: ~/repo zsh 80x24 ⚡Claude │
│                └────────────────────────────┘   │
└─────────────────────────────────────────────────┘
        │                    │
        │   Tauri IPC        │
        ▼                    ▼
┌─────────────────────────────────────────────────┐
│              Rust Backend (src-tauri/)           │
│                                                 │
│  PtyManager ── portable-pty (spawn, resize, IO) │
│  DB ── rusqlite (repos, tabs, layout state)     │
│  Config ── TOML (~/.config/lantern/config.toml) │
│  GitWatcher ── git2 (poll branch/dirty status)  │
│  AgentDetector ── /proc/PID/cmdline parsing     │
└─────────────────────────────────────────────────┘
```

### Data Flow: Terminal I/O

```
User types in xterm.js
  → terminal.onData(data)
  → invoke('terminal_write', { sessionId, data })  [Tauri IPC]
  → Rust: write bytes to PTY master fd
  → PTY subprocess (bash/zsh) processes input
  → PTY produces output on master fd
  → Rust reader thread: read 4KB chunks
  → Channel.send(bytes)  [Tauri Channel, ordered, binary]
  → Frontend: terminal.write(bytes)
  → xterm.js renders via WebGL
```

### Data Flow: Tab Lifecycle

```
User clicks [+] tab button
  → store.addTab(repoId)
  → invoke('terminal_create', { repoId })
  → Rust: insert session into SQLite, spawn PTY in repo cwd
  → Return TerminalSession { id, ptyId, ... }
  → Store updates: push tab into repo.tabs, set activeTabId
  → React renders new <TerminalInstance>
  → useEffect: terminalManager.create(tabId, ...)
  → invoke('terminal_subscribe', { sessionId, channel })
  → Channel streaming begins
```

### Data Flow: App Startup

```
Tauri app launches
  → Rust: init SQLite, load config, start GitWatcher
  → Frontend mount: store.hydrate()
  → invoke('repo_list') → populate repos with tabs
  → invoke('state_load_layout') → restore window/sidebar/active state
  → Normalize stale repo/tab selections to a valid active terminal
  → Active terminal mounts, subscribes, and spawns a fresh PTY on demand
  → Inactive tabs stay as saved structure until opened
  → Git poller starts (5s interval)
  → Active terminal foreground-process polling stays live for agent detection
```

---

## Data Storage

### Config — `~/.config/lantern/config.toml`

User-editable, human-readable. Version-controllable.

```toml
# Shell to use when creating new terminal tabs
default_shell = "/usr/bin/bash"

# Terminal appearance
font_family = "JetBrains Mono"
font_size = 14
scrollback_lines = 10000

# Theme name ("dark", "light", or path to custom theme TOML)
theme = "dark"

# How often to poll git status (seconds). 0 = disabled.
git_poll_interval_secs = 5

# Keybindings (optional overrides)
[keys]
new_tab = "Ctrl+T"
close_tab = "Ctrl+W"
next_tab = "Ctrl+Tab"
prev_tab = "Ctrl+Shift+Tab"
toggle_sidebar = "Ctrl+B"
```

### State — `~/.local/share/lantern/lantern.db` (SQLite)

Machine-specific, changes frequently, not user-editable.

```sql
-- Repositories tracked in the sidebar
CREATE TABLE repo (
    id          TEXT PRIMARY KEY,            -- UUID v4
    name        TEXT NOT NULL,               -- display name (defaults to dir name)
    path        TEXT NOT NULL UNIQUE,        -- absolute filesystem path
    sort_order  INTEGER NOT NULL DEFAULT 0   -- sidebar position
);

-- Terminal sessions (persisted so tabs survive restart)
CREATE TABLE terminal_session (
    id          TEXT PRIMARY KEY,            -- UUID v4
    repo_id     TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,               -- tab label ("Terminal 1")
    shell       TEXT,                        -- shell override, NULL = use default
    sort_order  INTEGER NOT NULL DEFAULT 0   -- tab position within repo
);

-- Singleton table for layout/window state
CREATE TABLE app_state (
    id              INTEGER PRIMARY KEY CHECK (id = 1),  -- enforce single row
    window_x        INTEGER,
    window_y        INTEGER,
    window_width    INTEGER NOT NULL DEFAULT 1200,
    window_height   INTEGER NOT NULL DEFAULT 800,
    window_maximized INTEGER NOT NULL DEFAULT 0,
    sidebar_width   INTEGER NOT NULL DEFAULT 250,
    active_repo_id  TEXT REFERENCES repo(id) ON DELETE SET NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Which tab is active per repo
CREATE TABLE active_tab (
    repo_id     TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
    session_id  TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
);

-- Schema version for migrations
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY
);
INSERT INTO schema_version VALUES (1);
```

**What goes where:**

| Data | Storage | Rationale |
|------|---------|-----------|
| Default shell, font, theme, keybindings | TOML config | User-editable, version-controllable |
| Window position/size, sidebar width | SQLite state | Machine-specific, changes every session |
| Repo list, tab structure, active tab | SQLite state | Relational data, changes frequently |

Tab structure persists across restarts. PTYs are re-spawned on startup; scrollback is ephemeral (deliberate simplification — serializing xterm.js state is complex and slow).

---

## Rust Backend Design

### File Structure

```
src-tauri/
├── Cargo.toml
├── tauri.conf.json
├── capabilities/
│   └── default.json
├── src/
│   ├── main.rs              -- Tauri bootstrap, register commands, setup hooks
│   ├── state.rs              -- AppState struct, initialization
│   ├── error.rs              -- LanternError enum, Serialize impl
│   ├── commands/
│   │   ├── mod.rs            -- re-exports
│   │   ├── repo.rs           -- repo_add, repo_remove, repo_list, repo_reorder
│   │   ├── terminal.rs       -- terminal_create, terminal_close, terminal_list, etc.
│   │   ├── pty_io.rs         -- terminal_write, terminal_resize, terminal_subscribe
│   │   ├── config.rs         -- config_get, config_update, config_get_path
│   │   └── layout.rs         -- state_save_layout, state_load_layout
│   ├── pty.rs                -- PtyManager, PtySession, spawn/resize/write/cleanup
│   ├── db.rs                 -- SQLite init, migrations, CRUD helpers
│   ├── config.rs             -- UserConfig, TOML load/save, defaults
│   ├── git.rs                -- GitWatcher, git_info(), polling loop
│   ├── agent.rs              -- Agent detection via /proc/PID/cmdline
│   └── paths.rs              -- XDG path helpers (config_dir, data_dir)
```

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum LanternError {
    #[error("Repository not found: {0}")]
    RepoNotFound(String),
    #[error("Repository path does not exist: {0}")]
    PathNotFound(String),
    #[error("Repository already added: {0}")]
    RepoAlreadyExists(String),
    #[error("Terminal session not found: {0}")]
    SessionNotFound(String),
    #[error("PTY error: {0}")]
    Pty(String),
    #[error("Git error: {0}")]
    Git(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(String),
}

impl serde::Serialize for LanternError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
```

### Core Types

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct Repo {
    pub id: String,           // UUID v4
    pub name: String,         // display name (defaults to dir name)
    pub path: String,         // absolute path
    pub sort_order: i32,      // sidebar position
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GitInfo {
    pub branch: Option<String>,  // None if not a git repo
    pub is_dirty: bool,
    pub detached: bool,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TerminalSession {
    pub id: String,           // UUID v4
    pub repo_id: String,
    pub title: String,        // tab label
    pub sort_order: i32,
    pub pid: Option<u32>,     // shell PID, None if exited
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserConfig {
    pub default_shell: String,
    pub font_family: String,
    pub font_size: u16,
    pub theme: String,
    pub scrollback_lines: u32,
    pub git_poll_interval_secs: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LayoutState {
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: u32,
    pub window_height: u32,
    pub window_maximized: bool,
    pub sidebar_width: u32,
    pub active_repo_id: Option<String>,
    pub active_tabs: HashMap<String, String>,  // repo_id -> session_id
}

#[derive(Serialize, Clone)]
pub struct ProcessInfo {
    pub name: String,           // "claude", "codex", "bash", etc.
    pub is_agent: bool,         // true if recognized AI agent
    pub agent_label: Option<String>,  // "Claude Code", "Codex", etc.
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum TerminalOutput {
    Data { bytes: Vec<u8> },
    Exited { code: Option<i32> },
}
```

### Tauri Command API

#### Repository Management

| Command | Parameters | Return | Errors |
|---------|-----------|--------|--------|
| `repo_add` | `path: String` | `Repo` | PathNotFound, RepoAlreadyExists |
| `repo_remove` | `repo_id: String` | `()` | RepoNotFound |
| `repo_list` | — | `Vec<Repo>` | Db |
| `repo_reorder` | `repo_ids: Vec<String>` | `()` | RepoNotFound |
| `repo_get_git_info` | `repo_id: String` | `GitInfo` | RepoNotFound |
| `repo_get_all_git_info` | — | `Vec<(String, GitInfo)>` | Git |

#### Terminal Session Lifecycle

| Command | Parameters | Return | Errors |
|---------|-----------|--------|--------|
| `terminal_create` | `repo_id, title?, shell?` | `TerminalSession` | RepoNotFound, Pty |
| `terminal_close` | `session_id: String` | `()` | SessionNotFound |
| `terminal_list` | `repo_id: String` | `Vec<TerminalSession>` | RepoNotFound |
| `terminal_rename` | `session_id, title` | `()` | SessionNotFound |
| `terminal_reorder` | `repo_id, session_ids` | `()` | RepoNotFound |

#### PTY I/O

| Command | Parameters | Return | Errors |
|---------|-----------|--------|--------|
| `terminal_write` | `session_id, data: Vec<u8>` | `()` | SessionNotFound, Pty |
| `terminal_resize` | `session_id, cols, rows` | `()` | SessionNotFound, Pty |
| `terminal_subscribe` | `session_id, Channel<TerminalOutput>` | `()` | SessionNotFound |
| `terminal_get_foreground_process` | `session_id` | `ProcessInfo` | SessionNotFound |

#### Settings & Layout

| Command | Parameters | Return | Errors |
|---------|-----------|--------|--------|
| `config_get` | — | `UserConfig` | Config |
| `config_update` | `patch: UserConfigPatch` | `UserConfig` | Config |
| `state_save_layout` | `layout: LayoutState` | `()` | Db |
| `state_load_layout` | — | `Option<LayoutState>` | Db |

### PTY Manager

The core runtime component. Owns all PTY sessions and their I/O threads.

```rust
pub struct AppState {
    pub pty_manager: Arc<Mutex<PtyManager>>,
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub config: Arc<RwLock<UserConfig>>,
    pub git_watcher: Arc<GitWatcher>,
}

pub struct PtyManager {
    sessions: HashMap<String, PtySession>,
}

// Each PtySession contains:
// - master: Box<dyn MasterPty + Send>
// - writer: Box<dyn Write + Send>
// - child: Box<dyn Child + Send>
// - reader_handle: JoinHandle<()>  (background thread reading PTY output)
// - shutdown: Arc<AtomicBool>      (signal to stop reader thread)
// - subscriber: Arc<Mutex<Option<Channel<TerminalOutput>>>>
```

**Spawn flow:**
1. `PtySystem::openpty()` with initial size
2. `CommandBuilder::new(shell)` with cwd and `TERM=xterm-256color`
3. `slave.spawn_command(cmd)` → child process
4. `master.take_writer()` → writer handle for input
5. `master.try_clone_reader()` → reader handle for output
6. Start reader thread: read in 4KB chunks, send via Channel

**Output streaming:** Uses Tauri **Channels** (not global events) because:
- Channels are scoped per-invocation (no event-name collision)
- Ordered delivery guaranteed
- Binary data without JSON serialization overhead

**Reader thread:**
```rust
// Reads PTY output in a loop, sends to frontend via Channel
loop {
    if shutdown.load(Ordering::Relaxed) { break; }
    match reader.read(&mut buf) {
        Ok(0) => { /* EOF: process exited */ send(Exited { code }); break; }
        Ok(n) => { send(Data { bytes: buf[..n].to_vec() }); }
        Err(_) => break,
    }
}
```

**Cleanup on tab close:**
1. Set `shutdown` flag on reader thread
2. Kill child process (`child.kill()`)
3. Drop writer (sends EOF to slave)
4. Join reader thread (with timeout)
5. Remove session from HashMap

**Cleanup on app exit:** Iterate all sessions, same cleanup, via Tauri `on_window_event(CloseRequested)`.

**Concurrency:** PtyManager is behind `Arc<Mutex<_>>`. Mutex held only briefly for HashMap lookups/inserts — actual I/O happens via owned handles, not while holding the lock.

### Git Integration

**Library:** `git2` crate (not shelling out). No subprocess overhead per poll, structured output, no dependency on user's git installation.

**Polling, not filesystem watching:**
- A filesystem watcher would fire on every file save, causing excessive recomputation
- 5s poll with `git2` is cheap (reads index + does diff, typically <10ms per repo)
- User can tune interval in config or set to 0 to disable

**Implementation:**
```rust
fn git_info_for_path(path: &str) -> GitInfo {
    // 1. Repository::discover(path) — handles nested dirs, fails gracefully for non-git
    // 2. repo.head() → branch name or detached SHA
    // 3. repo.statuses() with include_untracked → is_dirty
    // 4. graph_ahead_behind(local, upstream) → ahead/behind counts
}
```

**Delivery:** Background thread emits `git-status-update` global event every N seconds with all repos' git info. Single global event is appropriate here (sidebar-wide broadcast, not per-terminal).

### Agent Detection

Detects which AI agent is running in the active terminal's foreground process group.

```rust
fn get_foreground_process(child_pid: u32) -> ProcessInfo {
    // 1. Read /proc/{child_pid}/stat → extract foreground process group ID (field 8: tpgid)
    // 2. Read /proc/{tpgid}/cmdline → get process name
    // 3. Match against known agents:
    //    - "claude" → ProcessInfo { name: "claude", is_agent: true, agent_label: Some("Claude Code") }
    //    - "codex"  → ProcessInfo { name: "codex",  is_agent: true, agent_label: Some("Codex") }
    //    - "aider"  → ProcessInfo { name: "aider",  is_agent: true, agent_label: Some("Aider") }
    //    - "opencode" → ProcessInfo { name: "opencode", is_agent: true, agent_label: Some("OpenCode") }
    //    - anything else → ProcessInfo { name, is_agent: false, agent_label: None }
}
```

Polled every 2s for the **active terminal only** (not all terminals — minimize `/proc` reads).

### Cargo Dependencies

```toml
[dependencies]
tauri = { version = "2", features = ["unstable"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
portable-pty = "0.8"
git2 = "0.19"
rusqlite = { version = "0.31", features = ["bundled"] }
toml = "0.8"
uuid = { version = "1", features = ["v4"] }
thiserror = "2"
dirs = "5"

[dev-dependencies]
tempfile = "3"
```

No tokio runtime needed. PTY reads and git polling are plain `std::thread` (blocking I/O). Tauri v2 commands marked `async` run on Tauri's own async runtime, but our work is synchronous and short-lived.

### main.rs Structure

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let db = db::init()?;
            let config = config::UserConfig::load_or_default()?;
            let pty_manager = pty::PtyManager::new();
            let git_watcher = git::GitWatcher::new(
                config.git_poll_interval_secs,
                app.handle().clone(),
            );
            app.manage(state::AppState { db, config, pty_manager, git_watcher });
            // Start git polling
            let state = app.state::<state::AppState>();
            state.git_watcher.start(state.db.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // all commands registered here
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Save layout state, then clean up PTYs
            }
        })
        .run(tauri::generate_context!())
        .expect("error running Lantern");
}
```

---

## React Frontend Design

### File Structure

```
src/
├── main.tsx                    # createRoot, render <App />
├── App.tsx                     # ShortcutProvider + AppShell
├── components/
│   ├── AppShell.tsx            # CSS grid: sidebar | main
│   ├── AppShell.module.css
│   ├── Sidebar/
│   │   ├── Sidebar.tsx         # Header + RepoList + footer
│   │   ├── Sidebar.module.css
│   │   ├── RepoItem.tsx        # Repo name, branch badge, dirty dot
│   │   ├── RepoItem.module.css
│   │   ├── SidebarResizeHandle.tsx
│   │   └── SidebarResizeHandle.module.css
│   ├── TabBar/
│   │   ├── TabBar.tsx          # Tab strip + new tab button
│   │   ├── TabBar.module.css
│   │   ├── Tab.tsx             # Tab with close + inline rename
│   │   └── Tab.module.css
│   ├── Terminal/
│   │   ├── TerminalViewport.tsx  # Renders ALL terminals, toggles visibility
│   │   ├── TerminalViewport.module.css
│   │   ├── TerminalInstance.tsx  # Mounts one xterm.js via ref
│   │   ├── TerminalInstance.module.css
│   │   ├── EmptyState.tsx      # "Add a repo to get started"
│   │   └── EmptyState.module.css
│   ├── StatusBar/
│   │   ├── StatusBar.tsx       # cwd, shell, dimensions, agent indicator
│   │   └── StatusBar.module.css
│   └── Settings/
│       ├── SettingsDialog.tsx  # Modal for config
│       └── SettingsDialog.module.css
├── stores/
│   └── appStore.ts             # Zustand: repos, tabs, UI state, actions
├── lib/
│   ├── terminalManager.ts      # Singleton managing xterm.js instances + PTY connections
│   ├── tauriCommands.ts        # Typed invoke() wrappers
│   └── theme.ts                # getTerminalTheme() from CSS vars
├── hooks/
│   ├── useShortcuts.ts         # Global keyboard shortcut registration
│   ├── useGitPoller.ts         # Listen for git-status-update events
│   ├── useAgentDetector.ts     # Poll foreground process every 2s
│   └── useSidebarResize.ts     # Pointer drag logic
├── types/
│   └── index.ts                # Repo, TerminalTab, AppSettings, GitInfo, ProcessInfo
└── styles/
    ├── global.css              # CSS reset, base styles
    └── theme.css               # CSS custom properties (design tokens)
```

### Core Types

```typescript
interface Repo {
  id: string;
  path: string;
  name: string;          // last path segment
  branch: string;
  isDirty: boolean;
  detached: boolean;
  ahead: number;
  behind: number;
  tabs: TerminalTab[];
  activeTabId: string | null;
}

interface TerminalTab {
  id: string;
  repoId: string;
  name: string;          // "Terminal 1", user-renameable
  ptyId: string;
  createdAt: number;
}

interface GitInfo {
  branch: string | null;
  isDirty: boolean;
  detached: boolean;
  ahead: number;
  behind: number;
}

interface ProcessInfo {
  name: string;
  isAgent: boolean;
  agentLabel: string | null;
}

interface AppSettings {
  fontFamily: string;
  fontSize: number;
  theme: string;
  defaultShell: string;
  scrollbackLines: number;
  gitPollIntervalSecs: number;
}
```

### Component Architecture

```
<App>
  <ShortcutProvider>
    <AppShell>
      <Sidebar>
        <SidebarHeader />           // "Lantern" branding + collapse btn
        <RepoList>
          <RepoItem />              // per repo: name, branch, dirty dot
        </RepoList>
        <SidebarFooter />           // + Add repo, settings button
      </Sidebar>
      <SidebarResizeHandle />       // drag handle, 4px hit area
      <MainPanel>
        <TabBar>
          <Tab />                   // per terminal tab
          <NewTabButton />
        </TabBar>
        <TerminalViewport>
          <TerminalInstance />      // xterm.js mount (visibility toggled)
        </TerminalViewport>
        <StatusBar />               // cwd, shell, dimensions, agent
      </MainPanel>
    </AppShell>
    <SettingsDialog />              // modal overlay
  </ShortcutProvider>
</App>
```

### Zustand Store

Single store, sliced logically. No multiple stores.

```typescript
interface AppState {
  // Repo slice
  repos: Repo[];
  activeRepoId: string | null;

  // UI slice
  sidebarWidth: number;
  sidebarCollapsed: boolean;
  settingsOpen: boolean;

  // Actions
  addRepo: (path: string) => Promise<void>;
  removeRepo: (id: string) => Promise<void>;
  setActiveRepo: (id: string) => void;
  updateRepoGitStatus: (id: string, info: GitInfo) => void;
  addTab: (repoId: string) => Promise<void>;
  closeTab: (repoId: string, tabId: string) => Promise<void>;
  setActiveTab: (repoId: string, tabId: string) => void;
  renameTab: (repoId: string, tabId: string, name: string) => void;
  setSidebarWidth: (width: number) => void;
  toggleSidebar: () => void;
  toggleSettings: () => void;
  hydrate: () => Promise<void>;
}
```

**Middleware:** `immer` for mutable-style nested updates + `persist` for UI prefs only (repos/tabs come from SQLite).

**Action pattern:** Call `invoke()` first, update local state on success. No optimistic updates for destructive actions.

### TerminalManager (Critical Component)

Singleton class **outside React** — xterm.js instances are DOM-bound, can't live in React state.

```typescript
class TerminalManager {
  private terminals = new Map<string, ManagedTerminal>();
  private resizeObserver: ResizeObserver;

  async create(tabId: string, repoPath: string, container: HTMLDivElement): Promise<void> {
    // 1. new Terminal({ cursorBlink, fontFamily, fontSize, theme })
    // 2. loadAddon(FitAddon), loadAddon(SearchAddon), loadAddon(WebLinksAddon)
    // 3. terminal.open(container)
    // 4. try { loadAddon(WebglAddon) } catch { /* canvas fallback */ }
    // 5. fitAddon.fit()
    // 6. Create Channel<Uint8Array> for PTY output
    // 7. invoke('terminal_subscribe', { sessionId, onOutput: channel })
    // 8. terminal.onData → invoke('terminal_write')
    // 9. terminal.onResize → invoke('terminal_resize')
    // 10. resizeObserver.observe(container)
  }

  destroy(tabId: string): void { /* cleanup everything */ }
  fit(tabId: string): void { /* fitAddon.fit() */ }
  focus(tabId: string): void { /* terminal.focus() */ }
}

export const terminalManager = new TerminalManager();
```

### Terminal Persistence Across Switches

**All terminals rendered at once, visibility toggled via `display: none`:**

```tsx
// TerminalViewport renders terminals for ALL repos
{repos.flatMap(repo =>
  repo.tabs.map(tab => (
    <TerminalInstance
      key={tab.id}
      tabId={tab.id}
      isVisible={repo.id === activeRepoId && tab.id === repo.activeTabId}
    />
  ))
)}
```

xterm.js instances stay alive when switching repos/tabs. Only the visible one gets `display: block`. On becoming visible: `requestAnimationFrame` → `fit()` → `focus()`.

For typical usage (5-15 terminals), DOM overhead is negligible. WebGL context limit (~16) handled by falling back to canvas renderer.

### Typed Backend Interface

```typescript
// lib/tauriCommands.ts
export const commands = {
  // Repos
  repoList: () => invoke<Repo[]>('repo_list'),
  repoAdd: (path: string) => invoke<Repo>('repo_add', { path }),
  repoRemove: (id: string) => invoke<void>('repo_remove', { repoId: id }),
  repoGetAllGitInfo: () => invoke<[string, GitInfo][]>('repo_get_all_git_info'),

  // Terminals
  terminalCreate: (repoId: string) => invoke<TerminalSession>('terminal_create', { repoId }),
  terminalClose: (sessionId: string) => invoke<void>('terminal_close', { sessionId }),
  terminalList: (repoId: string) => invoke<TerminalSession[]>('terminal_list', { repoId }),
  terminalRename: (sessionId: string, title: string) => invoke<void>('terminal_rename', { sessionId, title }),
  terminalWrite: (sessionId: string, data: Uint8Array) => invoke<void>('terminal_write', { sessionId, data }),
  terminalResize: (sessionId: string, cols: number, rows: number) => invoke<void>('terminal_resize', { sessionId, cols, rows }),
  terminalGetForegroundProcess: (sessionId: string) => invoke<ProcessInfo>('terminal_get_foreground_process', { sessionId }),

  // Config & Layout
  configGet: () => invoke<AppSettings>('config_get'),
  configUpdate: (patch: Partial<AppSettings>) => invoke<AppSettings>('config_update', { patch }),
  stateSaveLayout: (layout: LayoutState) => invoke<void>('state_save_layout', { layout }),
  stateLoadLayout: () => invoke<LayoutState | null>('state_load_layout'),
} as const;
```

### Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| New tab | `Ctrl+T` |
| Close tab | `Ctrl+W` |
| Next tab | `Ctrl+Tab` |
| Previous tab | `Ctrl+Shift+Tab` |
| Switch to repo N | `Ctrl+1` through `Ctrl+9` |
| Toggle sidebar | `Ctrl+B` |
| Focus terminal | `Escape` |
| Settings | `Ctrl+,` |
| Search in terminal | `Ctrl+Shift+F` |
| Rename tab | `F2` |

Implemented via a single `keydown` listener in `ShortcutProvider`. No per-component listeners.

### Layout (CSS Grid)

```
+----------------------------------------------------------------------+
|  Lantern                                                    [_][O][X] |
+-------------------+--------------------------------------------------+
|                   |  [Terminal 1] [Terminal 2] [+]                    |
|   REPOSITORIES    +--------------------------------------------------+
|                   |                                                  |
|  > my-app     (3) |  $ git status                                    |
|    main · clean   |  On branch main                                  |
|                   |  Changes not staged for commit:                  |
|    api-server (2) |    modified: src/App.tsx                          |
|    feat/auth · 3  |                                                  |
|                   |  $ npm test                                      |
|    dotfiles   (1) |  PASS src/__tests__/App.test.tsx                  |
|    master         |                                                  |
|                   |                                                  |
|                   |                                                  |
|  [+ Add Repo] [⚙] |                                                  |
+-------------------+--------------------------------------------------+
|  ~/projects/my-app  |  zsh  |  142x38  |  ⚡ Claude Code              |
+----------------------------------------------------------------------+
```

```css
.appShell {
  display: grid;
  grid-template-columns: var(--sidebar-width) 1fr;
  grid-template-rows: 1fr auto;
  height: 100vh;
  overflow: hidden;
}
```

---

## Design System

Dark theme with green accent, inspired by Conductor's aesthetic.

### CSS Custom Properties

```css
:root {
  /* Surfaces */
  --bg-primary: #0a0f0d;              /* main background */
  --bg-secondary: #111916;            /* sidebar */
  --bg-tertiary: #182420;             /* hover states, active tab */
  --bg-elevated: #1a2b24;             /* dialogs, tooltips */

  /* Borders */
  --border: #1e3a2f;
  --border-focus: #2d6b4f;

  /* Text */
  --text-primary: #d4e8dc;            /* main text */
  --text-secondary: #7a9b8a;          /* dimmed labels */
  --text-tertiary: #4a6b5a;           /* disabled, hints */

  /* Accent */
  --accent: #3ecf8e;                  /* bright green */
  --accent-dim: #2a8f62;
  --accent-bg: rgba(62, 207, 142, 0.1);

  /* Status */
  --status-clean: #3ecf8e;            /* git clean */
  --status-dirty: #f0c674;            /* git dirty (amber) */
  --status-error: #e06c75;            /* errors */

  /* Terminal ANSI colors */
  --ansi-black: #1a1a2e;
  --ansi-red: #e06c75;
  --ansi-green: #3ecf8e;
  --ansi-yellow: #f0c674;
  --ansi-blue: #61afef;
  --ansi-magenta: #c678dd;
  --ansi-cyan: #56b6c2;
  --ansi-white: #d4e8dc;
  --ansi-bright-black: #4a6b5a;
  --ansi-bright-red: #e88892;
  --ansi-bright-green: #5ae0a8;
  --ansi-bright-yellow: #f5d99a;
  --ansi-bright-blue: #7ec4f8;
  --ansi-bright-magenta: #d898e8;
  --ansi-bright-cyan: #73cfdb;
  --ansi-bright-white: #f0f8f4;

  /* Typography */
  --font-ui: -apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', 'SF Mono', monospace;
  --font-size-xs: 11px;
  --font-size-sm: 12px;
  --font-size-base: 13px;
  --font-size-lg: 14px;

  /* Spacing (4px base) */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;

  /* Radius */
  --radius-sm: 4px;
  --radius-md: 6px;
  --radius-lg: 8px;

  /* Layout */
  --sidebar-width: 260px;
  --sidebar-min-width: 200px;
  --sidebar-max-width: 400px;
  --tab-height: 36px;
  --statusbar-height: 24px;

  /* Transitions */
  --transition-fast: 100ms ease;
  --transition-normal: 150ms ease;
}
```

### Component Style Examples

**Sidebar repo item:**
```css
.repoItem {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
  color: var(--text-secondary);
}
.repoItem:hover { background: var(--bg-tertiary); }
.repoItem[data-active="true"] {
  background: var(--accent-bg);
  color: var(--text-primary);
}
```

**Tab:**
```css
.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  height: var(--tab-height);
  color: var(--text-secondary);
  border-bottom: 2px solid transparent;
  cursor: pointer;
}
.tab[data-active="true"] {
  color: var(--text-primary);
  border-bottom-color: var(--accent);
}
```

---

## Additional V1 Features

- **Copy/paste**: Wire xterm.js clipboard to Tauri's clipboard API (Ctrl+Shift+C / Ctrl+Shift+V)
- **Clickable URLs**: xterm.js `WebLinksAddon` — opens links in default browser via Tauri shell API
- **Process exit handling**: PTY reader gets EOF → emit `Exited { code }` → show "Process exited (code N)" overlay with restart button
- **Shell detection**: Read `/etc/shells` on startup, let user pick default or per-repo override in settings
- **Unicode support**: xterm.js `Unicode11Addon` for proper CJK/emoji rendering
- **Agent detection**: Detect running AI agent via `/proc/PID/cmdline`, show in status bar (Claude Code, Codex, Aider, OpenCode)
- **Auth inheritance**: Terminals are real PTY sessions inheriting the user's full env. No re-login per tab.

---

## Testing Strategy

### Rust Backend Tests (`cargo test`)

Each phase has corresponding tests. All tests use `tempfile::tempdir()` for isolation — no state leaks between tests.

**Phase 2 — PTY tests** (`src-tauri/src/pty.rs`):
```
test_spawn_pty_returns_pid          — spawn bash, verify PID > 0
test_write_and_read_pty             — write "echo hello\n", read output contains "hello"
test_resize_pty                     — resize to 40x10, verify no error
test_close_pty_kills_process        — close session, verify child process exited
test_spawn_with_cwd                 — spawn in /tmp, run "pwd", output contains "/tmp"
test_spawn_nonexistent_shell_errors — spawn "/bin/doesnotexist", verify LanternError::Pty
test_multiple_concurrent_sessions   — spawn 5 PTYs, write to all, verify independent output
```

**Phase 3 — Database tests** (`src-tauri/src/db.rs`):
```
test_init_creates_tables            — init DB, verify all tables exist
test_add_repo                       — add repo, verify returned Repo has correct fields
test_add_duplicate_repo_errors      — add same path twice, verify RepoAlreadyExists
test_remove_repo_cascades_sessions  — add repo + sessions, remove repo, sessions gone
test_list_repos_ordered             — add 3 repos, verify sort_order respected
test_reorder_repos                  — reorder, verify new order persisted
test_repo_path_not_found            — add nonexistent path, verify PathNotFound error
```

**Phase 3 — Config tests** (`src-tauri/src/config.rs`):
```
test_load_default_config            — no file exists, get defaults
test_save_and_reload                — save config, reload, verify identical
test_partial_update                 — update font_size only, other fields unchanged
test_invalid_toml_returns_error     — write garbage to file, verify Config error
```

**Phase 4 — Terminal session DB tests** (`src-tauri/src/db.rs`):
```
test_create_session                 — create session for repo, verify fields
test_list_sessions_for_repo         — create 3 sessions, list returns all 3 ordered
test_close_session_removes_from_db  — create + close, verify gone
test_rename_session                 — rename, verify persisted
test_active_tab_persists            — set active tab, reload, verify same
```

**Phase 5 — Git tests** (`src-tauri/src/git.rs`):
```
test_git_info_clean_repo            — init repo, commit file, verify is_dirty=false
test_git_info_dirty_repo            — modify tracked file, verify is_dirty=true
test_git_info_branch_name           — create branch, verify branch name correct
test_git_info_detached_head         — checkout commit, verify detached=true
test_git_info_non_git_dir           — point to non-git dir, verify branch=None
test_git_info_deleted_dir           — point to nonexistent dir, verify no panic
test_git_info_ahead_behind          — create local+remote, diverge, verify counts
```

**Phase 6 — Layout & agent tests:**
```
test_save_and_load_layout           — save layout, load, verify all fields match
test_load_layout_no_existing        — load without save, verify returns None
test_layout_survives_repo_delete    — delete repo, layout still loads
test_agent_detection_bash           — bash process → is_agent=false
test_agent_detection_unknown_proc   — unknown proc → is_agent=false, name returned
```

### Frontend Tests (`npm test` via Vitest + React Testing Library)

**Component tests:**
```
Sidebar.test.tsx:
  renders repo list from store
  highlights active repo
  calls setActiveRepo on click
  shows "Add repository" button

RepoItem.test.tsx:
  displays repo name (last path segment)
  displays branch name
  shows dirty dot when isDirty=true
  hides dirty dot when isDirty=false

TabBar.test.tsx:
  renders tabs for active repo
  highlights active tab
  calls addTab on "+" click
  calls closeTab on X click
  shows no tabs when repo has none

Tab.test.tsx:
  displays tab name
  enters rename mode on double-click
  saves rename on Enter
  cancels rename on Escape
  shows close button on hover

StatusBar.test.tsx:
  displays current working directory
  displays shell name
  displays terminal dimensions
  displays agent name when agent is running
  shows no agent indicator when plain shell

SettingsDialog.test.tsx:
  renders current settings values
  updates font size
  saves on confirm
  reverts on cancel
```

**Store tests (mocking Tauri invoke):**
```
appStore.test.ts:
  hydrate populates repos from backend
  addRepo calls invoke and updates state
  removeRepo calls invoke and removes from state
  setActiveRepo updates activeRepoId
  addTab creates tab in correct repo
  closeTab removes tab and selects next
  closeTab on last tab leaves activeTabId null
  updateGitStatus updates branch and isDirty
  toggleSidebar flips collapsed state
```

**Integration tests:**
```
terminalManager.test.ts:
  create initializes xterm Terminal (mock DOM)
  destroy disposes terminal and listeners
  fit calls fitAddon.fit
  multiple terminals can coexist
```

### E2E Smoke Tests

Full-stack validation (manual or Playwright):
```
 1. App launches without errors
 2. Add repo via file picker → appears in sidebar
 3. First repo auto-creates first terminal tab
 4. Type "echo test" → output appears
 5. Create second tab → independent shell
 6. Switch tabs → previous tab output preserved
 7. Add second repo → switch between repos
 8. Close tab → terminal killed, tab removed
 9. Remove repo → all its tabs gone
10. Restart app → repos and tab structure restored
11. Resize window → terminal refits
12. Keyboard shortcuts work (Ctrl+T, Ctrl+W, Ctrl+B)
13. Agent indicator appears when running claude/codex in terminal
```

---

## Edge Cases

| Scenario | Handling |
|----------|----------|
| Repo path deleted/moved while app open | Git poller detects missing dir → error badge on repo, disable "new tab" |
| Shell process exits (`exit`) | PTY reader gets EOF → emit `Exited { code }` → overlay with restart button |
| Shell process crashes (SIGKILL) | Same as exit but code=None → "Process terminated" overlay |
| WebGL context limit (>16 terminals) | `WebglAddon` throws → catch, fall back to canvas renderer |
| Very large output (`cat /dev/urandom`) | xterm.js caps at `scrollback_lines` config value |
| Non-git repo added | `git2::Repository::discover()` fails → `branch=None`, no indicators |
| SQLite DB corrupted | Backup corrupted file, create fresh DB, log warning |
| Invalid TOML config | Parse error → fall back to defaults, log warning |
| Rapid tab switching | `display: none` toggle, `requestAnimationFrame` debounces fit/focus |
| Window minimized → restored | ResizeObserver fires → terminals refit |
| Multiple Lantern instances | SQLite write lock contention → "Already running" dialog |
| System shell not found | Detect from `/etc/shells`, fall back to `/bin/sh` |

---

## Implementation Order

### Phase 1: Scaffold + Hello World
1. `cargo create-tauri-app lantern` with React + TypeScript template
2. Set up project structure (Rust src dirs, React src dirs)
3. Add Cargo dependencies (portable-pty, git2, rusqlite, toml, uuid, thiserror, dirs, tempfile)
4. Add npm dependencies (@xterm/xterm, @xterm/addon-fit, @xterm/addon-webgl, @xterm/addon-search, @xterm/addon-web-links, zustand, immer)
5. Add dev dependencies (vitest, @testing-library/react, jsdom)
6. Verify `cargo tauri dev` opens an empty window
7. **Validate:** `cargo build` succeeds, `npm run build` succeeds, `cargo tauri dev` shows window

### Phase 2: PTY + Single Terminal
8. Implement `error.rs` — LanternError enum
9. Implement `pty.rs` — PtyManager with spawn, write, resize, subscribe, shutdown
10. Implement `commands/pty_io.rs` — terminal_write, terminal_resize, terminal_subscribe
11. Implement `commands/terminal.rs` — terminal_create (hardcoded cwd for now)
12. Build `lib/terminalManager.ts` — xterm.js + FitAddon + WebglAddon + Channel wiring
13. Build `components/Terminal/TerminalInstance.tsx` — mount xterm.js via ref
14. Render a single hardcoded terminal that connects to a PTY
15. **Validate:** `cargo test` passes PTY tests, typing in terminal works, output renders

### Phase 3: Database + Repo Management
16. Implement `paths.rs` — XDG directory helpers
17. Implement `db.rs` — SQLite init, schema creation, CRUD for repos
18. Implement `config.rs` — UserConfig struct, TOML load/save/defaults
19. Implement `state.rs` — AppState struct wiring everything together
20. Implement `commands/repo.rs` — repo_add, repo_remove, repo_list, repo_reorder
21. Implement `commands/config.rs` — config_get, config_update
22. Build `types/index.ts` — all TypeScript types
23. Build `lib/tauriCommands.ts` — typed invoke() wrappers
24. Build `stores/appStore.ts` — Zustand store with hydrate(), repo actions
25. Build `components/Sidebar/` — Sidebar, RepoItem, SidebarResizeHandle
26. Wire "Add Repository" button to Tauri file picker dialog
27. **Validate:** `cargo test` passes DB + config tests, `npm test` passes store + sidebar tests, repos persist across restart

### Phase 4: Multi-Terminal Tabs
28. Add terminal session CRUD to `db.rs`
29. Implement remaining `commands/terminal.rs` — terminal_list, terminal_close, terminal_rename, terminal_reorder
30. Build `components/TabBar/` — TabBar, Tab (with close, inline rename)
31. Build `components/Terminal/TerminalViewport.tsx` — render ALL terminals, toggle visibility
32. Build `components/Terminal/EmptyState.tsx`
33. Wire tab lifecycle: create (spawns PTY in repo cwd), close (kills PTY), switch, rename
34. Persist tab structure in SQLite, restore on startup (re-spawn PTYs)
35. **Validate:** `cargo test` passes session DB tests, `npm test` passes TabBar/Tab tests, multiple tabs work, state survives restart

### Phase 5: Git Integration
36. Implement `git.rs` — git_info_for_path() using git2, GitWatcher polling thread
37. Emit `git-status-update` global event from Rust every N seconds
38. Build `hooks/useGitPoller.ts` — listen for events, update store
39. Update RepoItem to display branch badge + dirty dot
40. **Validate:** `cargo test` passes git tests, sidebar updates live

### Phase 6: Layout, Config, Polish
41. Implement sidebar resize handle with `hooks/useSidebarResize.ts`
42. Implement `commands/layout.rs` — state_save_layout, state_load_layout
43. Save/restore window geometry + sidebar width + active repo/tab on exit/start
44. Build `components/StatusBar/StatusBar.tsx` — cwd, shell, dimensions, agent indicator
45. Implement `agent.rs` + `terminal_get_foreground_process` command
46. Build `hooks/useAgentDetector.ts` — poll active terminal every 2s
47. Build `hooks/useShortcuts.ts` + wire all keyboard shortcuts
48. Build `components/Settings/SettingsDialog.tsx`
49. Apply full design system: `styles/theme.css` + `styles/global.css` + all CSS modules
50. Add `WebLinksAddon` for clickable URLs
51. Wire clipboard (Ctrl+Shift+C/V) via Tauri clipboard API
52. Add `Unicode11Addon` for CJK/emoji support
53. **Validate:** all tests pass, layout persists, shortcuts work, agent indicator shows

### Phase 7: Edge Cases + Hardening
54. Handle repo path deleted/moved → error badge in sidebar
55. Handle shell exit → "Process exited" overlay with restart button
56. Handle shell crash → "Process terminated" overlay
57. WebGL context limit fallback (>16 terminals → canvas)
58. Graceful shutdown: save state → kill all PTYs → exit
59. SQLite corruption recovery: backup + fresh DB
60. Multiple instance detection
61. Shell detection from `/etc/shells` with `/bin/sh` fallback
62. **Validate:** all `cargo test` + `npm test` pass, E2E smoke tests pass

---

## Verification Checklist

1. `cargo tauri dev` — app opens, no panics
2. Add 2-3 repos via sidebar → appear with correct names
3. Create multiple tabs per repo → each has independent shell
4. Type commands → output renders correctly
5. Switch between repos → terminals preserved (no scrollback loss)
6. Switch between tabs → same preservation
7. Kill app, relaunch → repos and tab structure restored
8. Resize window → terminals refit
9. Resize sidebar → terminals refit
10. Git branch/dirty indicators update live
11. All keyboard shortcuts work
12. Settings dialog saves to TOML
13. Agent indicator shows when running claude/codex
14. Process exit overlay appears with restart option
15. Clickable URLs open in browser
