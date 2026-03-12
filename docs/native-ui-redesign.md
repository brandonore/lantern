# Native UI Redesign: GNOME Prompt-Inspired

## Context

The native Linux rewrite (`apps/lantern-native-linux/`) replaced Tauri/xterm.js with a 100% Rust app using GTK4/libadwaita/VTE. Two critical issues exist:

1. **Nothing works when clicked** — `Rc<NativeApp>` dropped immediately after window presentation, making all weak-reference signal handlers dead
2. **UI is rough** — 12 text-label buttons in header, wrong widget types, excessive margins, no icons

Target aesthetic: GNOME Prompt terminal with Supacode-style tab placement.

## Critical Bug: Rc Lifetime

### Root Cause

In `app.rs:15-28`, the `Rc<NativeApp>` is created inside `connect_activate` and dropped when the closure exits. Every signal handler uses `Rc::downgrade(self)` → `weak_self.upgrade()` which returns `None` after the strong reference is gone.

### Fix

Hold the `Rc<NativeApp>` in the outer scope of `run()`:

```rust
pub fn run() {
    adw::init().expect("failed to initialize libadwaita");
    let app = adw::Application::builder()
        .application_id("sh.lantern.NativeLinux")
        .build();

    let held: Rc<RefCell<Option<Rc<NativeApp>>>> = Rc::new(RefCell::new(None));
    let held_for_activate = held.clone();
    app.connect_activate(move |app| {
        let native_app = NativeApp::bootstrap(app);
        native_app.present();
        held_for_activate.replace(Some(native_app));
    });

    app.run();
    drop(held);
}
```

### Edge Cases

- **Re-activation**: GTK may call `connect_activate` again if a second instance attempts to launch. The `replace()` call handles this — old NativeApp is dropped, new one takes over.
- **Cleanup ordering**: `app.run()` blocks until quit. `held` drops after the event loop exits, so NativeApp outlives all GTK signal dispatch.

## Header Bar Redesign

### Layout

```
[sidebar-toggle] [add-repo] [new-tab]    "Lantern"    [search] [menu-button]
```

### Icon Buttons (visible in header)

| Action | Icon | Tooltip |
|--------|------|---------|
| Toggle sidebar | `sidebar-show-symbolic` | "Toggle Sidebar" |
| Add repo | `folder-new-symbolic` | "Add Repository" |
| New tab | `tab-new-symbolic` | "New Tab" |
| Search | `system-search-symbolic` | "Find in Terminal" |
| Menu | `open-menu-symbolic` | Primary menu |

All created via `gtk::Button::from_icon_name(...)` with tooltips. No text labels.

### Menu Popover (via `gio::Menu` + `gio::SimpleAction`)

```
┌──────────────────┐
│ Split Right       │  app.split-right
│ Split Down        │  app.split-down
│ Close Split       │  app.close-split
│ Next Pane         │  app.next-pane
│ Flip Split        │  app.flip-split
├──────────────────┤
│ Move Tab Left     │  app.move-tab-left
│ Move Tab Right    │  app.move-tab-right
│ Close Tab         │  app.close-tab
│ Remove Repository │  app.remove-repo
├──────────────────┤
│ Settings          │  app.settings
└──────────────────┘
```

Actions registered on `self.window` in `wire_events()`. Enable/disable state managed in `rebuild_tabs()` and `populate_sidebar()` via `action.set_enabled()`.

### Edge Cases

- **Action sensitivity sync**: When no repo is active, split/tab/remove actions must be disabled. When no session exists, close-tab and search must be disabled. All handled in `rebuild_tabs()`.
- **Menu button focus**: After selecting a menu item, focus must return to the active terminal. Use `grab_focus()` on the VTE widget after action dispatch.
- **Keyboard shortcuts unchanged**: All existing Ctrl+T/W/B/F shortcuts remain wired via the EventControllerKey. Menu items are secondary access paths.

## Tab Bar: adw::TabBar + adw::TabView

### Architecture

Replace the manual `gtk::Box` + `gtk::ToggleButton` tab bar with `adw::TabBar` + `adw::TabView`:

- `adw::TabView` replaces `gtk::Stack` as the terminal content container
- Each `adw::TabPage` holds a terminal layout widget (single VTE or split `gtk::Paned` tree)
- `adw::TabBar` renders the tab strip, positioned below the header bar
- Tab selection, reorder, and close are handled by `adw::TabView` signals

### Widget Tree Change

