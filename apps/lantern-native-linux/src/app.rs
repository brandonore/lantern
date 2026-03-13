use crate::terminal_host::{TerminalHost, VteTerminalHost};
use crate::theme::{
    native_theme_options, normalized_native_theme_id, sidebar_theme_css, theme_color_scheme,
    theme_is_dark,
};
use adw::prelude::*;
use gtk::gio;
use lantern_core::{
    db, git, AppLayout, DbConn, LanternError, NativeSplitOrientation, NativeSplitState,
    RepoWorkspace, UserConfig, WorkspaceState,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use uuid::Uuid;
use vte::prelude::TerminalExt;

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

#[allow(dead_code)]
struct NativeApp {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    split: gtk::Paned,
    // Sidebar
    sidebar_box: gtk::Box,
    sidebar_list: gtk::ListBox,
    add_repo_sidebar_button: gtk::Button,
    // Header buttons
    sidebar_toggle_button: gtk::Button,
    add_repo_header_button: gtk::Button,
    new_tab_button: gtk::Button,
    search_button: gtk::Button,
    menu_button: gtk::MenuButton,
    // Tab management
    tab_view: adw::TabView,
    tab_bar_widget: adw::TabBar,
    // Search
    search_revealer: gtk::Revealer,
    search_entry: gtk::SearchEntry,
    search_prev_button: gtk::Button,
    search_next_button: gtk::Button,
    search_close_button: gtk::Button,
    // Content
    empty_label: gtk::Label,
    status_label: gtk::Label,
    // Actions
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
    // State
    config: RefCell<UserConfig>,
    db: DbConn,
    workspace: RefCell<WorkspaceState>,
    host: RefCell<VteTerminalHost>,
    git_info_by_repo: RefCell<HashMap<String, git::GitInfo>>,
    git_refresh_source_id: RefCell<Option<gtk::glib::SourceId>>,
    active_process_info: RefCell<Option<git::ProcessInfo>>,
    process_refresh_source_id: RefCell<Option<gtk::glib::SourceId>>,
    layout_persist_source_id: RefCell<Option<gtk::glib::SourceId>>,
    session_command_running: RefCell<HashMap<String, bool>>,
    repo_split_state: RefCell<HashMap<String, NativeSplitState>>,
    closing_session_ids: RefCell<HashSet<String>>,
    rebuilding_tabs: Cell<bool>,
    paste_in_progress: Cell<bool>,
    sidebar_theme_css_provider: gtk::CssProvider,
}

impl NativeApp {
    fn bootstrap(app: &adw::Application) -> Rc<Self> {
        let config = UserConfig::load();
        let db = db::init_db(None).expect("failed to initialize Lantern database");
        let workspace = WorkspaceState::load(&db).unwrap_or_default();
        let repo_split_state = db::load_native_split_state(&db).unwrap_or_default();

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Lantern")
            .default_width(workspace.layout.window_width)
            .default_height(workspace.layout.window_height)
            .build();
        if workspace.layout.window_maximized {
            window.maximize();
        }
        let sidebar_width = workspace.layout.sidebar_width;
        let sidebar_collapsed = workspace.layout.sidebar_collapsed;

        // --- Header bar with icon buttons ---
        let header_bar = adw::HeaderBar::new();
        header_bar.set_title_widget(Some(&gtk::Label::new(Some("Lantern"))));

        let sidebar_toggle_button = gtk::Button::from_icon_name("sidebar-show-symbolic");
        sidebar_toggle_button.set_tooltip_text(Some("Toggle Sidebar"));
        let add_repo_header_button = gtk::Button::from_icon_name("folder-new-symbolic");
        add_repo_header_button.set_tooltip_text(Some("Add Repository"));
        let new_tab_button = gtk::Button::from_icon_name("tab-new-symbolic");
        new_tab_button.set_tooltip_text(Some("New Tab"));
        let search_button = gtk::Button::from_icon_name("system-search-symbolic");
        search_button.set_tooltip_text(Some("Find in Terminal"));

        // Menu popover with gio::Menu
        let menu = gio::Menu::new();
        let split_section = gio::Menu::new();
        split_section.append(Some("Split Right"), Some("win.split-right"));
        split_section.append(Some("Split Down"), Some("win.split-down"));
        split_section.append(Some("Close Split"), Some("win.close-split"));
        split_section.append(Some("Next Pane"), Some("win.next-pane"));
        split_section.append(Some("Flip Split"), Some("win.flip-split"));
        menu.append_section(None, &split_section);
        let tab_section = gio::Menu::new();
        tab_section.append(Some("Move Tab Left"), Some("win.move-tab-left"));
        tab_section.append(Some("Move Tab Right"), Some("win.move-tab-right"));
        tab_section.append(Some("Close Tab"), Some("win.close-tab"));
        tab_section.append(Some("Remove Repository"), Some("win.remove-repo"));
        menu.append_section(None, &tab_section);
        let settings_section = gio::Menu::new();
        settings_section.append(Some("Settings"), Some("win.settings"));
        menu.append_section(None, &settings_section);

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Menu"));
        menu_button.set_menu_model(Some(&menu));

        header_bar.pack_start(&sidebar_toggle_button);
        header_bar.pack_start(&add_repo_header_button);
        header_bar.pack_start(&new_tab_button);
        header_bar.pack_end(&menu_button);
        header_bar.pack_end(&search_button);

        // --- Actions ---
        let action_split_right = gio::SimpleAction::new("split-right", None);
        let action_split_down = gio::SimpleAction::new("split-down", None);
        let action_close_split = gio::SimpleAction::new("close-split", None);
        let action_next_pane = gio::SimpleAction::new("next-pane", None);
        let action_flip_split = gio::SimpleAction::new("flip-split", None);
        let action_move_tab_left = gio::SimpleAction::new("move-tab-left", None);
        let action_move_tab_right = gio::SimpleAction::new("move-tab-right", None);
        let action_close_tab = gio::SimpleAction::new("close-tab", None);
        let action_remove_repo = gio::SimpleAction::new("remove-repo", None);
        let action_settings = gio::SimpleAction::new("settings", None);
        window.add_action(&action_split_right);
        window.add_action(&action_split_down);
        window.add_action(&action_close_split);
        window.add_action(&action_next_pane);
        window.add_action(&action_flip_split);
        window.add_action(&action_move_tab_left);
        window.add_action(&action_move_tab_right);
        window.add_action(&action_close_tab);
        window.add_action(&action_remove_repo);
        window.add_action(&action_settings);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);

        let toast_overlay = adw::ToastOverlay::new();

        let split = gtk::Paned::new(gtk::Orientation::Horizontal);
        split.set_wide_handle(true);
        split.set_position(sidebar_width);

        // --- Sidebar: ListBox in ScrolledWindow + footer ---
        let sidebar_list = gtk::ListBox::new();
        sidebar_list.set_selection_mode(gtk::SelectionMode::None);
        sidebar_list.add_css_class("navigation-sidebar");

        let sidebar_scroll = gtk::ScrolledWindow::builder()
            .min_content_width(200)
            .child(&sidebar_list)
            .vexpand(true)
            .build();

        let add_repo_sidebar_button = gtk::Button::with_label("+ Add Repository");
        add_repo_sidebar_button.add_css_class("flat");
        add_repo_sidebar_button.set_margin_top(4);
        add_repo_sidebar_button.set_margin_bottom(8);
        add_repo_sidebar_button.set_margin_start(8);
        add_repo_sidebar_button.set_margin_end(8);

        let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_box.add_css_class("lantern-sidebar");
        sidebar_box.append(&sidebar_scroll);
        sidebar_box.append(&add_repo_sidebar_button);
        sidebar_box.set_visible(!sidebar_collapsed);
        split.set_start_child(Some(&sidebar_box));

        // --- Hover-visible remove button CSS ---
        let css = gtk::CssProvider::new();
        css.load_from_data(
            "listbox.navigation-sidebar row .remove-button { opacity: 0; transition: opacity 150ms; } \
             listbox.navigation-sidebar row:hover .remove-button { opacity: 1; } \
             listbox.navigation-sidebar row .sidebar-move-button { opacity: 0; transition: opacity 150ms; } \
             listbox.navigation-sidebar row:hover .sidebar-move-button { opacity: 0.7; } \
             listbox.navigation-sidebar row .sidebar-move-button image { -gtk-icon-size: 14px; } \
             listbox.navigation-sidebar row.sidebar-active-row { \
               background-color: alpha(@accent_bg_color, 0.12); \
               border-left: 3px solid @accent_bg_color; \
             }",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let sidebar_theme_css_provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &sidebar_theme_css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // --- Content area: zero margins ---
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // Tab bar (adw::TabBar + adw::TabView)
        let tab_view = adw::TabView::new();
        tab_view.set_hexpand(true);
        tab_view.set_vexpand(true);

        let tab_bar_widget = adw::TabBar::new();
        tab_bar_widget.set_view(Some(&tab_view));

        // Empty state label (toggled via visibility)
        let empty_label = gtk::Label::new(Some("No terminal selected."));
        empty_label.set_hexpand(true);
        empty_label.set_vexpand(true);
        empty_label.add_css_class("title-3");
        empty_label.set_visible(false);

        // Search bar with icon buttons
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_hexpand(true);
        search_entry.set_placeholder_text(Some("Search terminal output"));

        let search_prev_button = gtk::Button::from_icon_name("go-up-symbolic");
        search_prev_button.set_tooltip_text(Some("Previous Match"));
        let search_next_button = gtk::Button::from_icon_name("go-down-symbolic");
        search_next_button.set_tooltip_text(Some("Next Match"));
        let search_close_button = gtk::Button::from_icon_name("window-close-symbolic");
        search_close_button.set_tooltip_text(Some("Close Search"));

        let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        search_bar.set_margin_start(4);
        search_bar.set_margin_end(4);
        search_bar.append(&search_entry);
        search_bar.append(&search_prev_button);
        search_bar.append(&search_next_button);
        search_bar.append(&search_close_button);

        let search_revealer = gtk::Revealer::new();
        search_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        search_revealer.set_reveal_child(false);
        search_revealer.set_child(Some(&search_bar));

        // Status bar
        let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        status_box.set_margin_top(3);
        status_box.set_margin_bottom(3);
        status_box.set_margin_start(8);
        status_box.set_margin_end(8);
        let status_label = gtk::Label::new(Some("Starting native shell..."));
        status_label.set_xalign(0.0);
        status_label.set_hexpand(true);
        status_label.add_css_class("dim-label");
        status_box.append(&status_label);

        content_box.append(&tab_bar_widget);
        content_box.append(&search_revealer);
        content_box.append(&tab_view);
        content_box.append(&empty_label);
        content_box.append(&status_box);
        split.set_end_child(Some(&content_box));
        toast_overlay.set_child(Some(&split));
        toolbar_view.set_content(Some(&toast_overlay));
        window.set_content(Some(&toolbar_view));

        let native_app = Rc::new(Self {
            window,
            toast_overlay,
            split,
            sidebar_box,
            sidebar_list,
            add_repo_sidebar_button,
            sidebar_toggle_button,
            add_repo_header_button,
            new_tab_button,
            search_button,
            menu_button,
            tab_view,
            tab_bar_widget,
            search_revealer,
            search_entry,
            search_prev_button,
            search_next_button,
            search_close_button,
            empty_label,
            status_label,
            action_split_right,
            action_split_down,
            action_close_split,
            action_next_pane,
            action_flip_split,
            action_move_tab_left,
            action_move_tab_right,
            action_close_tab,
            action_remove_repo,
            action_settings,
            config: RefCell::new(config),
            db,
            workspace: RefCell::new(workspace),
            host: RefCell::new(VteTerminalHost::new()),
            git_info_by_repo: RefCell::new(HashMap::new()),
            git_refresh_source_id: RefCell::new(None),
            active_process_info: RefCell::new(None),
            process_refresh_source_id: RefCell::new(None),
            layout_persist_source_id: RefCell::new(None),
            session_command_running: RefCell::new(HashMap::new()),
            repo_split_state: RefCell::new(repo_split_state),
            closing_session_ids: RefCell::new(HashSet::new()),
            rebuilding_tabs: Cell::new(false),
            paste_in_progress: Cell::new(false),
            sidebar_theme_css_provider,
        });

        native_app.apply_theme();
        native_app.apply_ui_scale();
        native_app.ensure_active_repo_has_session();
        native_app.refresh_git_statuses();
        native_app.populate_sidebar();
        native_app.rebuild_tabs();
        native_app.show_active_terminal();
        native_app.wire_events();
        native_app.schedule_git_refresh();
        native_app.schedule_process_refresh();
        native_app
    }

    fn present(&self) {
        self.window.present();
    }

    fn wire_events(self: &Rc<Self>) {
        // --- Style manager ---
        let weak_self = Rc::downgrade(self);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            let config = native_app.config.borrow().clone();
            if config.theme.eq_ignore_ascii_case("system") {
                native_app.apply_sidebar_theme(&config.theme);
                native_app.apply_config_to_surfaces(&config);
            }
        });

        // --- Header icon buttons ---
        let weak_self = Rc::downgrade(self);
        self.sidebar_toggle_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.toggle_sidebar();
        });

        let weak_self = Rc::downgrade(self);
        self.add_repo_header_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.prompt_add_repo();
        });

        let weak_self = Rc::downgrade(self);
        self.add_repo_sidebar_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.prompt_add_repo();
        });

        let weak_self = Rc::downgrade(self);
        self.new_tab_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.create_tab();
        });

        let weak_self = Rc::downgrade(self);
        self.search_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.open_search();
        });

        // --- Menu actions ---
        let weak_self = Rc::downgrade(self);
        self.action_split_right.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.split_right();
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_split_down.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.split_down();
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_close_split.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.close_active_split();
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_next_pane.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.focus_other_split();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_flip_split.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.toggle_split_orientation();
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_move_tab_left.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.move_active_tab(-1);
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_move_tab_right.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.move_active_tab(1);
                native_app.focus_active_terminal();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_close_tab.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.close_active_tab();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_remove_repo.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.remove_active_repo();
            }
        });

        let weak_self = Rc::downgrade(self);
        self.action_settings.connect_activate(move |_, _| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.open_settings();
            }
        });

        // --- TabView signals ---
        let weak_self = Rc::downgrade(self);
        self.tab_view.connect_selected_page_notify(move |tab_view| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if native_app.rebuilding_tabs.get() {
                return;
            }
            let Some(page) = tab_view.selected_page() else {
                return;
            };
            let Some(session_id) = session_id_for_tab_page(&page) else {
                return;
            };
            let Some(repo_id) = native_app
                .workspace
                .borrow()
                .active_repo()
                .map(|repo| repo.repo.id.clone())
            else {
                return;
            };
            native_app.select_session(repo_id.as_str(), session_id.as_str());
        });

        let weak_self = Rc::downgrade(self);
        self.tab_view.connect_close_page(move |tab_view, page| {
            let Some(native_app) = weak_self.upgrade() else {
                tab_view.close_page_finish(page, true);
                return gtk::glib::Propagation::Stop;
            };
            if native_app.rebuilding_tabs.get() {
                tab_view.close_page_finish(page, true);
                return gtk::glib::Propagation::Stop;
            }
            let session_id = session_id_for_tab_page(page);
            let repo_id = native_app
                .workspace
                .borrow()
                .active_repo()
                .map(|repo| repo.repo.id.clone());
            // Deny the TabView close — close_tab handles removal via rebuild_tabs
            tab_view.close_page_finish(page, false);
            if let (Some(repo_id), Some(session_id)) = (repo_id, session_id) {
                native_app.close_tab(repo_id.as_str(), session_id.as_str());
            }
            gtk::glib::Propagation::Stop
        });

        let weak_self = Rc::downgrade(self);
        self.tab_view.connect_page_reordered(move |tab_view, _page, _position| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if native_app.rebuilding_tabs.get() {
                return;
            }
            let mut new_order = Vec::new();
            for i in 0..tab_view.n_pages() {
                let page = tab_view.nth_page(i);
                if let Some(session_id) = session_id_for_tab_page(&page) {
                    new_order.push(session_id);
                }
            }
            let repo_id = native_app
                .workspace
                .borrow()
                .active_repo()
                .map(|repo| repo.repo.id.clone());
            if let Some(repo_id) = repo_id {
                if let Err(error) =
                    db::reorder_sessions(&native_app.db, repo_id.as_str(), &new_order)
                {
                    native_app
                        .status_label
                        .set_text(format!("Failed to reorder tabs: {error}").as_str());
                    return;
                }
                native_app
                    .workspace
                    .borrow_mut()
                    .reorder_sessions(repo_id.as_str(), &new_order);
            }
        });

        // --- Sidebar row activation ---
        let weak_self = Rc::downgrade(self);
        self.sidebar_list.connect_row_activated(move |_, row| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            let repo_id = row.widget_name();
            if !repo_id.is_empty() {
                native_app.select_repo_by_id(&repo_id);
            }
        });

        // --- Search ---
        let weak_self = Rc::downgrade(self);
        self.search_entry.connect_search_changed(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Next) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_entry.connect_activate(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Next) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_entry.connect_next_match(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Next) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_entry.connect_previous_match(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Previous) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_entry.connect_stop_search(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.close_search();
        });

        let weak_self = Rc::downgrade(self);
        self.search_prev_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Previous) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_next_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if let Err(error) = native_app.search_active_terminal(SearchDirection::Next) {
                native_app
                    .status_label
                    .set_text(format!("Search failed: {error}").as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        self.search_close_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.close_search();
        });

        let weak_self = Rc::downgrade(self);
        self.split.connect_position_notify(move |split| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if native_app.sidebar_box.is_visible() {
                native_app.workspace.borrow_mut().layout.sidebar_width = split.position();
                native_app.schedule_layout_persist();
            }
        });

        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak_self = Rc::downgrade(self);
        key_controller.connect_key_pressed(move |_, key, _, state| {
            let Some(native_app) = weak_self.upgrade() else {
                return gtk::glib::Propagation::Proceed;
            };

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'t'))
                    .unwrap_or(false)
            {
                native_app.create_tab();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'w'))
                    .unwrap_or(false)
            {
                native_app.close_active_tab();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) && key == gtk::gdk::Key::Tab {
                native_app.select_relative_tab(
                    if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                        -1
                    } else {
                        1
                    },
                );
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key == gtk::gdk::Key::Page_Up
            {
                native_app.move_active_tab(-1);
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key == gtk::gdk::Key::Page_Down
            {
                native_app.move_active_tab(1);
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key == gtk::gdk::Key::Page_Up
            {
                native_app.move_active_split(-1);
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key == gtk::gdk::Key::Page_Down
            {
                native_app.move_active_split(1);
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
            {
                if let Some(character) = key.to_unicode() {
                    if state.contains(gtk::gdk::ModifierType::ALT_MASK)
                        && ('1'..='6').contains(&character)
                    {
                        native_app.focus_split_by_index((character as u8 - b'1') as usize);
                        return gtk::glib::Propagation::Stop;
                    }

                    if ('1'..='9').contains(&character) {
                        native_app.select_repo_by_shortcut((character as u8 - b'1') as usize);
                        return gtk::glib::Propagation::Stop;
                    }
                }
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'b'))
                    .unwrap_or(false)
            {
                native_app.toggle_sidebar();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key == gtk::gdk::Key::comma
            {
                native_app.open_settings();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key == gtk::gdk::Key::Right
            {
                native_app.split_right();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key == gtk::gdk::Key::Down
            {
                native_app.split_down();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'w'))
                    .unwrap_or(false)
            {
                native_app.close_active_split();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'o'))
                    .unwrap_or(false)
            {
                native_app.focus_other_split();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::ALT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'r'))
                    .unwrap_or(false)
            {
                native_app.toggle_split_orientation();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'f'))
                    .unwrap_or(false)
            {
                if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                    if native_app.search_revealer.reveals_child() {
                        native_app.close_search();
                    } else {
                        native_app.open_search();
                    }
                } else {
                    native_app.open_search();
                }
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'c'))
                    .unwrap_or(false)
            {
                native_app.copy_active_selection();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'v'))
                    .unwrap_or(false)
            {
                native_app.paste_clipboard_into_active_terminal();
                return gtk::glib::Propagation::Stop;
            }

            if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gtk::gdk::ModifierType::SHIFT_MASK)
                && key
                    .to_unicode()
                    .map(|character| character.eq_ignore_ascii_case(&'v'))
                    .unwrap_or(false)
            {
                native_app.smart_paste();
                return gtk::glib::Propagation::Stop;
            }

            if key == gtk::gdk::Key::Escape && native_app.search_revealer.reveals_child() {
                native_app.close_search();
                return gtk::glib::Propagation::Stop;
            }

            if key == gtk::gdk::Key::Escape {
                if let Some(surface) = native_app.active_surface() {
                    if surface.terminal().has_focus() {
                        return gtk::glib::Propagation::Proceed;
                    }
                }
                native_app.focus_active_terminal();
                return gtk::glib::Propagation::Stop;
            }

            if key == gtk::gdk::Key::F2 {
                native_app.prompt_rename_active_tab();
                return gtk::glib::Propagation::Stop;
            }

            gtk::glib::Propagation::Proceed
        });
        self.window.add_controller(key_controller);

        let weak_self = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.flush_layout_persist();
            }
            gtk::glib::Propagation::Proceed
        });

        let weak_self = Rc::downgrade(self);
        self.window.connect_maximized_notify(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.schedule_layout_persist();
        });

        let weak_self = Rc::downgrade(self);
        self.window.connect_default_width_notify(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.schedule_layout_persist();
        });

        let weak_self = Rc::downgrade(self);
        self.window.connect_default_height_notify(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.schedule_layout_persist();
        });
    }

    fn populate_sidebar(self: &Rc<Self>) {
        while let Some(child) = self.sidebar_list.first_child() {
            self.sidebar_list.remove(&child);
        }

        let workspace = self.workspace.borrow();
        let git_info_by_repo = self.git_info_by_repo.borrow();
        let font_size = self.config.borrow().font_size;
        let name_attrs = sidebar_font_attrs(font_size);
        let small_attrs = sidebar_font_attrs(font_size.saturating_sub(2).max(8));

        if workspace.repos.is_empty() {
            let empty_row = gtk::ListBoxRow::new();
            empty_row.set_activatable(false);
            empty_row.set_selectable(false);
            let empty_label = gtk::Label::new(Some("Add a repository to get started"));
            empty_label.add_css_class("dim-label");
            empty_label.set_margin_top(24);
            empty_label.set_margin_bottom(24);
            empty_row.set_child(Some(&empty_label));
            self.sidebar_list.append(&empty_row);
            return;
        }

        for (group_index, group) in sidebar_groups(&workspace.repos).iter().enumerate() {
            let collapsed = workspace
                .layout
                .collapsed_group_ids
                .iter()
                .any(|group_id| group_id == &group.group_id);

            // Group header row (non-activatable)
            let header_row = gtk::ListBoxRow::new();
            header_row.set_activatable(false);
            header_row.set_selectable(false);
            let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            header_box.set_margin_top(if group_index == 0 { 4 } else { 12 });
            header_box.set_margin_bottom(2);
            header_box.set_margin_end(4);

            let collapse_icon = gtk::Image::from_icon_name(if collapsed {
                "pan-end-symbolic"
            } else {
                "pan-down-symbolic"
            });
            collapse_icon.set_pixel_size(12);
            collapse_icon.add_css_class("dim-label");

            let header_label = gtk::Label::new(Some(group.name.to_uppercase().as_str()));
            header_label.set_xalign(0.0);
            header_label.set_hexpand(true);
            header_label.add_css_class("dim-label");
            header_label.set_attributes(Some(&small_attrs));

            let collapse_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            collapse_content.append(&collapse_icon);
            collapse_content.append(&header_label);

            let collapse_button = gtk::Button::new();
            collapse_button.add_css_class("flat");
            collapse_button.set_child(Some(&collapse_content));
            collapse_button.set_hexpand(true);
            let weak_self = Rc::downgrade(self);
            let group_id = group.group_id.clone();
            collapse_button.connect_clicked(move |_| {
                let Some(native_app) = weak_self.upgrade() else {
                    return;
                };
                native_app.toggle_group_collapsed(group_id.as_str());
            });
            header_box.append(&collapse_button);

            let move_up_button = gtk::Button::from_icon_name("go-up-symbolic");
            move_up_button.add_css_class("flat");
            move_up_button.add_css_class("sidebar-move-button");
            move_up_button.set_tooltip_text(Some("Move Group Up"));
            let weak_self = Rc::downgrade(self);
            let group_id = group.group_id.clone();
            move_up_button.connect_clicked(move |_| {
                let Some(native_app) = weak_self.upgrade() else {
                    return;
                };
                native_app.move_sidebar_group(group_id.as_str(), -1);
            });
            header_box.append(&move_up_button);

            let move_down_button = gtk::Button::from_icon_name("go-down-symbolic");
            move_down_button.add_css_class("flat");
            move_down_button.add_css_class("sidebar-move-button");
            move_down_button.set_tooltip_text(Some("Move Group Down"));
            let weak_self = Rc::downgrade(self);
            let group_id = group.group_id.clone();
            move_down_button.connect_clicked(move |_| {
                let Some(native_app) = weak_self.upgrade() else {
                    return;
                };
                native_app.move_sidebar_group(group_id.as_str(), 1);
            });
            header_box.append(&move_down_button);

            header_row.set_child(Some(&header_box));
            self.sidebar_list.append(&header_row);

            if !collapsed {
                for repo in &group.repos {
                    let is_active =
                        workspace.active_repo_id.as_deref() == Some(repo.repo.id.as_str());

                    let row = gtk::ListBoxRow::new();
                    row.set_widget_name(&repo.repo.id);
                    if is_active {
                        row.add_css_class("sidebar-active-row");
                    }

                    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    row_box.set_margin_start(8);
                    row_box.set_margin_end(4);
                    row_box.set_margin_top(3);
                    row_box.set_margin_bottom(3);

                    // Icon: main repo gets folder, worktree branches get branch icon
                    let icon_name = if group.is_worktree_group && !repo.repo.is_default {
                        "branch-symbolic"
                    } else {
                        "folder-open-symbolic"
                    };
                    let repo_icon = gtk::Image::from_icon_name(icon_name);
                    repo_icon.set_pixel_size(16);
                    repo_icon.set_valign(gtk::Align::Center);
                    repo_icon.set_opacity(if is_active { 0.9 } else { 0.5 });
                    if is_active {
                        repo_icon.add_css_class("accent");
                    }
                    row_box.append(&repo_icon);

                    let repo_content = gtk::Box::new(gtk::Orientation::Vertical, 1);
                    repo_content.set_halign(gtk::Align::Start);
                    repo_content.set_hexpand(true);

                    let repo_name = gtk::Label::new(Some(repo.repo.name.as_str()));
                    repo_name.set_xalign(0.0);
                    repo_name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    repo_name.set_attributes(Some(&name_attrs));
                    if is_active {
                        repo_name.add_css_class("accent");
                    }
                    repo_content.append(&repo_name);

                    let git_meta = git_info_by_repo
                        .get(repo.repo.id.as_str())
                        .map(sidebar_git_meta);
                    let has_meta = git_meta
                        .as_ref()
                        .is_some_and(|meta| !meta.text.is_empty());
                    if has_meta {
                        let meta = git_meta.unwrap();
                        let meta_label = if meta.use_markup {
                            let label = gtk::Label::new(None);
                            label.set_markup(meta.text.as_str());
                            label
                        } else {
                            gtk::Label::new(Some(meta.text.as_str()))
                        };
                        meta_label.set_xalign(0.0);
                        meta_label.add_css_class("dim-label");
                        meta_label.set_attributes(Some(&small_attrs));
                        repo_content.append(&meta_label);
                    }

                    row_box.append(&repo_content);

                    let remove_button = gtk::Button::from_icon_name("user-trash-symbolic");
                    remove_button.add_css_class("flat");
                    remove_button.add_css_class("remove-button");
                    remove_button.set_tooltip_text(Some("Remove Repository"));
                    remove_button.set_valign(gtk::Align::Center);
                    let weak_self = Rc::downgrade(self);
                    let repo_id = repo.repo.id.clone();
                    remove_button.connect_clicked(move |_| {
                        let Some(native_app) = weak_self.upgrade() else {
                            return;
                        };
                        native_app.remove_repo(repo_id.as_str());
                    });
                    row_box.append(&remove_button);

                    row.set_child(Some(&row_box));
                    self.sidebar_list.append(&row);
                }
            }
        }
    }

    fn rebuild_tabs(self: &Rc<Self>) {
        self.rebuilding_tabs.set(true);

        // Remove all existing pages
        while self.tab_view.n_pages() > 0 {
            self.tab_view.close_page(&self.tab_view.nth_page(0));
        }

        let Some(active_repo) = self.workspace.borrow().active_repo().cloned() else {
            self.new_tab_button.set_sensitive(false);
            self.search_button.set_sensitive(false);
            self.action_remove_repo.set_enabled(false);
            self.action_close_tab.set_enabled(false);
            self.action_move_tab_left.set_enabled(false);
            self.action_move_tab_right.set_enabled(false);
            self.action_split_right.set_enabled(false);
            self.action_split_down.set_enabled(false);
            self.action_close_split.set_enabled(false);
            self.action_next_pane.set_enabled(false);
            self.action_flip_split.set_enabled(false);
            self.action_settings.set_enabled(true);
            self.empty_label.set_text("No repositories configured yet.");
            self.empty_label.set_visible(true);
            self.tab_view.set_visible(false);
            self.rebuilding_tabs.set(false);
            return;
        };

        // Create pages for each session
        let mut selected_page: Option<adw::TabPage> = None;
        for session in &active_repo.sessions {
            let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
            wrapper.set_hexpand(true);
            wrapper.set_vexpand(true);
            wrapper.set_widget_name(&session.id);
            let page = self.tab_view.append(&wrapper);
            page.set_title(&session.title);
            if active_repo.active_session_id.as_deref() == Some(session.id.as_str()) {
                selected_page = Some(page);
            }
        }

        if let Some(page) = selected_page {
            self.tab_view.set_selected_page(&page);
        }

        // Update action sensitivity
        let has_session = active_repo.active_session_id.is_some();
        let active_session_index =
            active_repo
                .active_session_id
                .as_deref()
                .and_then(|session_id| {
                    active_repo
                        .sessions
                        .iter()
                        .position(|session| session.id == session_id)
                });
        let split_count = self
            .split_state_for_repo(
                active_repo.repo.id.as_str(),
                active_repo.active_session_id.as_deref(),
            )
            .visible_session_ids
            .len();

        self.new_tab_button.set_sensitive(true);
        self.search_button.set_sensitive(has_session);
        self.action_remove_repo.set_enabled(true);
        self.action_close_tab.set_enabled(has_session);
        self.action_move_tab_left
            .set_enabled(matches!(active_session_index, Some(index) if index > 0));
        self.action_move_tab_right.set_enabled(matches!(
            active_session_index,
            Some(index) if index + 1 < active_repo.sessions.len()
        ));
        self.action_split_right.set_enabled(has_session);
        self.action_split_down.set_enabled(has_session);
        self.action_close_split.set_enabled(split_count > 1);
        self.action_next_pane.set_enabled(split_count > 1);
        self.action_flip_split.set_enabled(split_count > 1);
        self.action_settings.set_enabled(true);

        if active_repo.sessions.is_empty() {
            self.empty_label
                .set_text("This repository has no saved terminal tabs yet.");
            self.empty_label.set_visible(true);
            self.tab_view.set_visible(false);
        } else {
            self.empty_label.set_visible(false);
            self.tab_view.set_visible(true);
        }

        self.rebuilding_tabs.set(false);
    }

    fn select_repo_by_id(self: &Rc<Self>, repo_id: &str) {
        self.workspace.borrow_mut().set_active_repo(repo_id);
        self.ensure_active_repo_has_session();
        self.populate_sidebar();
        self.rebuild_tabs();
        self.show_active_terminal();
        self.schedule_layout_persist();
    }

    fn select_session(self: &Rc<Self>, repo_id: &str, session_id: &str) {
        let previous_active_session_id = self
            .workspace
            .borrow()
            .repos
            .iter()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.active_session_id.clone());

        if previous_active_session_id.as_deref() == Some(session_id) {
            return;
        }

        self.workspace
            .borrow_mut()
            .set_active_session(repo_id, session_id);
        self.sync_visible_sessions_after_selection(
            repo_id,
            previous_active_session_id.as_deref(),
            session_id,
        );
        if let Err(error) = db::set_active_tab(&self.db, repo_id, session_id) {
            self.status_label
                .set_text(format!("Failed to persist active tab: {error}").as_str());
        }
        // Sync TabView selection without triggering handler
        self.select_tab_page_for_session(session_id);
        self.update_action_sensitivity();
        self.show_active_terminal();
    }

    fn select_tab_page_for_session(&self, session_id: &str) {
        self.rebuilding_tabs.set(true);
        if let Some(page) = find_tab_page_for_session(&self.tab_view, session_id) {
            self.tab_view.set_selected_page(&page);
        }
        self.rebuilding_tabs.set(false);
    }

    fn update_action_sensitivity(&self) {
        let Some(active_repo) = self.workspace.borrow().active_repo().cloned() else {
            self.new_tab_button.set_sensitive(false);
            self.search_button.set_sensitive(false);
            self.action_remove_repo.set_enabled(false);
            self.action_close_tab.set_enabled(false);
            self.action_move_tab_left.set_enabled(false);
            self.action_move_tab_right.set_enabled(false);
            self.action_split_right.set_enabled(false);
            self.action_split_down.set_enabled(false);
            self.action_close_split.set_enabled(false);
            self.action_next_pane.set_enabled(false);
            self.action_flip_split.set_enabled(false);
            return;
        };

        let has_session = active_repo.active_session_id.is_some();
        let active_session_index =
            active_repo
                .active_session_id
                .as_deref()
                .and_then(|session_id| {
                    active_repo
                        .sessions
                        .iter()
                        .position(|session| session.id == session_id)
                });
        let split_count = self
            .split_state_for_repo(
                active_repo.repo.id.as_str(),
                active_repo.active_session_id.as_deref(),
            )
            .visible_session_ids
            .len();

        self.new_tab_button.set_sensitive(true);
        self.search_button.set_sensitive(has_session);
        self.action_remove_repo.set_enabled(true);
        self.action_close_tab.set_enabled(has_session);
        self.action_move_tab_left
            .set_enabled(matches!(active_session_index, Some(index) if index > 0));
        self.action_move_tab_right.set_enabled(matches!(
            active_session_index,
            Some(index) if index + 1 < active_repo.sessions.len()
        ));
        self.action_split_right.set_enabled(has_session);
        self.action_split_down.set_enabled(has_session);
        self.action_close_split.set_enabled(split_count > 1);
        self.action_next_pane.set_enabled(split_count > 1);
        self.action_flip_split.set_enabled(split_count > 1);
    }

    fn prompt_add_repo(self: &Rc<Self>) {
        let dialog = gtk::FileChooserNative::builder()
            .title("Add Repository")
            .transient_for(&self.window)
            .accept_label("Add")
            .cancel_label("Cancel")
            .action(gtk::FileChooserAction::SelectFolder)
            .modal(true)
            .build();

        let weak_self = Rc::downgrade(self);
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(path) = dialog.file().and_then(|file| file.path()) {
                    if let Some(native_app) = weak_self.upgrade() {
                        native_app.handle_selected_repo_path(path.to_string_lossy().as_ref());
                    }
                }
            }
            dialog.hide();
        });

        dialog.show();
    }

    fn handle_selected_repo_path(self: &Rc<Self>, path: &str) {
        let Some(worktree_info) = git::detect_worktree_info(path) else {
            self.add_repo_path(path);
            return;
        };

        if worktree_info.entries.len() <= 1 {
            self.add_repo_path(path);
            return;
        }

        self.prompt_add_worktrees(path, &worktree_info);
    }

    fn add_repo_path(self: &Rc<Self>, path: &str) {
        match db::add_repo(&self.db, path) {
            Ok(repo) => {
                let repo = self.apply_worktree_grouping(repo);
                self.workspace.borrow_mut().add_repo(repo);
                self.refresh_git_statuses();
                self.ensure_active_repo_has_session();
                self.populate_sidebar();
                self.rebuild_tabs();
                self.show_active_terminal();
                self.schedule_layout_persist();
            }
            Err(error) => {
                self.status_label
                    .set_text(format!("Failed to add repository: {error}").as_str());
            }
        }
    }

    fn prompt_add_worktrees(self: &Rc<Self>, path: &str, worktree_info: &git::WorktreeInfo) {
        let dialog = gtk::Dialog::builder()
            .title(format!(
                "Worktrees detected for {}",
                worktree_info.repo_name
            ))
            .transient_for(&self.window)
            .modal(true)
            .build();
        dialog.add_button("Add All", gtk::ResponseType::Accept);
        dialog.add_button("Only This One", gtk::ResponseType::Apply);
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.set_default_response(gtk::ResponseType::Accept);

        let content = dialog.content_area();
        let description = gtk::Label::new(Some("Choose how to import this worktree family."));
        description.set_wrap(true);
        description.set_xalign(0.0);
        content.append(&description);

        let entries_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        entries_box.set_margin_top(8);
        for entry in &worktree_info.entries {
            let label = gtk::Label::new(Some(
                format!(
                    "• {}{}",
                    entry.branch.as_deref().unwrap_or(entry.name.as_str()),
                    if entry.is_main { " (main)" } else { "" }
                )
                .as_str(),
            ));
            label.set_xalign(0.0);
            entries_box.append(&label);
        }
        content.append(&entries_box);

        let weak_self = Rc::downgrade(self);
        let selected_path = path.to_string();
        dialog.connect_response(move |dialog, response| {
            if let Some(native_app) = weak_self.upgrade() {
                match response {
                    gtk::ResponseType::Accept => {
                        native_app.add_repo_with_worktrees(selected_path.as_str());
                    }
                    gtk::ResponseType::Apply => {
                        native_app.add_repo_path(selected_path.as_str());
                    }
                    _ => {}
                }
            }
            dialog.close();
        });

        dialog.present();
    }

    fn add_repo_with_worktrees(self: &Rc<Self>, path: &str) {
        let Some(worktree_info) = git::detect_worktree_info(path) else {
            self.add_repo_path(path);
            return;
        };

        let all_paths = worktree_info
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let group_id = db::find_group_id_by_paths(&self.db, &all_paths)
            .ok()
            .flatten()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut added_repo_ids = Vec::new();
        for entry in &worktree_info.entries {
            match db::add_repo_grouped(
                &self.db,
                entry.path.as_str(),
                Some(group_id.as_str()),
                entry.is_main,
            ) {
                Ok(repo) => {
                    let repo_id = repo.id.clone();
                    self.workspace.borrow_mut().add_repo(repo);
                    added_repo_ids.push(repo_id);
                }
                Err(LanternError::RepoAlreadyExists(_)) | Err(LanternError::PathNotFound(_)) => {
                    continue;
                }
                Err(error) => {
                    self.status_label
                        .set_text(format!("Failed to add worktree repository: {error}").as_str());
                    return;
                }
            }
        }

        for repo_id in &added_repo_ids {
            if let Err(error) = self.create_session_for_repo(repo_id.as_str()) {
                self.status_label
                    .set_text(format!("Failed to create initial terminal tab: {error}").as_str());
                return;
            }
        }

        let active_repo_id = self
            .workspace
            .borrow()
            .repos
            .iter()
            .find(|repo| repo.repo.path == path)
            .map(|repo| repo.repo.id.clone())
            .or_else(|| added_repo_ids.first().cloned());
        if let Some(active_repo_id) = active_repo_id {
            self.workspace
                .borrow_mut()
                .set_active_repo(active_repo_id.as_str());
        }

        self.refresh_git_statuses();
        self.populate_sidebar();
        self.rebuild_tabs();
        self.show_active_terminal();
        self.schedule_layout_persist();
    }

    fn apply_worktree_grouping(&self, repo: lantern_core::Repo) -> lantern_core::Repo {
        let Some(worktree_info) = git::detect_worktree_info(repo.path.as_str()) else {
            return repo;
        };

        let sibling_paths = worktree_info
            .entries
            .iter()
            .filter(|entry| entry.path != repo.path)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();

        let group_id = match db::find_group_id_by_paths(&self.db, &sibling_paths) {
            Ok(Some(id)) => id,
            _ => {
                // No existing group — check if any sibling repo exists ungrouped
                let sibling = sibling_paths.iter().find_map(|path| {
                    db::find_repo_id_by_path(&self.db, path).ok().flatten()
                });
                let Some(sibling_id) = sibling else {
                    return repo;
                };
                let new_group_id = Uuid::new_v4().to_string();
                let sibling_is_main = worktree_info
                    .entries
                    .iter()
                    .any(|e| sibling_paths.contains(&e.path) && e.is_main);
                if db::set_repo_group(&self.db, &sibling_id, &new_group_id, sibling_is_main)
                    .is_err()
                {
                    return repo;
                }
                // Update the sibling's in-memory state in the workspace
                if let Ok(Some(updated_sibling)) = db::list_repos(&self.db).map(|repos| {
                    repos.into_iter().find(|r| r.id == sibling_id)
                }) {
                    self.workspace.borrow_mut().update_repo(updated_sibling);
                }
                new_group_id
            }
        };

        let is_main = worktree_info
            .entries
            .iter()
            .any(|entry| entry.path == repo.path && entry.is_main);
        if db::set_repo_group(&self.db, repo.id.as_str(), group_id.as_str(), is_main).is_err() {
            return repo;
        }

        db::list_repos(&self.db)
            .ok()
            .and_then(|repos| {
                repos
                    .into_iter()
                    .find(|updated_repo| updated_repo.id == repo.id)
            })
            .unwrap_or(repo)
    }

    fn remove_active_repo(self: &Rc<Self>) {
        let Some(active_repo) = self.workspace.borrow().active_repo().cloned() else {
            self.status_label
                .set_text("No repository selected to remove.");
            return;
        };

        self.remove_repo(active_repo.repo.id.as_str());
    }

    fn remove_repo(self: &Rc<Self>, repo_id: &str) {
        let repo = self
            .workspace
            .borrow()
            .repos
            .iter()
            .find(|repo| repo.repo.id == repo_id)
            .cloned();
        let Some(repo) = repo else {
            self.status_label
                .set_text("No repository selected to remove.");
            return;
        };

        if let Err(error) = db::remove_repo(&self.db, repo.repo.id.as_str()) {
            self.status_label
                .set_text(format!("Failed to remove repository: {error}").as_str());
            return;
        }

        for session in &repo.sessions {
            self.host.borrow_mut().remove_surface(session.id.as_str());
            self.clear_session_runtime_state(session.id.as_str());
        }
        self.git_info_by_repo
            .borrow_mut()
            .remove(repo.repo.id.as_str());
        self.repo_split_state
            .borrow_mut()
            .remove(repo.repo.id.as_str());
        self.workspace
            .borrow_mut()
            .remove_repo(repo.repo.id.as_str());
        self.ensure_active_repo_has_session();
        self.populate_sidebar();
        self.rebuild_tabs();
        self.show_active_terminal();
        self.schedule_layout_persist();
    }

    fn select_relative_tab(self: &Rc<Self>, direction: isize) {
        let Some(active_repo) = self.workspace.borrow().active_repo().cloned() else {
            return;
        };
        if active_repo.sessions.is_empty() {
            return;
        }

        let Some(current_index) = active_repo.sessions.iter().position(|session| {
            Some(session.id.as_str()) == active_repo.active_session_id.as_deref()
        }) else {
            return;
        };
        let next_index = wrapped_index(active_repo.sessions.len(), current_index, direction);
        let next_session_id = active_repo.sessions[next_index].id.clone();
        self.select_session(active_repo.repo.id.as_str(), next_session_id.as_str());
    }

    fn move_active_tab(self: &Rc<Self>, direction: isize) {
        let Some(active_repo) = self.workspace.borrow().active_repo().cloned() else {
            return;
        };
        let Some(active_session_id) = active_repo.active_session_id.as_deref() else {
            return;
        };

        let session_ids = active_repo
            .sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<Vec<_>>();
        let Some(reordered_session_ids) =
            reordered_session_ids_for_tab_move(&session_ids, active_session_id, direction)
        else {
            return;
        };

        if let Err(error) = db::reorder_sessions(
            &self.db,
            active_repo.repo.id.as_str(),
            &reordered_session_ids,
        ) {
            self.status_label
                .set_text(format!("Failed to reorder tabs: {error}").as_str());
            return;
        }

        self.workspace
            .borrow_mut()
            .reorder_sessions(active_repo.repo.id.as_str(), &reordered_session_ids);
        self.rebuild_tabs();
        self.show_active_terminal();
    }

    fn move_active_split(self: &Rc<Self>, direction: isize) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        let Some(reordered_visible_session_ids) = reordered_visible_session_ids_for_pane_move(
            &split_state.visible_session_ids,
            active_session_id.as_str(),
            direction,
        ) else {
            return;
        };

        self.set_split_state(
            repo_id.as_str(),
            NativeSplitState {
                visible_session_ids: reordered_visible_session_ids,
                orientation: split_state.orientation,
                divider_positions: split_state.divider_positions,
            },
            Some(active_session_id.as_str()),
        );
        self.show_active_terminal();
    }

    fn select_repo_by_shortcut(self: &Rc<Self>, index: usize) {
        if let Some(repo_id) = self
            .workspace
            .borrow()
            .repos
            .get(index)
            .map(|repo| repo.repo.id.clone())
        {
            self.select_repo_by_id(repo_id.as_str());
        }
    }

    fn toggle_group_collapsed(self: &Rc<Self>, group_id: &str) {
        {
            let mut workspace = self.workspace.borrow_mut();
            if let Some(index) = workspace
                .layout
                .collapsed_group_ids
                .iter()
                .position(|collapsed_group_id| collapsed_group_id == group_id)
            {
                workspace.layout.collapsed_group_ids.remove(index);
            } else {
                workspace
                    .layout
                    .collapsed_group_ids
                    .push(group_id.to_string());
            }
        }
        self.populate_sidebar();
        self.schedule_layout_persist();
    }

    fn move_sidebar_group(self: &Rc<Self>, group_id: &str, direction: isize) {
        let group_order = sidebar_groups(&self.workspace.borrow().repos);
        let Some(reordered_repo_ids) =
            reordered_repo_ids_for_group_move(&group_order, group_id, direction)
        else {
            return;
        };

        if let Err(error) = db::reorder_repos(&self.db, &reordered_repo_ids) {
            self.status_label
                .set_text(format!("Failed to reorder repositories: {error}").as_str());
            return;
        }

        self.workspace
            .borrow_mut()
            .reorder_repos(&reordered_repo_ids);
        self.populate_sidebar();
        self.schedule_layout_persist();
    }

    fn toggle_sidebar(&self) {
        let sidebar_is_visible = self.sidebar_box.is_visible();
        if sidebar_is_visible {
            self.workspace.borrow_mut().layout.sidebar_width = self.split.position();
            self.sidebar_box.set_visible(false);
            self.workspace.borrow_mut().layout.sidebar_collapsed = true;
        } else {
            let sidebar_width = self.workspace.borrow().layout.sidebar_width.max(240);
            self.sidebar_box.set_visible(true);
            self.split.set_position(sidebar_width);
            self.workspace.borrow_mut().layout.sidebar_collapsed = false;
        }
        self.flush_layout_persist();
    }

    fn open_settings(self: &Rc<Self>) {
        let config = self.config.borrow().clone();
        let dialog = gtk::Dialog::builder()
            .title("Settings")
            .transient_for(&self.window)
            .modal(true)
            .build();
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Save", gtk::ResponseType::Accept);
        dialog.set_default_response(gtk::ResponseType::Accept);

        let grid = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let default_shell_entry =
            labeled_entry(&grid, 0, "Default Shell", config.default_shell.as_str());
        let font_family_entry = labeled_entry(&grid, 1, "Font Family", config.font_family.as_str());
        let theme_label = gtk::Label::new(Some("Theme"));
        theme_label.set_xalign(0.0);
        let theme_id = normalized_native_theme_id(config.theme.as_str());
        let theme_combo = gtk::ComboBoxText::new();
        for (id, label) in native_theme_options() {
            theme_combo.append(Some(id), label);
        }
        theme_combo.set_active_id(Some(theme_id.as_str()));
        theme_combo.set_hexpand(true);
        grid.attach(&theme_label, 0, 2, 1, 1);
        grid.attach(&theme_combo, 1, 2, 1, 1);
        let font_size = labeled_spin_button(
            &grid,
            3,
            "Font Size",
            8.0,
            32.0,
            1.0,
            config.font_size as f64,
        );
        let scrollback = labeled_spin_button(
            &grid,
            4,
            "Scrollback Lines",
            100.0,
            100000.0,
            100.0,
            config.scrollback_lines as f64,
        );
        let git_poll = labeled_spin_button(
            &grid,
            5,
            "Git Poll Interval",
            0.0,
            60.0,
            1.0,
            config.git_poll_interval_secs as f64,
        );
        let ui_scale = labeled_spin_button(&grid, 6, "UI Scale", 0.8, 1.5, 0.05, config.ui_scale);
        ui_scale.set_digits(2);

        // Restart hint row (initially hidden)
        let restart_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        restart_box.set_halign(gtk::Align::End);
        let restart_hint = gtk::Label::new(Some("Restart required for full effect"));
        restart_hint.add_css_class("dim-label");
        let restart_button = gtk::Button::with_label("Restart Now");
        restart_button.add_css_class("suggested-action");
        restart_box.append(&restart_hint);
        restart_box.append(&restart_button);
        restart_box.set_visible(false);
        grid.attach(&restart_box, 0, 7, 2, 1);

        dialog.content_area().append(&grid);

        let saved = Rc::new(std::cell::Cell::new(false));
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };

        let weak_self = Rc::downgrade(self);
        let preview_config = config.clone();
        theme_combo.connect_changed(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.apply_runtime_config(
                &settings_dialog_config(&preview_config, &preview_widgets),
                false,
            );
        });

        let weak_self = Rc::downgrade(self);
        let preview_config = config.clone();
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        font_family_entry.connect_changed(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.apply_runtime_config(
                &settings_dialog_config(&preview_config, &preview_widgets),
                false,
            );
        });

        let weak_self = Rc::downgrade(self);
        let preview_config = config.clone();
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        font_size.connect_value_changed(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.apply_runtime_config(
                &settings_dialog_config(&preview_config, &preview_widgets),
                false,
            );
        });

        let weak_self = Rc::downgrade(self);
        let preview_config = config.clone();
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        scrollback.connect_value_changed(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.apply_runtime_config(
                &settings_dialog_config(&preview_config, &preview_widgets),
                false,
            );
        });

        let weak_self = Rc::downgrade(self);
        let preview_config = config.clone();
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        let original_ui_scale = config.ui_scale;
        let restart_box_for_scale = restart_box.clone();
        ui_scale.connect_value_changed(move |spin| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            native_app.apply_runtime_config(
                &settings_dialog_config(&preview_config, &preview_widgets),
                false,
            );
            let changed = (spin.value() - original_ui_scale).abs() > f64::EPSILON;
            restart_box_for_scale.set_visible(changed);
        });

        let weak_self = Rc::downgrade(self);
        let restart_saved = saved.clone();
        let restart_config = config.clone();
        let restart_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        let restart_dialog = dialog.downgrade();
        restart_button.connect_clicked(move |_| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            restart_saved.set(true);
            let updated_config = settings_dialog_config(&restart_config, &restart_widgets);
            native_app.save_settings(updated_config);
            if let Some(dialog) = restart_dialog.upgrade() {
                dialog.close();
            }
            relaunch_app();
            native_app.window.close();
        });

        let weak_self = Rc::downgrade(self);
        let saved_flag = saved.clone();
        let response_config = config.clone();
        let preview_widgets = SettingsDialogWidgets {
            default_shell_entry: default_shell_entry.clone(),
            font_family_entry: font_family_entry.clone(),
            theme_combo: theme_combo.clone(),
            font_size: font_size.clone(),
            scrollback: scrollback.clone(),
            git_poll: git_poll.clone(),
            ui_scale: ui_scale.clone(),
        };
        dialog.connect_response(move |dialog, response| {
            if let Some(native_app) = weak_self.upgrade() {
                if response == gtk::ResponseType::Accept {
                    saved_flag.set(true);
                    let updated_config = settings_dialog_config(&response_config, &preview_widgets);
                    native_app.save_settings(updated_config);
                } else {
                    native_app.apply_runtime_config(&response_config, true);
                }
            }
            dialog.close();
        });

        let weak_self = Rc::downgrade(self);
        let saved_flag = saved.clone();
        let original_config = config.clone();
        dialog.connect_close_request(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                if !saved_flag.get() {
                    native_app.apply_runtime_config(&original_config, true);
                }
            }
            gtk::glib::Propagation::Proceed
        });

        dialog.present();
    }

    fn save_settings(self: &Rc<Self>, config: UserConfig) {
        let previous_ui_scale = self.config.borrow().ui_scale;

        if let Err(error) = config.save() {
            let current_config = self.config.borrow().clone();
            self.apply_runtime_config(&current_config, true);
            self.status_label
                .set_text(format!("Failed to save settings: {error}").as_str());
            return;
        }

        let ui_scale_changed = (config.ui_scale - previous_ui_scale).abs() > f64::EPSILON;
        self.config.replace(config.clone());
        self.apply_runtime_config(&config, true);
        self.show_toast("Settings saved.");
        if ui_scale_changed {
            self.show_toast("UI scale changes take full effect after restart.");
        }
    }

    fn apply_theme(&self) {
        let config = self.config.borrow().clone();
        self.apply_theme_for(config.theme.as_str());
        self.apply_sidebar_theme(config.theme.as_str());
    }

    fn apply_ui_scale(&self) {
        let config = self.config.borrow().clone();
        self.apply_ui_scale_for(config.ui_scale);
    }

    fn apply_runtime_config(self: &Rc<Self>, config: &UserConfig, refresh_git_schedule: bool) {
        self.apply_theme_for(config.theme.as_str());
        self.apply_sidebar_theme(config.theme.as_str());
        self.apply_ui_scale_for(config.ui_scale);
        self.apply_config_to_surfaces(config);
        if refresh_git_schedule {
            self.schedule_git_refresh_for(config.git_poll_interval_secs);
        }
        self.populate_sidebar();
        self.refresh_active_terminal_chrome();
    }

    fn apply_theme_for(&self, theme: &str) {
        let style_manager = adw::StyleManager::default();
        let color_scheme = theme_color_scheme(theme);
        style_manager.set_color_scheme(color_scheme);

        if let Some(settings) = gtk::Settings::default() {
            let prefer_dark = matches!(
                color_scheme,
                adw::ColorScheme::ForceDark | adw::ColorScheme::PreferDark
            );
            settings.set_gtk_application_prefer_dark_theme(prefer_dark);
        }
    }

    fn apply_sidebar_theme(&self, theme: &str) {
        let is_dark = theme_is_dark(theme);
        let css = sidebar_theme_css(theme, is_dark);
        self.sidebar_theme_css_provider.load_from_data(&css);
    }

    fn apply_ui_scale_for(&self, ui_scale: f64) {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_xft_dpi(ui_scale_to_xft_dpi(ui_scale));
        }
    }

    fn refresh_git_statuses(self: &Rc<Self>) {
        let repo_git_info = self
            .workspace
            .borrow()
            .repos
            .iter()
            .map(|repo| {
                (
                    repo.repo.id.clone(),
                    git::git_info_for_path(repo.repo.path.as_str()),
                )
            })
            .collect::<HashMap<_, _>>();
        let did_change = *self.git_info_by_repo.borrow() != repo_git_info;
        if did_change {
            self.git_info_by_repo.replace(repo_git_info);
            self.populate_sidebar();
        }

        self.discover_new_worktrees();
    }

    fn discover_new_worktrees(self: &Rc<Self>) {
        let known_paths: HashSet<String> = self
            .workspace
            .borrow()
            .repos
            .iter()
            .map(|r| r.repo.path.clone())
            .collect();

        let repo_paths: Vec<String> = self
            .workspace
            .borrow()
            .repos
            .iter()
            .map(|r| r.repo.path.clone())
            .collect();

        let mut added_any = false;
        for path in &repo_paths {
            let Some(worktree_info) = git::detect_worktree_info(path.as_str()) else {
                continue;
            };

            let new_entries: Vec<_> = worktree_info
                .entries
                .iter()
                .filter(|entry| !known_paths.contains(&entry.path))
                .collect();

            if new_entries.is_empty() {
                continue;
            }

            // Determine or create the group_id for this worktree family
            let all_paths: Vec<String> = worktree_info
                .entries
                .iter()
                .map(|e| e.path.clone())
                .collect();
            let group_id = db::find_group_id_by_paths(&self.db, &all_paths)
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    let new_group_id = Uuid::new_v4().to_string();
                    // Assign the new group to all existing siblings that lack one
                    for entry in &worktree_info.entries {
                        if known_paths.contains(&entry.path) {
                            if let Ok(Some(repo_id)) =
                                db::find_repo_id_by_path(&self.db, &entry.path)
                            {
                                let _ = db::set_repo_group(
                                    &self.db,
                                    &repo_id,
                                    &new_group_id,
                                    entry.is_main,
                                );
                            }
                        }
                    }
                    new_group_id
                });

            for entry in &new_entries {
                match db::add_repo_grouped(
                    &self.db,
                    &entry.path,
                    Some(&group_id),
                    entry.is_main,
                ) {
                    Ok(repo) => {
                        let repo_id = repo.id.clone();
                        self.workspace.borrow_mut().add_repo(repo);
                        let _ = self.create_session_for_repo(&repo_id);
                        added_any = true;
                    }
                    Err(LanternError::RepoAlreadyExists(_))
                    | Err(LanternError::PathNotFound(_)) => continue,
                    Err(_) => continue,
                }
            }
        }

        if added_any {
            // Reload all repos from DB so group_id changes are reflected
            if let Ok(repos) = db::list_repos(&self.db) {
                for repo in repos {
                    self.workspace.borrow_mut().update_repo(repo);
                }
            }
            self.populate_sidebar();
            self.rebuild_tabs();
            self.show_active_terminal();
            self.schedule_layout_persist();
        }
    }

    fn schedule_git_refresh(self: &Rc<Self>) {
        let git_poll_interval_secs = self.config.borrow().git_poll_interval_secs;
        self.schedule_git_refresh_for(git_poll_interval_secs);
    }

    fn schedule_git_refresh_for(self: &Rc<Self>, git_poll_interval_secs: u64) {
        if let Some(source_id) = self.git_refresh_source_id.borrow_mut().take() {
            source_id.remove();
        }

        let Some(git_poll_interval_secs) = effective_git_poll_interval_secs(git_poll_interval_secs)
        else {
            return;
        };

        let weak_self = Rc::downgrade(self);
        let source_id = gtk::glib::timeout_add_seconds_local(git_poll_interval_secs, move || {
            let Some(native_app) = weak_self.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            native_app.refresh_git_statuses();
            gtk::glib::ControlFlow::Continue
        });
        self.git_refresh_source_id.replace(Some(source_id));
    }

    fn schedule_process_refresh(self: &Rc<Self>) {
        if let Some(source_id) = self.process_refresh_source_id.borrow_mut().take() {
            source_id.remove();
        }

        let weak_self = Rc::downgrade(self);
        let source_id = gtk::glib::timeout_add_local(Duration::from_millis(750), move || {
            let Some(native_app) = weak_self.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            if should_poll_process_info(native_app.active_command_running_state()) {
                native_app.refresh_active_process_info();
            }
            gtk::glib::ControlFlow::Continue
        });
        self.process_refresh_source_id.replace(Some(source_id));
    }

    fn refresh_active_process_info(&self) {
        let next_process_info = self
            .active_surface()
            .and_then(|surface| surface.child_pid())
            .and_then(git::get_foreground_process);
        let did_change = *self.active_process_info.borrow() != next_process_info;
        if did_change {
            self.active_process_info.replace(next_process_info);
            self.refresh_active_terminal_chrome();
        }
    }

    fn clear_active_process_info(&self) {
        if self.active_process_info.borrow().is_some() {
            self.active_process_info.replace(None);
        }
    }

    fn active_command_running_state(&self) -> Option<bool> {
        let (_, session_id) = self.active_tab_ids()?;
        self.session_command_running
            .borrow()
            .get(session_id.as_str())
            .copied()
    }

    fn set_session_command_running(&self, session_id: &str, running: bool) {
        let did_change = self
            .session_command_running
            .borrow()
            .get(session_id)
            .copied()
            .unwrap_or(false)
            != running;
        if !did_change {
            return;
        }

        self.session_command_running
            .borrow_mut()
            .insert(session_id.to_string(), running);
        self.refresh_terminal_runtime_for_session(session_id);
    }

    fn clear_session_runtime_state(&self, session_id: &str) {
        let removed = self
            .session_command_running
            .borrow_mut()
            .remove(session_id)
            .is_some();
        if !removed {
            return;
        }

        let is_active = matches!(
            self.active_tab_ids(),
            Some((_, active_session_id)) if active_session_id == session_id
        );
        if is_active {
            self.clear_active_process_info();
            self.refresh_active_terminal_chrome();
        }
    }

    fn refresh_terminal_runtime_for_session(&self, session_id: &str) {
        let is_active = matches!(
            self.active_tab_ids(),
            Some((_, active_session_id)) if active_session_id == session_id
        );
        if !is_active {
            return;
        }

        self.refresh_active_process_info();
        self.refresh_active_terminal_chrome();
    }

    fn apply_config_to_surfaces(&self, config: &UserConfig) {
        for surface in self.host.borrow().surfaces() {
            surface.apply_config(config);
        }
    }

    fn focus_active_terminal(&self) {
        if let Some(surface) = self.active_surface() {
            surface.terminal().grab_focus();
        }
    }

    fn copy_active_selection(&self) {
        if let Some(surface) = self.active_surface() {
            surface.terminal().copy_clipboard_format(vte::Format::Text);
        }
    }

    fn paste_clipboard_into_active_terminal(self: &Rc<Self>) {
        if let Some(surface) = self.active_surface() {
            self.paste_in_progress.set(true);
            surface.terminal().paste_clipboard();
            let weak_self = Rc::downgrade(self);
            gtk::glib::idle_add_local_once(move || {
                if let Some(native_app) = weak_self.upgrade() {
                    native_app.paste_in_progress.set(false);
                }
            });
        }
    }

    fn smart_paste(self: &Rc<Self>) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        let clipboard = display.clipboard();
        let formats = clipboard.formats();

        if formats.contains_type(gtk::gdk::Texture::static_type()) {
            self.paste_image_from_clipboard(&clipboard);
        } else {
            self.paste_clipboard_into_active_terminal();
        }
    }

    fn paste_image_from_clipboard(self: &Rc<Self>, clipboard: &gtk::gdk::Clipboard) {
        let weak_self = Rc::downgrade(self);
        clipboard.read_texture_async(
            gtk::gio::Cancellable::NONE,
            move |result| {
                let Some(native_app) = weak_self.upgrade() else {
                    return;
                };
                match result {
                    Ok(Some(texture)) => {
                        let filename = format!("lantern-paste-{}.png", Uuid::new_v4());
                        let path = std::path::Path::new("/tmp").join(&filename);
                        if let Err(err) = texture.save_to_png(&path) {
                            eprintln!("lantern: failed to save clipboard image: {err}");
                            native_app.paste_clipboard_into_active_terminal();
                            return;
                        }
                        if let Some(surface) = native_app.active_surface() {
                            let path_str = path.to_string_lossy();
                            surface.terminal().feed_child(path_str.as_bytes());
                        }
                    }
                    _ => {
                        native_app.paste_clipboard_into_active_terminal();
                    }
                }
            },
        );
    }

    fn prompt_rename_active_tab(self: &Rc<Self>) {
        let Some((repo_id, session_id, current_title)) = self.active_tab_metadata() else {
            self.status_label
                .set_text("No active terminal tab to rename.");
            return;
        };

        self.prompt_rename_tab(
            repo_id.as_str(),
            session_id.as_str(),
            current_title.as_str(),
        );
    }

    fn prompt_rename_tab(self: &Rc<Self>, repo_id: &str, session_id: &str, current_title: &str) {
        let dialog = gtk::Dialog::builder()
            .title("Rename Tab")
            .transient_for(&self.window)
            .modal(true)
            .build();
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Rename", gtk::ResponseType::Accept);
        dialog.set_default_response(gtk::ResponseType::Accept);

        let entry = gtk::Entry::new();
        entry.set_hexpand(true);
        entry.set_text(current_title);
        entry.select_region(0, -1);
        entry.set_activates_default(true);
        dialog.content_area().append(&entry);

        let weak_self = Rc::downgrade(self);
        let entry_for_response = entry.clone();
        let repo_id = repo_id.to_string();
        let session_id = session_id.to_string();
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(native_app) = weak_self.upgrade() {
                    native_app.rename_active_tab(
                        repo_id.as_str(),
                        session_id.as_str(),
                        entry_for_response.text().as_str(),
                    );
                }
            }
            dialog.close();
        });

        dialog.present();
        entry.grab_focus();
    }

    fn rename_active_tab(self: &Rc<Self>, repo_id: &str, session_id: &str, raw_title: &str) {
        let Some(title) = normalized_session_title(raw_title) else {
            self.status_label.set_text("Tab title cannot be empty.");
            return;
        };

        if let Err(error) = db::rename_session(&self.db, session_id, title.as_str()) {
            self.status_label
                .set_text(format!("Failed to rename terminal tab: {error}").as_str());
            return;
        }

        self.workspace
            .borrow_mut()
            .rename_session(repo_id, session_id, title.as_str());
        if let Some(surface) = self.host.borrow().surface(session_id) {
            surface.set_fallback_title(title.as_str());
        }
        self.rebuild_tabs();
        self.refresh_active_terminal_chrome();
    }

    fn create_tab(self: &Rc<Self>) {
        let Some(repo_id) = self
            .workspace
            .borrow()
            .active_repo()
            .map(|repo| repo.repo.id.clone())
        else {
            self.status_label
                .set_text("Select a repository before creating a terminal tab.");
            return;
        };
        let previous_active_session_id = self
            .workspace
            .borrow()
            .active_repo()
            .and_then(|repo| repo.active_session_id.clone());

        match self.create_session_for_repo(repo_id.as_str()) {
            Ok(session_id) => {
                self.sync_visible_sessions_after_selection(
                    repo_id.as_str(),
                    previous_active_session_id.as_deref(),
                    session_id.as_str(),
                );
                self.rebuild_tabs();
                self.show_active_terminal();
            }
            Err(error) => {
                self.status_label
                    .set_text(format!("Failed to create terminal tab: {error}").as_str());
            }
        }
    }

    fn close_active_tab(self: &Rc<Self>) {
        let Some((repo_id, session_id)) = self.active_tab_ids() else {
            self.status_label
                .set_text("No active terminal tab to close.");
            return;
        };

        self.close_tab(repo_id.as_str(), session_id.as_str());
    }

    fn close_tab(self: &Rc<Self>, repo_id: &str, session_id: &str) {
        let current_active_session_id = self
            .workspace
            .borrow()
            .repos
            .iter()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.active_session_id.clone());
        let split_state = self.split_state_for_repo(repo_id, current_active_session_id.as_deref());

        if let Err(error) = db::close_session(&self.db, session_id) {
            self.status_label
                .set_text(format!("Failed to close terminal tab: {error}").as_str());
            return;
        }

        self.closing_session_ids
            .borrow_mut()
            .insert(session_id.to_string());
        self.host.borrow_mut().remove_surface(session_id);
        self.clear_session_runtime_state(session_id);
        self.closing_session_ids
            .borrow_mut()
            .remove(session_id);

        let next_active_session_id = {
            let mut workspace = self.workspace.borrow_mut();
            let was_active = current_active_session_id.as_deref() == Some(session_id);
            workspace.close_session(repo_id, session_id);
            let next_visible_session_id = remove_visible_session(
                &split_state.visible_session_ids,
                session_id,
                current_active_session_id
                    .as_deref()
                    .filter(|active_id| *active_id != session_id),
            )
            .first()
            .cloned();
            if was_active {
                if let Some(next_visible_session_id) = next_visible_session_id.clone() {
                    workspace.set_active_session(repo_id, next_visible_session_id.as_str());
                }
            }
            next_visible_session_id.or_else(|| {
                workspace
                    .repos
                    .iter()
                    .find(|repo| repo.repo.id == repo_id)
                    .and_then(|repo| repo.active_session_id.clone())
            })
        };
        self.remove_closed_session_from_visible_state(
            repo_id,
            session_id,
            next_active_session_id.as_deref(),
        );

        if let Some(next_active_session_id) = next_active_session_id {
            if let Err(error) =
                db::set_active_tab(&self.db, repo_id, next_active_session_id.as_str())
            {
                self.status_label.set_text(
                    format!("Closed tab but failed to persist selection: {error}").as_str(),
                );
            }
        }

        self.rebuild_tabs();
        self.show_active_terminal();
    }

    fn split_right(self: &Rc<Self>) {
        self.open_split(NativeSplitOrientation::Horizontal);
    }

    fn split_down(self: &Rc<Self>) {
        self.open_split(NativeSplitOrientation::Vertical);
    }

    fn focus_other_split(self: &Rc<Self>) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active split to focus.");
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        let Some(next_session_id) =
            next_visible_session_id(&split_state.visible_session_ids, active_session_id.as_str())
        else {
            self.status_label.set_text("No other pane to focus.");
            return;
        };

        self.select_session(repo_id.as_str(), next_session_id.as_str());
    }

    fn focus_split_by_index(self: &Rc<Self>, index: usize) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active split to focus.");
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        let Some(session_id) = nth_visible_session_id(&split_state.visible_session_ids, index)
        else {
            self.status_label
                .set_text(format!("Pane {} is not open.", index + 1).as_str());
            return;
        };

        self.select_session(repo_id.as_str(), session_id.as_str());
    }

    fn toggle_split_orientation(self: &Rc<Self>) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active split to flip.");
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        if split_state.visible_session_ids.len() <= 1 {
            self.status_label.set_text("No split to flip.");
            return;
        }

        self.set_split_state(
            repo_id.as_str(),
            NativeSplitState {
                visible_session_ids: split_state.visible_session_ids,
                orientation: toggled_split_orientation(split_state.orientation),
                divider_positions: Vec::new(),
            },
            Some(active_session_id.as_str()),
        );
        self.rebuild_tabs();
        self.show_active_terminal();
    }

    fn open_split(self: &Rc<Self>, orientation: NativeSplitOrientation) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active terminal to split.");
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        if split_state.visible_session_ids.len() >= MAX_VISIBLE_SPLITS {
            self.status_label.set_text(
                format!("Only {MAX_VISIBLE_SPLITS} panes are supported right now.").as_str(),
            );
            return;
        }

        match self.create_session_for_repo(repo_id.as_str()) {
            Ok(session_id) => {
                self.set_split_state(
                    repo_id.as_str(),
                    NativeSplitState {
                        visible_session_ids: append_split_session(
                            &split_state.visible_session_ids,
                            session_id.as_str(),
                        ),
                        orientation,
                        divider_positions: Vec::new(),
                    },
                    Some(session_id.as_str()),
                );
                self.rebuild_tabs();
                self.show_active_terminal();
            }
            Err(error) => {
                self.status_label
                    .set_text(format!("Failed to split terminal: {error}").as_str());
            }
        }
    }

    fn close_active_split(self: &Rc<Self>) {
        let Some((repo_id, active_session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active split to close.");
            return;
        };

        let split_state =
            self.split_state_for_repo(repo_id.as_str(), Some(active_session_id.as_str()));
        if split_state.visible_session_ids.len() <= 1 {
            self.status_label.set_text("No split to close.");
            return;
        }

        let remaining_sessions = remove_visible_session(
            &split_state.visible_session_ids,
            active_session_id.as_str(),
            None,
        );
        let Some(next_active_session_id) = remaining_sessions.first().cloned() else {
            self.status_label.set_text("No split to focus.");
            return;
        };

        self.workspace
            .borrow_mut()
            .set_active_session(repo_id.as_str(), next_active_session_id.as_str());
        self.set_split_state(
            repo_id.as_str(),
            NativeSplitState {
                visible_session_ids: remaining_sessions,
                orientation: split_state.orientation,
                divider_positions: split_state.divider_positions,
            },
            Some(next_active_session_id.as_str()),
        );
        if let Err(error) =
            db::set_active_tab(&self.db, repo_id.as_str(), next_active_session_id.as_str())
        {
            self.status_label
                .set_text(format!("Failed to persist active split: {error}").as_str());
        }

        self.rebuild_tabs();
        self.show_active_terminal();
    }

    fn show_active_terminal(self: &Rc<Self>) {
        let active = {
            let workspace = self.workspace.borrow();
            match workspace.active_repo().cloned() {
                Some(repo) => match repo.active_session_id.clone() {
                    Some(session_id) => Some((repo, Some(session_id))),
                    None => Some((repo, None)),
                },
                None => None,
            }
        };

        let Some((repo, session)) = active else {
            self.close_search();
            self.clear_active_process_info();
            self.empty_label.set_text("No repositories configured yet.");
            self.empty_label.set_visible(true);
            self.tab_view.set_visible(false);
            self.status_label.set_text("No active repository.");
            self.window.set_title(Some("Lantern"));
            return;
        };

        let Some(active_session_id) = session else {
            self.close_search();
            self.clear_active_process_info();
            self.empty_label
                .set_text("This repository has no saved terminal tabs yet.");
            self.empty_label.set_visible(true);
            self.tab_view.set_visible(false);
            self.status_label
                .set_text(format!("{} • no tabs", repo.repo.path).as_str());
            self.window.set_title(Some("Lantern"));
            return;
        };

        self.empty_label.set_visible(false);
        self.tab_view.set_visible(true);
        let split_state =
            self.split_state_for_repo(repo.repo.id.as_str(), Some(active_session_id.as_str()));
        let layout = self.build_terminal_layout(&repo, &split_state);
        self.replace_terminal_layout(&layout);
        if let Some(surface) = self.active_surface() {
            surface.terminal().grab_focus();
        }
        self.refresh_active_process_info();
        self.refresh_active_terminal_chrome();
        self.sync_search_with_active_terminal();
    }

    fn persist_layout(&self) {
        let workspace = self.workspace.borrow();
        let layout = AppLayout {
            window_x: None,
            window_y: None,
            window_width: self.window.width(),
            window_height: self.window.height(),
            window_maximized: self.window.is_maximized(),
            sidebar_width: if self.sidebar_box.is_visible() {
                self.split.position()
            } else {
                workspace.layout.sidebar_width
            },
            sidebar_collapsed: workspace.layout.sidebar_collapsed,
            active_repo_id: workspace.active_repo_id.clone(),
            collapsed_group_ids: workspace.layout.collapsed_group_ids.clone(),
        };

        if let Err(error) = db::save_layout(&self.db, &layout) {
            eprintln!("Failed to save native window layout: {error}");
        }
    }

    fn schedule_layout_persist(self: &Rc<Self>) {
        if let Some(source_id) = self.layout_persist_source_id.borrow_mut().take() {
            source_id.remove();
        }

        let weak_self = Rc::downgrade(self);
        let source_id = gtk::glib::timeout_add_local(Duration::from_millis(200), move || {
            let Some(native_app) = weak_self.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            native_app.layout_persist_source_id.borrow_mut().take();
            native_app.persist_layout();
            gtk::glib::ControlFlow::Break
        });
        self.layout_persist_source_id.replace(Some(source_id));
    }

    fn flush_layout_persist(&self) {
        if let Some(source_id) = self.layout_persist_source_id.borrow_mut().take() {
            source_id.remove();
        }
        self.persist_layout();
    }

    fn active_tab_ids(&self) -> Option<(String, String)> {
        let workspace = self.workspace.borrow();
        let repo = workspace.active_repo()?;
        let session_id = repo.active_session_id.clone()?;
        Some((repo.repo.id.clone(), session_id))
    }

    fn active_tab_metadata(&self) -> Option<(String, String, String)> {
        let workspace = self.workspace.borrow();
        let repo = workspace.active_repo()?;
        let session = repo.active_session_id.as_deref().and_then(|session_id| {
            repo.sessions
                .iter()
                .find(|session| session.id == session_id)
        })?;

        Some((
            repo.repo.id.clone(),
            session.id.clone(),
            session.title.clone(),
        ))
    }

    fn ensure_active_repo_has_session(self: &Rc<Self>) {
        let workspace = self.workspace.borrow();
        let Some(repo) = workspace.active_repo() else {
            return;
        };
        if !repo.sessions.is_empty() {
            return;
        }

        let repo_id = repo.repo.id.clone();
        drop(workspace);

        if let Err(error) = self.create_session_for_repo(repo_id.as_str()) {
            self.status_label
                .set_text(format!("Failed to create initial terminal tab: {error}").as_str());
        }
    }

    fn create_session_for_repo(self: &Rc<Self>, repo_id: &str) -> Result<String, LanternError> {
        let title = {
            let workspace = self.workspace.borrow();
            let repo = workspace
                .repos
                .iter()
                .find(|repo| repo.repo.id == repo_id)
                .ok_or_else(|| LanternError::RepoNotFound(repo_id.to_string()))?;
            next_session_title(&repo.sessions)
        };

        let session = db::create_session(&self.db, repo_id, title.as_str(), None)?;
        let session_id = session.id.clone();
        db::set_active_tab(&self.db, repo_id, session.id.as_str())?;
        self.workspace.borrow_mut().add_session(session);
        Ok(session_id)
    }

    fn bind_surface_events(
        self: &Rc<Self>,
        repo_id: &str,
        session_id: &str,
        terminal: &vte::Terminal,
    ) {
        let terminal = terminal.clone();

        let weak_self = Rc::downgrade(self);
        let title_session_id = session_id.to_string();
        terminal.connect_window_title_changed(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.refresh_terminal_chrome_for_session(title_session_id.as_str());
            }
        });

        let weak_self = Rc::downgrade(self);
        let cwd_session_id = session_id.to_string();
        terminal.connect_current_directory_uri_changed(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.refresh_terminal_chrome_for_session(cwd_session_id.as_str());
            }
        });

        if signal_exists(&terminal, "shell-preexec") {
            let weak_self = Rc::downgrade(self);
            let running_session_id = session_id.to_string();
            terminal.connect_shell_preexec(move |_| {
                if let Some(native_app) = weak_self.upgrade() {
                    native_app.set_session_command_running(running_session_id.as_str(), true);
                }
            });
        }

        if signal_exists(&terminal, "shell-precmd") {
            let weak_self = Rc::downgrade(self);
            let idle_session_id = session_id.to_string();
            terminal.connect_shell_precmd(move |_| {
                if let Some(native_app) = weak_self.upgrade() {
                    native_app.set_session_command_running(idle_session_id.as_str(), false);
                }
            });
        }

        let weak_self = Rc::downgrade(self);
        let refresh_session_id = session_id.to_string();
        let last_dimensions = Rc::new(Cell::new((terminal.column_count(), terminal.row_count())));
        let dimensions_handle = last_dimensions.clone();
        terminal.connect_contents_changed(move |terminal| {
            let next_dimensions = (terminal.column_count(), terminal.row_count());
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            let should_refresh = native_app
                .host
                .borrow()
                .surface(refresh_session_id.as_str())
                .and_then(|surface| surface.launch_error())
                .is_some();
            if !should_refresh && dimensions_handle.get() == next_dimensions {
                return;
            }
            dimensions_handle.set(next_dimensions);
            native_app.refresh_terminal_chrome_for_session(refresh_session_id.as_str());
        });

        let weak_self = Rc::downgrade(self);
        let exited_session_id = session_id.to_string();
        terminal.connect_child_exited(move |_, _| {
            let Some(native_app) = weak_self.upgrade() else {
                return;
            };
            if native_app
                .closing_session_ids
                .borrow()
                .contains(exited_session_id.as_str())
            {
                return;
            }
            native_app.clear_session_runtime_state(exited_session_id.as_str());
            native_app.refresh_active_process_info();
            native_app.refresh_terminal_chrome_for_session(exited_session_id.as_str());
            native_app.notify_session_exit(exited_session_id.as_str());
        });

        let weak_self = Rc::downgrade(self);
        let bell_session_id = session_id.to_string();
        terminal.connect_bell(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.notify_session_bell(bell_session_id.as_str());
            }
        });

        let focus_controller = gtk::EventControllerFocus::new();
        let weak_self = Rc::downgrade(self);
        let focus_repo_id = repo_id.to_string();
        let focus_session_id = session_id.to_string();
        focus_controller.connect_enter(move |_| {
            if let Some(native_app) = weak_self.upgrade() {
                native_app.select_session(focus_repo_id.as_str(), focus_session_id.as_str());
            }
        });
        terminal.add_controller(focus_controller);
    }

    fn build_terminal_layout(
        self: &Rc<Self>,
        repo: &RepoWorkspace,
        split_state: &NativeSplitState,
    ) -> gtk::Widget {
        let surfaces = split_state
            .visible_session_ids
            .iter()
            .filter_map(|session_id| self.ensure_surface(repo, session_id))
            .collect::<Vec<_>>();

        self.build_split_widget(
            repo.repo.id.as_str(),
            split_state.orientation,
            &split_state.divider_positions,
            surfaces.as_slice(),
            0,
        )
    }

    fn build_split_widget(
        self: &Rc<Self>,
        repo_id: &str,
        orientation: NativeSplitOrientation,
        divider_positions: &[i32],
        surfaces: &[crate::terminal_host::TerminalSurface],
        divider_index: usize,
    ) -> gtk::Widget {
        match surfaces {
            [] => gtk::Box::new(gtk::Orientation::Vertical, 0).upcast::<gtk::Widget>(),
            [surface] => {
                let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
                detach_widget(surface.terminal());
                container.append(surface.terminal());
                container.upcast::<gtk::Widget>()
            }
            [first, remaining @ ..] => {
                detach_widget(first.terminal());
                let paned = gtk::Paned::new(gtk_orientation(orientation));
                paned.set_wide_handle(true);
                paned.set_position(divider_positions.get(divider_index).copied().unwrap_or(
                    default_split_position(orientation, self.window.width(), self.window.height()),
                ));
                paned.set_start_child(Some(first.terminal()));

                let nested = self.build_split_widget(
                    repo_id,
                    orientation,
                    divider_positions,
                    remaining,
                    divider_index + 1,
                );
                paned.set_end_child(Some(&nested));

                let weak_self = Rc::downgrade(self);
                let repo_id = repo_id.to_string();
                paned.connect_position_notify(move |paned| {
                    if let Some(native_app) = weak_self.upgrade() {
                        native_app.update_split_divider_position(
                            repo_id.as_str(),
                            divider_index,
                            paned.position(),
                        );
                    }
                });
                paned.upcast::<gtk::Widget>()
            }
        }
    }

    fn replace_terminal_layout(&self, layout: &impl IsA<gtk::Widget>) {
        let Some(page) = self.tab_view.selected_page() else {
            return;
        };
        let wrapper = page.child();
        clear_box_children(&wrapper);
        if let Some(wrapper_box) = wrapper.downcast_ref::<gtk::Box>() {
            wrapper_box.append(layout);
        }
    }

    fn ensure_surface(
        self: &Rc<Self>,
        repo: &RepoWorkspace,
        session_id: &str,
    ) -> Option<crate::terminal_host::TerminalSurface> {
        let session = repo
            .sessions
            .iter()
            .find(|session| session.id == session_id)?
            .clone();
        let needs_binding = self.host.borrow().surface(session_id).is_none();
        let config = self.config.borrow();
        let surface = self
            .host
            .borrow_mut()
            .ensure_surface(repo, &session, &config);
        if needs_binding {
            self.bind_surface_events(repo.repo.id.as_str(), session_id, surface.terminal());
        }
        Some(surface)
    }

    fn split_state_for_repo(
        &self,
        repo_id: &str,
        active_session_id: Option<&str>,
    ) -> NativeSplitState {
        let available_session_ids = self.available_session_ids(repo_id);
        let current_split_state = self
            .repo_split_state
            .borrow()
            .get(repo_id)
            .cloned()
            .unwrap_or_default();
        let normalized_split_state = normalize_split_state(
            &current_split_state,
            &available_session_ids,
            active_session_id,
        );
        let should_persist = normalized_split_state != current_split_state
            || (normalized_split_state.visible_session_ids.len() <= 1
                && !current_split_state.visible_session_ids.is_empty());
        self.repo_split_state
            .borrow_mut()
            .insert(repo_id.to_string(), normalized_split_state.clone());
        if should_persist {
            self.persist_split_state(repo_id, &normalized_split_state);
        }
        normalized_split_state
    }

    fn available_session_ids(&self, repo_id: &str) -> Vec<String> {
        self.workspace
            .borrow()
            .repos
            .iter()
            .find(|repo| repo.repo.id == repo_id)
            .map(|repo| {
                repo.sessions
                    .iter()
                    .map(|session| session.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn set_split_state(
        &self,
        repo_id: &str,
        split_state: NativeSplitState,
        active_session_id: Option<&str>,
    ) {
        let available_session_ids = self.available_session_ids(repo_id);
        let normalized_split_state =
            normalize_split_state(&split_state, &available_session_ids, active_session_id);
        if normalized_split_state.visible_session_ids.is_empty() {
            self.repo_split_state.borrow_mut().remove(repo_id);
            self.persist_split_state(repo_id, &normalized_split_state);
            return;
        }
        self.repo_split_state
            .borrow_mut()
            .insert(repo_id.to_string(), normalized_split_state.clone());
        self.persist_split_state(repo_id, &normalized_split_state);
    }

    fn sync_visible_sessions_after_selection(
        &self,
        repo_id: &str,
        previous_active_session_id: Option<&str>,
        new_active_session_id: &str,
    ) {
        let split_state = self.split_state_for_repo(
            repo_id,
            previous_active_session_id.or(Some(new_active_session_id)),
        );
        let updated_visible_sessions = apply_active_session_change(
            &split_state.visible_session_ids,
            previous_active_session_id,
            new_active_session_id,
        );
        self.set_split_state(
            repo_id,
            NativeSplitState {
                visible_session_ids: updated_visible_sessions,
                orientation: split_state.orientation,
                divider_positions: split_state.divider_positions,
            },
            Some(new_active_session_id),
        );
    }

    fn remove_closed_session_from_visible_state(
        &self,
        repo_id: &str,
        removed_session_id: &str,
        fallback_active_session_id: Option<&str>,
    ) {
        let split_state = self.split_state_for_repo(repo_id, fallback_active_session_id);
        let updated_visible_sessions = remove_visible_session(
            &split_state.visible_session_ids,
            removed_session_id,
            fallback_active_session_id,
        );
        self.set_split_state(
            repo_id,
            NativeSplitState {
                visible_session_ids: updated_visible_sessions,
                orientation: split_state.orientation,
                divider_positions: split_state.divider_positions,
            },
            fallback_active_session_id,
        );
    }

    fn persist_split_state(&self, repo_id: &str, split_state: &NativeSplitState) {
        let result = if split_state.visible_session_ids.len() > 1 {
            db::save_native_split_state(&self.db, repo_id, split_state)
        } else {
            db::delete_native_split_state(&self.db, repo_id)
        };

        if let Err(error) = result {
            eprintln!("Failed to persist native split state for {repo_id}: {error}");
        }
    }

    fn update_split_divider_position(
        &self,
        repo_id: &str,
        divider_index: usize,
        divider_position: i32,
    ) {
        let Some(current_split_state) = self.repo_split_state.borrow().get(repo_id).cloned() else {
            return;
        };
        let divider_count = current_split_state
            .visible_session_ids
            .len()
            .saturating_sub(1);
        if divider_count == 0 || divider_index >= divider_count {
            return;
        }

        let mut divider_positions = current_split_state.divider_positions;
        let default_position = default_split_position(
            current_split_state.orientation,
            self.window.width(),
            self.window.height(),
        );
        divider_positions.resize(divider_count, default_position);
        if divider_positions[divider_index] == divider_position {
            return;
        }
        divider_positions[divider_index] = divider_position;
        let updated_split_state = NativeSplitState {
            divider_positions,
            ..current_split_state
        };
        self.repo_split_state
            .borrow_mut()
            .insert(repo_id.to_string(), updated_split_state.clone());
        self.persist_split_state(repo_id, &updated_split_state);
    }

    fn refresh_terminal_chrome_for_session(&self, session_id: &str) {
        let is_active = matches!(
            self.active_tab_ids(),
            Some((_, active_session_id)) if active_session_id == session_id
        );
        if is_active {
            self.refresh_active_terminal_chrome();
        }
    }

    fn notify_session_bell(&self, session_id: &str) {
        if self.paste_in_progress.get() {
            return;
        }
        let Some(surface) = self.host.borrow().surface(session_id) else {
            self.show_toast("Terminal bell");
            return;
        };
        self.show_toast(format!("Bell from {}", surface.title()).as_str());
    }

    fn notify_session_exit(&self, session_id: &str) {
        let Some(surface) = self.host.borrow().surface(session_id) else {
            return;
        };
        let Some(exit_status) = surface.exit_status() else {
            return;
        };
        let message = match exit_status {
            0 => format!("{} finished", surface.title()),
            status => format!("{} exited with status {}", surface.title(), status),
        };
        self.show_toast(message.as_str());
    }

    fn show_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(3);
        self.toast_overlay.add_toast(toast);
    }

    fn refresh_active_terminal_chrome(&self) {
        let Some((repo_id, session_id)) = self.active_tab_ids() else {
            self.status_label.set_text("No active repository.");
            self.window.set_title(Some("Lantern"));
            return;
        };

        let Some(surface) = self.host.borrow().surface(session_id.as_str()) else {
            self.status_label.set_text("Starting native shell...");
            self.window.set_title(Some("Lantern"));
            return;
        };

        let title = surface.title();
        let working_directory = surface.working_directory();
        if let Some(error) = surface.launch_error() {
            self.status_label.set_text(
                launch_failure_status(working_directory.as_str(), error.as_str()).as_str(),
            );
            self.window
                .set_title(Some(window_title(title.as_str()).as_str()));
            return;
        }
        let git_meta = self
            .git_info_by_repo
            .borrow()
            .get(repo_id.as_str())
            .map(status_git_meta)
            .unwrap_or_default();
        let active_process_info = self.active_process_info.borrow().clone();
        let dimensions = terminal_dimensions(surface.terminal());
        let command_running = self
            .session_command_running
            .borrow()
            .get(session_id.as_str())
            .copied()
            .unwrap_or(false);
        let status = status_text(
            working_directory.as_str(),
            title.as_str(),
            surface.exit_status(),
            git_meta.as_str(),
            active_process_info.as_ref(),
            dimensions.as_deref(),
            command_running,
        );
        let window_title = window_title(title.as_str());

        self.status_label.set_text(status.as_str());
        self.window.set_title(Some(window_title.as_str()));
    }

    fn active_surface(&self) -> Option<crate::terminal_host::TerminalSurface> {
        let (_, session_id) = self.active_tab_ids()?;
        self.host.borrow().surface(session_id.as_str())
    }

    fn open_search(&self) {
        if self.active_surface().is_none() {
            return;
        }

        self.search_revealer.set_reveal_child(true);
        self.search_entry.grab_focus();
        self.search_entry.select_region(0, -1);

        if !self.search_entry.text().is_empty() {
            let _ = self.search_active_terminal(SearchDirection::Next);
        }
    }

    fn close_search(&self) {
        self.search_revealer.set_reveal_child(false);
        self.search_entry.set_text("");
        self.clear_search_on_active_terminal();
        self.refresh_active_terminal_chrome();

        if let Some(surface) = self.active_surface() {
            surface.terminal().grab_focus();
        }
    }

    fn sync_search_with_active_terminal(&self) {
        if self.search_revealer.reveals_child() && !self.search_entry.text().is_empty() {
            let _ = self.search_active_terminal(SearchDirection::Next);
        } else {
            self.clear_search_on_active_terminal();
        }
    }

    fn clear_search_on_active_terminal(&self) {
        if let Some(surface) = self.active_surface() {
            surface.terminal().search_set_regex(None, 0);
        }
    }

    fn search_active_terminal(&self, direction: SearchDirection) -> Result<bool, String> {
        let Some(surface) = self.active_surface() else {
            return Ok(false);
        };

        let query = self.search_entry.text().to_string();
        if query.trim().is_empty() {
            surface.terminal().search_set_regex(None, 0);
            self.refresh_active_terminal_chrome();
            return Ok(true);
        }

        let regex = vte::Regex::for_search(escape_search_pattern(query.as_str()).as_str(), 0)
            .map_err(|error| error.to_string())?;
        surface.terminal().search_set_wrap_around(true);
        surface.terminal().search_set_regex(Some(&regex), 0);

        let found = match direction {
            SearchDirection::Next => surface.terminal().search_find_next(),
            SearchDirection::Previous => surface.terminal().search_find_previous(),
        };

        if found {
            self.refresh_active_terminal_chrome();
        } else {
            self.status_label
                .set_text(search_miss_status(query.as_str()).as_str());
        }

        Ok(found)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SidebarRepoGroup {
    group_id: String,
    name: String,
    repos: Vec<RepoWorkspace>,
    is_worktree_group: bool,
}

#[derive(Clone)]
struct SettingsDialogWidgets {
    default_shell_entry: gtk::Entry,
    font_family_entry: gtk::Entry,
    theme_combo: gtk::ComboBoxText,
    font_size: gtk::SpinButton,
    scrollback: gtk::SpinButton,
    git_poll: gtk::SpinButton,
    ui_scale: gtk::SpinButton,
}

const MAX_VISIBLE_SPLITS: usize = 6;

fn settings_dialog_config(base_config: &UserConfig, widgets: &SettingsDialogWidgets) -> UserConfig {
    UserConfig {
        default_shell: widgets.default_shell_entry.text().to_string(),
        font_family: widgets.font_family_entry.text().to_string(),
        font_size: widgets.font_size.value() as u32,
        scrollback_lines: widgets.scrollback.value() as u32,
        theme: widgets
            .theme_combo
            .active_id()
            .map(|value| value.to_string())
            .unwrap_or_else(|| normalized_native_theme_id(base_config.theme.as_str())),
        git_poll_interval_secs: widgets.git_poll.value() as u64,
        ui_scale: widgets.ui_scale.value(),
        terminal_latency_mode: base_config.terminal_latency_mode.clone(),
    }
}

fn sidebar_font_attrs(size_pt: u32) -> gtk::pango::AttrList {
    let attrs = gtk::pango::AttrList::new();
    let size = gtk::pango::AttrSize::new((size_pt as i32) * gtk::pango::SCALE);
    attrs.insert(size);
    attrs
}

struct SidebarGitMeta {
    text: String,
    use_markup: bool,
}

fn sidebar_git_meta(info: &git::GitInfo) -> SidebarGitMeta {
    let mut parts = Vec::new();
    if info.is_dirty {
        parts.push("M".to_string());
    }
    if let Some(branch) = info.branch.as_deref() {
        parts.push(glib_markup_escape(branch));
    }
    if info.ahead > 0 {
        parts.push(format!("↑{}", info.ahead));
    }
    if info.behind > 0 {
        parts.push(format!("↓{}", info.behind));
    }

    let mut use_markup = false;
    if info.insertions > 0 || info.deletions > 0 {
        use_markup = true;
        let mut stat_parts = Vec::new();
        if info.insertions > 0 {
            stat_parts.push(format!(
                "<span color='#50fa7b' weight='bold'>+{}</span>",
                info.insertions
            ));
        }
        if info.deletions > 0 {
            stat_parts.push(format!(
                "<span color='#ff6e6e' weight='bold'>−{}</span>",
                info.deletions
            ));
        }
        parts.push(stat_parts.join(" "));
    }

    SidebarGitMeta {
        text: parts.join("  "),
        use_markup,
    }
}

fn glib_markup_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sidebar_groups(repos: &[RepoWorkspace]) -> Vec<SidebarRepoGroup> {
    let mut groups: Vec<SidebarRepoGroup> = Vec::new();

    for repo in repos {
        let group_id = repo
            .repo
            .group_id
            .clone()
            .unwrap_or_else(|| format!("standalone-{}", repo.repo.id));

        if let Some(group) = groups.iter_mut().find(|group| group.group_id == group_id) {
            group.repos.push(repo.clone());
            continue;
        }

        groups.push(SidebarRepoGroup {
            group_id,
            name: repo.repo.name.clone(),
            repos: vec![repo.clone()],
            is_worktree_group: repo.repo.group_id.is_some(),
        });
    }

    for group in &mut groups {
        group.repos.sort_by(
            |left, right| match (left.repo.is_default, right.repo.is_default) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => left.repo.sort_order.cmp(&right.repo.sort_order),
            },
        );
        if let Some(default_repo) = group.repos.iter().find(|repo| repo.repo.is_default) {
            group.name = default_repo.repo.name.clone();
        }
    }

    groups
}

fn reordered_repo_ids_for_group_move(
    groups: &[SidebarRepoGroup],
    group_id: &str,
    direction: isize,
) -> Option<Vec<String>> {
    let current_index = groups.iter().position(|group| group.group_id == group_id)?;
    let target_index = current_index.checked_add_signed(direction)?;
    if target_index >= groups.len() {
        return None;
    }

    let mut reordered_groups = groups.to_vec();
    let moved_group = reordered_groups.remove(current_index);
    reordered_groups.insert(target_index, moved_group);

    Some(
        reordered_groups
            .into_iter()
            .flat_map(|group| group.repos.into_iter().map(|repo| repo.repo.id))
            .collect(),
    )
}

fn reordered_session_ids_for_tab_move(
    session_ids: &[String],
    active_session_id: &str,
    direction: isize,
) -> Option<Vec<String>> {
    let current_index = session_ids
        .iter()
        .position(|session_id| session_id == active_session_id)?;
    let target_index = current_index.checked_add_signed(direction)?;
    if target_index >= session_ids.len() {
        return None;
    }

    let mut reordered_session_ids = session_ids.to_vec();
    let moved_session_id = reordered_session_ids.remove(current_index);
    reordered_session_ids.insert(target_index, moved_session_id);
    Some(reordered_session_ids)
}

fn reordered_visible_session_ids_for_pane_move(
    visible_session_ids: &[String],
    active_session_id: &str,
    direction: isize,
) -> Option<Vec<String>> {
    reordered_session_ids_for_tab_move(visible_session_ids, active_session_id, direction)
}

fn normalize_split_state(
    split_state: &NativeSplitState,
    available_session_ids: &[String],
    active_session_id: Option<&str>,
) -> NativeSplitState {
    let visible_session_ids = normalize_visible_sessions(
        &split_state.visible_session_ids,
        available_session_ids,
        active_session_id,
    );
    NativeSplitState {
        visible_session_ids: visible_session_ids.clone(),
        orientation: split_state.orientation,
        divider_positions: split_state
            .divider_positions
            .iter()
            .copied()
            .take(visible_session_ids.len().saturating_sub(1))
            .collect(),
    }
}

fn normalize_visible_sessions(
    visible_sessions: &[String],
    available_session_ids: &[String],
    active_session_id: Option<&str>,
) -> Vec<String> {
    let mut normalized_visible_sessions = Vec::new();

    for session_id in visible_sessions {
        if available_session_ids
            .iter()
            .any(|available| available == session_id)
            && !normalized_visible_sessions.contains(session_id)
        {
            normalized_visible_sessions.push(session_id.clone());
        }
        if normalized_visible_sessions.len() == MAX_VISIBLE_SPLITS {
            break;
        }
    }

    if let Some(active_session_id) = active_session_id.filter(|active| {
        available_session_ids
            .iter()
            .any(|available| available == active)
    }) {
        if !normalized_visible_sessions
            .iter()
            .any(|session_id| session_id == active_session_id)
        {
            if normalized_visible_sessions.len() == MAX_VISIBLE_SPLITS {
                normalized_visible_sessions.pop();
            }
            normalized_visible_sessions.push(active_session_id.to_string());
        }
    }

    if normalized_visible_sessions.is_empty() {
        if let Some(active_session_id) = active_session_id {
            if available_session_ids
                .iter()
                .any(|available| available == active_session_id)
            {
                normalized_visible_sessions.push(active_session_id.to_string());
                return normalized_visible_sessions;
            }
        }

        if let Some(first_session_id) = available_session_ids.first() {
            normalized_visible_sessions.push(first_session_id.clone());
        }
    }

    normalized_visible_sessions
}

fn gtk_orientation(orientation: NativeSplitOrientation) -> gtk::Orientation {
    match orientation {
        NativeSplitOrientation::Horizontal => gtk::Orientation::Horizontal,
        NativeSplitOrientation::Vertical => gtk::Orientation::Vertical,
    }
}

fn toggled_split_orientation(orientation: NativeSplitOrientation) -> NativeSplitOrientation {
    match orientation {
        NativeSplitOrientation::Horizontal => NativeSplitOrientation::Vertical,
        NativeSplitOrientation::Vertical => NativeSplitOrientation::Horizontal,
    }
}

fn default_split_position(
    orientation: NativeSplitOrientation,
    window_width: i32,
    window_height: i32,
) -> i32 {
    match orientation {
        NativeSplitOrientation::Horizontal => (window_width / 2).max(320),
        NativeSplitOrientation::Vertical => (window_height / 2).max(180),
    }
}

fn apply_active_session_change(
    visible_sessions: &[String],
    _previous_active_session_id: Option<&str>,
    new_active_session_id: &str,
) -> Vec<String> {
    if visible_sessions
        .iter()
        .any(|session_id| session_id == new_active_session_id)
    {
        return visible_sessions.to_vec();
    }

    vec![new_active_session_id.to_string()]
}

fn append_split_session(visible_sessions: &[String], new_session_id: &str) -> Vec<String> {
    let mut updated_visible_sessions = visible_sessions.to_vec();
    if !updated_visible_sessions
        .iter()
        .any(|session_id| session_id == new_session_id)
    {
        updated_visible_sessions.push(new_session_id.to_string());
    }
    updated_visible_sessions.truncate(MAX_VISIBLE_SPLITS);
    updated_visible_sessions
}

fn remove_visible_session(
    visible_sessions: &[String],
    removed_session_id: &str,
    fallback_active_session_id: Option<&str>,
) -> Vec<String> {
    let mut updated_visible_sessions = visible_sessions
        .iter()
        .filter(|session_id| session_id.as_str() != removed_session_id)
        .cloned()
        .collect::<Vec<_>>();

    if updated_visible_sessions.is_empty() {
        if let Some(fallback_active_session_id) = fallback_active_session_id {
            updated_visible_sessions.push(fallback_active_session_id.to_string());
        }
    }

    updated_visible_sessions
}

fn next_visible_session_id(visible_sessions: &[String], active_session_id: &str) -> Option<String> {
    let active_index = visible_sessions
        .iter()
        .position(|session_id| session_id == active_session_id)?;
    if visible_sessions.len() <= 1 {
        return None;
    }

    Some(visible_sessions[wrapped_index(visible_sessions.len(), active_index, 1)].clone())
}

fn nth_visible_session_id(visible_sessions: &[String], index: usize) -> Option<String> {
    visible_sessions.get(index).cloned()
}

fn wrapped_index(len: usize, current_index: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let len = len as isize;
    (current_index as isize + direction).rem_euclid(len) as usize
}

fn normalized_session_title(raw_title: &str) -> Option<String> {
    let title = raw_title.trim();
    if title.is_empty() {
        return None;
    }

    Some(title.to_string())
}

fn next_session_title(existing_sessions: &[lantern_core::TerminalSession]) -> String {
    let max_n = existing_sessions
        .iter()
        .filter_map(|s| s.title.strip_prefix("Terminal "))
        .filter_map(|n| n.parse::<usize>().ok())
        .max()
        .unwrap_or(0);
    format!("Terminal {}", max_n + 1)
}

fn status_text(
    working_directory: &str,
    title: &str,
    exit_status: Option<i32>,
    git_meta: &str,
    process_info: Option<&git::ProcessInfo>,
    dimensions: Option<&str>,
    command_running: bool,
) -> String {
    let process_suffix = status_process_label(process_info, title)
        .map(|label| format!(" • {label}"))
        .unwrap_or_default();
    let git_suffix = if git_meta.is_empty() {
        String::new()
    } else {
        format!(" • {git_meta}")
    };
    let dimensions_suffix = dimensions
        .filter(|dimensions| !dimensions.is_empty())
        .map(|dimensions| format!(" • {dimensions}"))
        .unwrap_or_default();
    let running_suffix = if command_running { " • running" } else { "" };

    match exit_status {
        Some(status) => {
            format!(
                "{working_directory} • exited with status {status}{git_suffix}{dimensions_suffix}"
            )
        }
        None => {
            format!(
                "{working_directory} • {title}{process_suffix}{git_suffix}{running_suffix}{dimensions_suffix}"
            )
        }
    }
}

fn status_process_label(process_info: Option<&git::ProcessInfo>, title: &str) -> Option<String> {
    let process_info = process_info?;
    if process_info.is_agent {
        return process_info
            .agent_label
            .clone()
            .or_else(|| Some(process_info.name.clone()));
    }

    if process_info.name.eq_ignore_ascii_case(title.trim()) {
        return None;
    }

    Some(process_info.name.clone())
}

fn window_title(title: &str) -> String {
    format!("Lantern • {title}")
}

fn status_git_meta(info: &git::GitInfo) -> String {
    let meta = sidebar_git_meta(info);
    if meta.use_markup {
        // Strip Pango markup for plain-text status bar
        meta.text
            .replace("<span color='#a3be8c'>", "")
            .replace("<span color='#bf616a'>", "")
            .replace("</span>", "")
    } else {
        meta.text
    }
}

fn terminal_dimensions(terminal: &vte::Terminal) -> Option<String> {
    let columns = terminal.column_count();
    let rows = terminal.row_count();
    if columns <= 0 || rows <= 0 {
        return None;
    }

    Some(format!("{columns}x{rows}"))
}

fn launch_failure_status(working_directory: &str, error: &str) -> String {
    format!("{working_directory} • failed to start shell: {error}")
}

fn effective_git_poll_interval_secs(git_poll_interval_secs: u64) -> Option<u32> {
    if git_poll_interval_secs == 0 {
        return None;
    }

    Some(git_poll_interval_secs.min(u32::MAX as u64) as u32)
}

fn should_poll_process_info(command_running_state: Option<bool>) -> bool {
    command_running_state.unwrap_or(true)
}

fn signal_exists<T: gtk::prelude::ObjectType>(object: &T, signal_name: &str) -> bool {
    type_has_signal(object.type_(), signal_name)
}

fn type_has_signal(type_: gtk::glib::Type, signal_name: &str) -> bool {
    gtk::glib::subclass::SignalId::lookup(signal_name, type_).is_some()
}

fn relaunch_app() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
}

fn ui_scale_to_xft_dpi(ui_scale: f64) -> i32 {
    let clamped = ui_scale.clamp(0.5, 3.0);
    (96.0 * 1024.0 * clamped).round() as i32
}

fn search_miss_status(query: &str) -> String {
    format!("No matches for \"{query}\"")
}

fn escape_search_pattern(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for character in query.chars() {
        if matches!(
            character,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn labeled_entry(grid: &gtk::Grid, row: i32, label: &str, value: &str) -> gtk::Entry {
    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_hexpand(true);
    grid.attach(&label, 0, row, 1, 1);
    grid.attach(&entry, 1, row, 1, 1);
    entry
}

fn labeled_spin_button(
    grid: &gtk::Grid,
    row: i32,
    label: &str,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
) -> gtk::SpinButton {
    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    let spin_button = gtk::SpinButton::with_range(min, max, step);
    spin_button.set_value(value.clamp(min, max));
    spin_button.set_hexpand(true);
    grid.attach(&label, 0, row, 1, 1);
    grid.attach(&spin_button, 1, row, 1, 1);
    spin_button
}

fn session_id_for_tab_page(page: &adw::TabPage) -> Option<String> {
    let name = page.child().widget_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn find_tab_page_for_session(tab_view: &adw::TabView, session_id: &str) -> Option<adw::TabPage> {
    for i in 0..tab_view.n_pages() {
        let page = tab_view.nth_page(i);
        if page.child().widget_name() == session_id {
            return Some(page);
        }
    }
    None
}

fn clear_box_children<W: IsA<gtk::Widget>>(container: &W) {
    while let Some(child) = container.first_child() {
        child.unparent();
    }
}

fn detach_widget<W: IsA<gtk::Widget>>(widget: &W) {
    if widget.as_ref().parent().is_some() {
        widget.as_ref().unparent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_session_title_avoids_duplicates() {
        use lantern_core::TerminalSession;

        let make = |title: &str| TerminalSession {
            id: String::new(),
            repo_id: String::new(),
            title: title.to_string(),
            shell: None,
            sort_order: 0,
        };

        assert_eq!(next_session_title(&[]), "Terminal 1");
        assert_eq!(
            next_session_title(&[make("Terminal 1"), make("Terminal 3")]),
            "Terminal 4"
        );
        assert_eq!(
            next_session_title(&[make("Terminal 2")]),
            "Terminal 3"
        );
        assert_eq!(
            next_session_title(&[make("custom name")]),
            "Terminal 1"
        );
    }

    #[test]
    fn status_text_reports_live_terminal_title_when_running() {
        assert_eq!(
            status_text("/tmp/repo", "claude", None, "", None, None, false),
            "/tmp/repo • claude"
        );
    }

    #[test]
    fn status_text_reports_exit_code_after_process_exit() {
        assert_eq!(
            status_text("/tmp/repo", "Terminal 1", Some(130), "", None, None, false),
            "/tmp/repo • exited with status 130"
        );
    }

    #[test]
    fn status_text_appends_git_branch_when_available() {
        assert_eq!(
            status_text("/tmp/repo", "bash", None, "main", None, None, false),
            "/tmp/repo • bash • main"
        );
    }

    #[test]
    fn status_git_meta_includes_dirty_and_divergence() {
        assert_eq!(
            status_git_meta(&git::GitInfo {
                branch: Some("main".to_string()),
                is_dirty: true,
                detached: false,
                ahead: 2,
                behind: 1,
                insertions: 0,
                deletions: 0,
            }),
            "M  main  ↑2  ↓1"
        );
    }

    #[test]
    fn status_text_appends_agent_label_when_detected() {
        assert_eq!(
            status_text(
                "/tmp/repo",
                "claude",
                None,
                "",
                Some(&git::ProcessInfo {
                    name: "claude".to_string(),
                    is_agent: true,
                    agent_label: Some("Claude Code".to_string()),
                }),
                None,
                false,
            ),
            "/tmp/repo • claude • Claude Code"
        );
    }

    #[test]
    fn status_text_appends_terminal_dimensions_when_available() {
        assert_eq!(
            status_text("/tmp/repo", "bash", None, "", None, Some("120x32"), false),
            "/tmp/repo • bash • 120x32"
        );
    }

    #[test]
    fn status_text_appends_running_state_when_command_is_active() {
        assert_eq!(
            status_text("/tmp/repo", "bash", None, "", None, None, true),
            "/tmp/repo • bash • running"
        );
    }

    #[test]
    fn effective_git_poll_interval_secs_disables_zero() {
        assert_eq!(effective_git_poll_interval_secs(0), None);
        assert_eq!(effective_git_poll_interval_secs(5), Some(5));
    }

    #[test]
    fn launch_failure_status_includes_working_directory_and_error() {
        assert_eq!(
            launch_failure_status("/tmp/repo", "No such file or directory"),
            "/tmp/repo • failed to start shell: No such file or directory"
        );
    }

    #[test]
    fn should_poll_process_info_only_skips_when_shell_state_reports_idle() {
        assert!(should_poll_process_info(None));
        assert!(should_poll_process_info(Some(true)));
        assert!(!should_poll_process_info(Some(false)));
    }

    #[test]
    fn type_has_signal_returns_false_for_unknown_vte_signal() {
        assert!(!type_has_signal(
            vte::Terminal::static_type(),
            "definitely-not-a-vte-signal"
        ));
    }

    #[test]
    fn status_process_label_avoids_duplicate_non_agent_titles() {
        assert_eq!(
            status_process_label(
                Some(&git::ProcessInfo {
                    name: "bash".to_string(),
                    is_agent: false,
                    agent_label: None,
                }),
                "bash",
            ),
            None
        );
    }

    #[test]
    fn escape_search_pattern_treats_query_as_literal_text() {
        assert_eq!(
            escape_search_pattern(r".*foo?(bar)[baz]\path"),
            r"\.\*foo\?\(bar\)\[baz\]\\path"
        );
    }

    #[test]
    fn search_miss_status_includes_query() {
        assert_eq!(search_miss_status("needle"), "No matches for \"needle\"");
    }

    #[test]
    fn normalize_visible_sessions_keeps_active_session_visible() {
        assert_eq!(
            normalize_visible_sessions(
                &["tab-1".to_string(), "tab-2".to_string()],
                &[
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string()
                ],
                Some("tab-3"),
            ),
            vec![
                "tab-1".to_string(),
                "tab-2".to_string(),
                "tab-3".to_string()
            ]
        );
    }

    #[test]
    fn normalize_split_state_preserves_orientation() {
        assert_eq!(
            normalize_split_state(
                &NativeSplitState {
                    visible_session_ids: vec!["tab-1".to_string()],
                    orientation: NativeSplitOrientation::Vertical,
                    divider_positions: vec![360, 180],
                },
                &["tab-1".to_string(), "tab-2".to_string()],
                Some("tab-2"),
            ),
            NativeSplitState {
                visible_session_ids: vec!["tab-1".to_string(), "tab-2".to_string()],
                orientation: NativeSplitOrientation::Vertical,
                divider_positions: vec![360],
            }
        );
    }

    #[test]
    fn normalize_split_state_truncates_extra_divider_positions() {
        assert_eq!(
            normalize_split_state(
                &NativeSplitState {
                    visible_session_ids: vec![
                        "tab-1".to_string(),
                        "tab-2".to_string(),
                        "tab-3".to_string(),
                    ],
                    orientation: NativeSplitOrientation::Horizontal,
                    divider_positions: vec![500, 320, 120],
                },
                &[
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                Some("tab-3"),
            ),
            NativeSplitState {
                visible_session_ids: vec![
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                orientation: NativeSplitOrientation::Horizontal,
                divider_positions: vec![500, 320],
            }
        );
    }

    #[test]
    fn apply_active_session_change_collapses_split_for_new_session() {
        assert_eq!(
            apply_active_session_change(
                &["tab-1".to_string(), "tab-2".to_string()],
                Some("tab-2"),
                "tab-3",
            ),
            vec!["tab-3".to_string()]
        );
    }

    #[test]
    fn remove_visible_session_falls_back_to_remaining_session() {
        assert_eq!(
            remove_visible_session(
                &["tab-1".to_string(), "tab-2".to_string()],
                "tab-2",
                Some("tab-1"),
            ),
            vec!["tab-1".to_string()]
        );
    }

    #[test]
    fn default_split_position_uses_height_for_vertical_splits() {
        assert_eq!(
            default_split_position(NativeSplitOrientation::Vertical, 1200, 900),
            450
        );
    }

    #[test]
    fn default_split_position_uses_width_for_horizontal_splits() {
        assert_eq!(
            default_split_position(NativeSplitOrientation::Horizontal, 1200, 900),
            600
        );
    }

    #[test]
    fn toggled_split_orientation_switches_axes() {
        assert_eq!(
            toggled_split_orientation(NativeSplitOrientation::Horizontal),
            NativeSplitOrientation::Vertical
        );
        assert_eq!(
            toggled_split_orientation(NativeSplitOrientation::Vertical),
            NativeSplitOrientation::Horizontal
        );
    }

    #[test]
    fn next_visible_session_id_cycles_to_the_next_pane() {
        assert_eq!(
            next_visible_session_id(
                &[
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string()
                ],
                "tab-1",
            ),
            Some("tab-2".to_string())
        );
        assert_eq!(
            next_visible_session_id(
                &[
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string()
                ],
                "tab-3",
            ),
            Some("tab-1".to_string())
        );
    }

    #[test]
    fn next_visible_session_id_returns_none_for_single_pane() {
        assert_eq!(
            next_visible_session_id(&["tab-1".to_string()], "tab-1"),
            None
        );
    }

    #[test]
    fn nth_visible_session_id_returns_requested_pane() {
        assert_eq!(
            nth_visible_session_id(
                &[
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                1,
            ),
            Some("tab-2".to_string())
        );
    }

    #[test]
    fn nth_visible_session_id_returns_none_when_pane_is_missing() {
        assert_eq!(nth_visible_session_id(&["tab-1".to_string()], 2), None);
    }

    #[test]
    fn wrapped_index_cycles_forward() {
        assert_eq!(wrapped_index(3, 2, 1), 0);
    }

    #[test]
    fn wrapped_index_cycles_backward() {
        assert_eq!(wrapped_index(3, 0, -1), 2);
    }

    #[test]
    fn normalized_session_title_trims_whitespace() {
        assert_eq!(
            normalized_session_title("  Logs  "),
            Some("Logs".to_string())
        );
    }

    #[test]
    fn normalized_session_title_rejects_empty_values() {
        assert_eq!(normalized_session_title("   "), None);
    }

    #[test]
    fn sidebar_groups_keep_worktree_members_together() {
        let groups = sidebar_groups(&[
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "main".to_string(),
                    name: "main".to_string(),
                    path: "/tmp/main".to_string(),
                    sort_order: 0,
                    group_id: Some("group-1".to_string()),
                    is_default: true,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "feature".to_string(),
                    name: "feature".to_string(),
                    path: "/tmp/feature".to_string(),
                    sort_order: 1,
                    group_id: Some("group-1".to_string()),
                    is_default: false,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "solo".to_string(),
                    name: "solo".to_string(),
                    path: "/tmp/solo".to_string(),
                    sort_order: 2,
                    group_id: None,
                    is_default: false,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
        ]);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].group_id, "group-1");
        assert_eq!(groups[0].name, "main");
        assert_eq!(
            groups[0]
                .repos
                .iter()
                .map(|repo| repo.repo.id.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "feature"]
        );
        assert_eq!(groups[1].group_id, "standalone-solo");
    }

    #[test]
    fn sidebar_git_meta_formats_branch_dirty_and_divergence() {
        let meta = sidebar_git_meta(&git::GitInfo {
            branch: Some("feat/auth".to_string()),
            is_dirty: true,
            detached: false,
            ahead: 2,
            behind: 1,
            insertions: 0,
            deletions: 0,
        });

        assert_eq!(meta.text, "M  feat/auth  ↑2  ↓1");
        assert!(!meta.use_markup);
    }

    #[test]
    fn sidebar_git_meta_includes_diff_stats_with_markup() {
        let meta = sidebar_git_meta(&git::GitInfo {
            branch: Some("main".to_string()),
            is_dirty: true,
            detached: false,
            ahead: 0,
            behind: 0,
            insertions: 42,
            deletions: 7,
        });

        assert!(meta.use_markup);
        assert!(meta.text.contains("+42"));
        assert!(meta.text.contains("−7"));
    }

    #[test]
    fn sidebar_git_meta_omits_empty_values() {
        let meta = sidebar_git_meta(&git::GitInfo::default());
        assert!(meta.text.is_empty());
    }

    #[test]
    fn status_git_meta_prefers_branch_name() {
        assert_eq!(
            status_git_meta(&git::GitInfo {
                branch: Some("main".to_string()),
                ..git::GitInfo::default()
            }),
            "main"
        );
    }

    #[test]
    fn ui_scale_to_xft_dpi_uses_ninety_six_dpi_baseline() {
        assert_eq!(ui_scale_to_xft_dpi(1.0), 98304);
        assert_eq!(ui_scale_to_xft_dpi(1.25), 122880);
    }

    #[test]
    fn normalized_native_theme_id_keeps_supported_theme_ids() {
        assert_eq!(
            normalized_native_theme_id("github-light"),
            "github-light".to_string()
        );
    }

    #[test]
    fn normalized_native_theme_id_maps_legacy_values_to_supported_themes() {
        assert_eq!(
            normalized_native_theme_id("light"),
            "github-light".to_string()
        );
        assert_eq!(normalized_native_theme_id("dark"), "nord-dark".to_string());
        assert_eq!(
            normalized_native_theme_id("custom-theme"),
            "nord-dark".to_string()
        );
    }

    #[test]
    fn theme_color_scheme_handles_named_and_suffixed_modes() {
        assert_eq!(theme_color_scheme("light"), adw::ColorScheme::ForceLight);
        assert_eq!(theme_color_scheme("dark"), adw::ColorScheme::ForceDark);
        assert_eq!(
            theme_color_scheme("github-light"),
            adw::ColorScheme::ForceLight
        );
        assert_eq!(theme_color_scheme("nord-dark"), adw::ColorScheme::ForceDark);
        assert_eq!(theme_color_scheme("system"), adw::ColorScheme::Default);
    }

    #[test]
    fn reordered_repo_ids_for_group_move_moves_group_as_a_block() {
        let groups = sidebar_groups(&[
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "repo-1".to_string(),
                    name: "repo-1".to_string(),
                    path: "/tmp/repo-1".to_string(),
                    sort_order: 0,
                    group_id: None,
                    is_default: false,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "main".to_string(),
                    name: "main".to_string(),
                    path: "/tmp/main".to_string(),
                    sort_order: 1,
                    group_id: Some("group-1".to_string()),
                    is_default: true,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
            RepoWorkspace {
                repo: lantern_core::Repo {
                    id: "feature".to_string(),
                    name: "feature".to_string(),
                    path: "/tmp/feature".to_string(),
                    sort_order: 2,
                    group_id: Some("group-1".to_string()),
                    is_default: false,
                },
                sessions: Vec::new(),
                active_session_id: None,
            },
        ]);

        let reordered_repo_ids = reordered_repo_ids_for_group_move(&groups, "group-1", -1).unwrap();
        assert_eq!(
            reordered_repo_ids,
            vec![
                "main".to_string(),
                "feature".to_string(),
                "repo-1".to_string(),
            ]
        );
    }

    #[test]
    fn reordered_repo_ids_for_group_move_returns_none_at_edges() {
        let groups = vec![SidebarRepoGroup {
            group_id: "only".to_string(),
            name: "only".to_string(),
            repos: Vec::new(),
            is_worktree_group: false,
        }];

        assert_eq!(reordered_repo_ids_for_group_move(&groups, "only", -1), None);
    }

    #[test]
    fn reordered_session_ids_for_tab_move_moves_active_tab_left_and_right() {
        let session_ids = vec![
            "tab-1".to_string(),
            "tab-2".to_string(),
            "tab-3".to_string(),
        ];

        assert_eq!(
            reordered_session_ids_for_tab_move(&session_ids, "tab-2", -1).unwrap(),
            vec![
                "tab-2".to_string(),
                "tab-1".to_string(),
                "tab-3".to_string(),
            ]
        );
        assert_eq!(
            reordered_session_ids_for_tab_move(&session_ids, "tab-2", 1).unwrap(),
            vec![
                "tab-1".to_string(),
                "tab-3".to_string(),
                "tab-2".to_string(),
            ]
        );
    }

    #[test]
    fn reordered_session_ids_for_tab_move_returns_none_at_edges() {
        let session_ids = vec!["tab-1".to_string(), "tab-2".to_string()];

        assert_eq!(
            reordered_session_ids_for_tab_move(&session_ids, "tab-1", -1),
            None
        );
        assert_eq!(
            reordered_session_ids_for_tab_move(&session_ids, "tab-2", 1),
            None
        );
    }

    #[test]
    fn reordered_visible_session_ids_for_pane_move_swaps_adjacent_panes() {
        let visible_session_ids = vec![
            "tab-1".to_string(),
            "tab-2".to_string(),
            "tab-3".to_string(),
        ];

        assert_eq!(
            reordered_visible_session_ids_for_pane_move(&visible_session_ids, "tab-2", -1),
            Some(vec![
                "tab-2".to_string(),
                "tab-1".to_string(),
                "tab-3".to_string(),
            ])
        );
        assert_eq!(
            reordered_visible_session_ids_for_pane_move(&visible_session_ids, "tab-2", 1),
            Some(vec![
                "tab-1".to_string(),
                "tab-3".to_string(),
                "tab-2".to_string(),
            ])
        );
    }
}
