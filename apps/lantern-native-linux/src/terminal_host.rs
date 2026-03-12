use crate::theme;
use gtk::prelude::*;
use lantern_core::{RepoWorkspace, TerminalSession, UserConfig};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use vte::prelude::*;

#[derive(Clone)]
pub struct TerminalSurface {
    fallback_title: Rc<RefCell<String>>,
    fallback_working_directory: String,
    exit_status: Rc<Cell<Option<i32>>>,
    child_pid: Rc<Cell<Option<u32>>>,
    launch_error: Rc<RefCell<Option<String>>>,
    terminal: vte::Terminal,
}

impl TerminalSurface {
    fn new(session: &TerminalSession, repo: &RepoWorkspace, config: &UserConfig) -> Self {
        let terminal = vte::Terminal::new();
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);
        terminal.set_scrollback_lines(config.scrollback_lines as i64);

        let font = gtk::pango::FontDescription::from_string(&format!(
            "{} {}",
            config.font_family, config.font_size
        ));
        terminal.set_font_desc(Some(&font));
        theme::apply_terminal_theme(
            &terminal,
            config.theme.as_str(),
            theme::theme_is_dark(config.theme.as_str()),
        );

        let exit_status = Rc::new(Cell::new(None));
        let exit_status_handle = exit_status.clone();
        terminal.connect_child_exited(move |_, status| {
            exit_status_handle.set(Some(status));
        });
        let child_pid = Rc::new(Cell::new(None));
        let launch_error = Rc::new(RefCell::new(None));

        let shell = resolved_shell(session, config);
        let argv = [shell];
        let terminal_for_spawn = terminal.clone();
        let child_pid_handle = child_pid.clone();
        let launch_error_handle = launch_error.clone();

        terminal.spawn_async(
            vte::PtyFlags::DEFAULT,
            Some(repo.repo.path.as_str()),
            &argv,
            &[],
            gtk::glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            None::<&gtk::gio::Cancellable>,
            move |result| match result {
                Ok(pid) => {
                    launch_error_handle.replace(None);
                    child_pid_handle.set(u32::try_from(pid.0).ok());
                    terminal_for_spawn.watch_child(pid);
                }
                Err(error) => {
                    let message = error.to_string();
                    launch_error_handle.replace(Some(message.clone()));
                    terminal_for_spawn
                        .feed(format!("Failed to start shell: {message}\r\n").as_bytes());
                    eprintln!("Failed to spawn shell for native terminal: {error}");
                }
            },
        );

        Self {
            fallback_title: Rc::new(RefCell::new(session.title.clone())),
            fallback_working_directory: repo.repo.path.clone(),
            exit_status,
            child_pid,
            launch_error,
            terminal,
        }
    }

    pub fn title(&self) -> String {
        self.terminal
            .window_title()
            .map(|title| title.to_string())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| self.fallback_title.borrow().clone())
    }

    pub fn working_directory(&self) -> String {
        self.terminal
            .current_directory_uri()
            .and_then(|uri| file_path_from_uri(uri.as_str()))
            .unwrap_or_else(|| self.fallback_working_directory.clone())
    }

    pub fn exit_status(&self) -> Option<i32> {
        self.exit_status.get()
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid.get()
    }

    pub fn launch_error(&self) -> Option<String> {
        self.launch_error.borrow().clone()
    }

    pub fn terminal(&self) -> &vte::Terminal {
        &self.terminal
    }

    pub fn set_fallback_title(&self, title: &str) {
        self.fallback_title.replace(title.to_string());
    }

    pub fn apply_config(&self, config: &UserConfig) {
        self.terminal
            .set_scrollback_lines(config.scrollback_lines as i64);
        let font = gtk::pango::FontDescription::from_string(&format!(
            "{} {}",
            config.font_family, config.font_size
        ));
        self.terminal.set_font_desc(Some(&font));
        theme::apply_terminal_theme(
            &self.terminal,
            config.theme.as_str(),
            theme::theme_is_dark(config.theme.as_str()),
        );
    }
}

pub trait TerminalHost {
    fn ensure_surface(
        &mut self,
        repo: &RepoWorkspace,
        session: &TerminalSession,
        config: &UserConfig,
    ) -> TerminalSurface;
}

#[derive(Default)]
pub struct VteTerminalHost {
    surfaces: HashMap<String, TerminalSurface>,
}

impl VteTerminalHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn surface(&self, session_id: &str) -> Option<TerminalSurface> {
        self.surfaces.get(session_id).cloned()
    }

    pub fn remove_surface(&mut self, session_id: &str) -> Option<TerminalSurface> {
        self.surfaces.remove(session_id)
    }

    pub fn surfaces(&self) -> Vec<TerminalSurface> {
        self.surfaces.values().cloned().collect()
    }
}

impl TerminalHost for VteTerminalHost {
    fn ensure_surface(
        &mut self,
        repo: &RepoWorkspace,
        session: &TerminalSession,
        config: &UserConfig,
    ) -> TerminalSurface {
        if let Some(surface) = self.surfaces.get(session.id.as_str()) {
            return surface.clone();
        }

        let surface = TerminalSurface::new(session, repo, config);
        self.surfaces.insert(session.id.clone(), surface.clone());
        surface
    }
}

fn resolved_shell<'a>(session: &'a TerminalSession, config: &'a UserConfig) -> &'a str {
    session
        .shell
        .as_deref()
        .filter(|shell| !shell.is_empty())
        .unwrap_or(config.default_shell.as_str())
}

fn file_path_from_uri(uri: &str) -> Option<String> {
    gtk::gio::File::for_uri(uri)
        .path()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_shell_prefers_session_override() {
        let session = TerminalSession {
            id: "tab-1".to_string(),
            repo_id: "repo-1".to_string(),
            title: "Terminal 1".to_string(),
            shell: Some("/bin/bash".to_string()),
            sort_order: 0,
        };
        let config = UserConfig::default();
        assert_eq!(resolved_shell(&session, &config), "/bin/bash");
    }

    #[test]
    fn resolved_shell_falls_back_to_default_shell() {
        let session = TerminalSession {
            id: "tab-1".to_string(),
            repo_id: "repo-1".to_string(),
            title: "Terminal 1".to_string(),
            shell: None,
            sort_order: 0,
        };
        let config = UserConfig {
            default_shell: "/bin/zsh".to_string(),
            ..UserConfig::default()
        };

        assert_eq!(resolved_shell(&session, &config), "/bin/zsh");
    }

    #[test]
    fn file_path_from_file_uri_returns_local_path() {
        assert_eq!(
            file_path_from_uri("file:///tmp/lantern"),
            Some("/tmp/lantern".to_string())
        );
    }

    #[test]
    fn file_path_from_non_file_uri_returns_none() {
        assert_eq!(file_path_from_uri("ssh://example.com/home/user"), None);
    }
}