```
Before:
  content_box
    ├── tab_bar (gtk::Box with ToggleButtons)
    ├── search_revealer
    ├── terminal_stack (gtk::Stack)
    └── status_label

After:
  content_box
    ├── adw::TabBar (connected to TabView)
    ├── search_revealer
    ├── adw::TabView (contains terminal pages)
    └── status_label
```

### Session ↔ TabPage Mapping

- When a session is created (`create_session_for_repo`), append a new `TabPage` to the `TabView`
- Store session_id on each page via `page.set_data("session-id", ...)` or maintain a `HashMap<session_id, adw::TabPage>`
- When switching repos, clear all pages and rebuild for the new repo's sessions
- When closing a session, remove its page from the TabView

### Signal Wiring

- `tab_view.connect_selected_page_notify` → update workspace active session, refresh chrome
- `tab_view.connect_close_page` → handle session close (DB delete, surface cleanup, split state update). Return `true` to confirm close.
- `tab_view.connect_page_reordered` → update session sort_order in workspace and DB
- `tab_view.connect_create_window` → return `None` (no tab tear-off support)

### Rebuilding on Repo Switch

When `select_repo_by_id()` is called:
1. Disconnect `selected_page_notify` temporarily (prevent cascading updates)
2. Remove all existing pages from `TabView`
3. For each session in the new repo, create a page with its terminal layout
4. Select the page matching the repo's `active_session_id`
5. Reconnect `selected_page_notify`

### Edge Cases

- **Empty repo (no sessions)**: TabView has zero pages. Show empty state overlay. `adw::TabBar` handles empty state gracefully.
- **Rapid repo switching**: Disconnecting/reconnecting the `selected_page_notify` signal prevents spurious state updates during page teardown/rebuild.
- **Split terminals inside tabs**: Each TabPage's child is either a single VTE widget or a `gtk::Paned` tree. The existing `build_terminal_layout()` / `build_split_widget()` functions return a `gtk::Widget` that becomes the page content.
- **Tab title updates**: Use `page.set_title()` when VTE title changes or user renames. `adw::TabBar` reflects this automatically.
- **Tab reorder persistence**: On `page_reordered`, extract new ordering from TabView and call `db::reorder_sessions()`.
- **Double-click rename**: `adw::TabBar` doesn't natively support double-click rename. Wire a `GestureClick` on the TabBar widget, map click coordinates to the underlying page, and show the rename dialog.
- **Performance on repo switch**: Remove pages in bulk before adding new ones. Avoid per-page signal emissions during teardown by disconnecting first.

## Sidebar: gtk::ListBox Migration

### Architecture

Replace `gtk::Box` (with `navigation-sidebar` class that has no effect on Box) with `gtk::ListBox`:

```rust
let sidebar_list = gtk::ListBox::new();
sidebar_list.set_selection_mode(gtk::SelectionMode::None); // we manage selection manually
sidebar_list.add_css_class("navigation-sidebar");
```

Using `SelectionMode::None` because we have mixed row types (group headers + repo rows) and need custom selection behavior per repo switch.

### Row Types

**Group header row** (non-activatable):
```
[▾ supacode]                    [↑] [↓]
```
- `gtk::ListBoxRow` with `set_activatable(false)` and `set_selectable(false)`
- Uppercase label + collapse icon
- Move up/down icon buttons (`go-up-symbolic`, `go-down-symbolic`)

**Repo row** (activatable):
```
  main                           [🗑]
  main • clean
```
- `gtk::ListBoxRow` with custom child Box
- Repo name label (bold for active)
- Git meta subtitle (branch, dirty, ahead/behind)
- Remove button (`user-trash-symbolic`, flat, only visible on hover via CSS)
- Active repo gets `suggested-action` or accent background via CSS class

### Selection Handling

- `sidebar_list.connect_row_activated` → `select_repo_by_id()`
- After selecting, iterate rows and update CSS classes (remove `active` from old, add to new)
- Active repo row highlighted with accent color

### "Add Repository" Button

Positioned at the bottom of the sidebar, outside the ListBox, in a fixed footer area:
```rust
let add_button = gtk::Button::with_label("+ Add Repository");
add_button.add_css_class("flat");
// ... wire to prompt_add_repo()
sidebar_box.append(&sidebar_scroll); // ListBox in scrolled window
sidebar_box.append(&add_repo_footer); // Fixed at bottom
```

### Edge Cases

- **Empty state**: When no repos exist, show a centered label "Add a repository to get started" inside the sidebar.
- **Group collapse**: Toggling collapse hides/shows repo rows under the group header. Use `row.set_visible(false)` on child rows rather than removing/re-adding them.
- **Worktree groups**: Default repo within a group shows a small "default" badge. Worktree groups have slightly indented repo rows.
- **Long repo names**: Sidebar has `min_content_width(200)` on ScrolledWindow. Repo name labels use `set_ellipsize(pango::EllipsizeMode::End)`.
- **Git status updates**: When `refresh_git_statuses()` detects changes, update only the affected row's subtitle label rather than rebuilding the entire sidebar. This avoids flicker and preserves scroll position.

## Content Area: Edge-to-Edge Terminal

### Margins

```rust
let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
// No margins — terminal fills all space
```

- Tab bar: 0 external margin (adw::TabBar handles its own padding)
- Search revealer: small internal padding (4px horizontal)
- Terminal: zero margin, fills all remaining space
- Status bar: 3px vertical, 8px horizontal padding

### Status Bar

Thin bar at the bottom of the content area:
```
[main • dirty • 2 ahead]                    [Claude Code]
```
- Left: git info for active repo
- Right: agent detection label (Claude Code, Codex, Aider, etc.)
- Uses `dim-label` CSS class, small font size

## Struct Changes

### Fields Removed from NativeApp

```
remove_repo_button, close_tab_button, move_tab_left_button,
move_tab_right_button, split_right_button, split_down_button,
close_split_button, next_pane_button, flip_split_button,
settings_button, tab_bar, terminal_stack, empty_label
```

### Fields Added to NativeApp

```
tab_view: adw::TabView,
tab_bar_widget: adw::TabBar,
menu_button: gtk::MenuButton,
sidebar_list: gtk::ListBox,
// Action references for enable/disable
action_split_right: gio::SimpleAction,
action_split_down: gio::SimpleAction,
action_close_split: gio::SimpleAction,
action_next_pane: gio::SimpleAction,
action_flip_split: gio::SimpleAction,
action_move_tab_left: gio::SimpleAction,
action_move_tab_right: gio::SimpleAction,
action_close_tab: gio::SimpleAction,
action_remove_repo: gio::SimpleAction,
action_settings: gio::SimpleAction,
```

### Methods Significantly Changed

- `bootstrap()` — new widget construction, action registration
- `wire_events()` — action handlers instead of button handlers, TabView signals
- `populate_sidebar()` — ListBox rows instead of Box children
- `rebuild_tabs()` — replaced by TabView page management + action sensitivity updates
- `show_active_terminal()` — simplified, TabView manages visible page
- `select_repo_by_id()` — rebuilds TabView pages for new repo
- `select_session()` — selects TabView page instead of rebuilding buttons
- `create_tab()` — appends TabView page
- `close_tab()` — removes TabView page

## Performance Considerations

- **Sidebar rebuild**: Prefer updating existing rows over full clear+rebuild on git status refresh. Only full-rebuild on repo add/remove/reorder.
- **Repo switch**: Disconnect TabView signals during page teardown/rebuild to avoid O(n) intermediate signal emissions.
- **VTE surface caching**: Terminal surfaces are already cached in `VteTerminalHost`. Switching repos detaches surfaces from pages but keeps them alive for fast re-attach.
- **Layout persistence debounce**: Keep existing 200ms debounce on window resize/move/split resize.
- **Git polling**: Keep existing configurable interval with `glib::timeout_add_seconds_local`. No change needed.
- **Signal handler cleanup**: All handlers use `Rc::downgrade()` pattern. When NativeApp is dropped, handlers naturally become no-ops.

## Search Bar

Replace text buttons with icon buttons:
- Previous: `go-up-symbolic`
- Next: `go-down-symbolic`
- Close: `window-close-symbolic`

Search integration with `adw::TabView`: search operates on the active page's VTE terminal(s). No change to search logic, just button styling.

## Files Modified

- `apps/lantern-native-linux/src/app.rs` — all changes (struct, bootstrap, wire_events, sidebar, tabs, terminal display)
- `apps/lantern-native-linux/Cargo.toml` — may need `gio` feature flag check (should already be available via gtk4 dependency)

## Verification

1. `cargo check -p lantern-native-linux`
2. `cargo build -p lantern-native-linux`
3. `cargo test -p lantern-core` (shared core unchanged)
4. Manual testing:
   - Launch app, verify window appears with clean UI
   - Add repo via sidebar button, verify terminal spawns
   - Switch repos in sidebar, verify tab bar updates
   - Create/close/reorder tabs
   - Split terminals (via menu), verify pane rendering
   - Search (Ctrl+F), verify prev/next/close icons
   - Settings (via menu), verify dialog opens and saves
   - Keyboard shortcuts: Ctrl+T/W/B/F/1-9, verify all work
   - Window resize/maximize, verify layout persistence
   - Restart app, verify state restored
