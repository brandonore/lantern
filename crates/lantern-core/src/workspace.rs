use crate::db::{self, DbConn};
use crate::error::LanternError;
use crate::models::{
    AppLayout, RepoWorkspace, TabWorkspace, TerminalSession, TerminalTab, WorkspaceSnapshot,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceState {
    pub repos: Vec<RepoWorkspace>,
    pub active_repo_id: Option<String>,
    pub layout: AppLayout,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            active_repo_id: None,
            layout: AppLayout::default(),
        }
    }
}

impl WorkspaceState {
    pub fn load(conn: &DbConn) -> Result<Self, LanternError> {
        let repos = db::list_repos(conn)?
            .into_iter()
            .map(|repo| {
                let mut tabs = db::list_tabs(conn, &repo.id)?
                    .into_iter()
                    .map(|mut tab| {
                        let sessions = db::list_sessions(conn, &tab.id)?;
                        tab.active_session_id =
                            normalize_active_session_id(&sessions, tab.active_session_id.clone());

                        if let Some(active_session_id) = tab.active_session_id.as_deref() {
                            db::set_active_session(conn, &tab.id, active_session_id)?;
                        }

                        Ok(TabWorkspace { tab, sessions })
                    })
                    .collect::<Result<Vec<_>, LanternError>>()?;

                sort_tabs(&mut tabs);
                let active_tab_id = normalize_active_tab_id(&tabs, db::get_active_tab(conn, &repo.id)?);

                if let Some(active_tab_id) = active_tab_id.as_deref() {
                    db::set_active_tab(conn, &repo.id, active_tab_id)?;
                }

                Ok(RepoWorkspace {
                    repo,
                    tabs,
                    active_tab_id,
                })
            })
            .collect::<Result<Vec<_>, LanternError>>()?;

        let snapshot = WorkspaceSnapshot {
            repos,
            layout: db::load_layout(conn)?.unwrap_or_default(),
        };

        Ok(Self::from_snapshot(snapshot))
    }

    pub fn from_snapshot(snapshot: WorkspaceSnapshot) -> Self {
        let mut repos = snapshot.repos;
        for repo in &mut repos {
            for tab in &mut repo.tabs {
                tab.tab.active_session_id =
                    normalize_active_session_id(&tab.sessions, tab.tab.active_session_id.clone());
                sort_sessions(&mut tab.sessions);
            }
            sort_tabs(&mut repo.tabs);
            repo.active_tab_id = normalize_active_tab_id(&repo.tabs, repo.active_tab_id.clone());
        }
        sort_repos(&mut repos);

        let active_repo_id =
            normalize_active_repo_id(&repos, snapshot.layout.active_repo_id.clone());
        let mut layout = snapshot.layout;
        layout.active_repo_id = active_repo_id.clone();

        Self {
            repos,
            active_repo_id,
            layout,
        }
    }

    pub fn active_repo(&self) -> Option<&RepoWorkspace> {
        self.active_repo_id
            .as_ref()
            .and_then(|repo_id| self.repos.iter().find(|repo| repo.repo.id == *repo_id))
            .or_else(|| self.repos.first())
    }

    pub fn active_tab(&self) -> Option<&TabWorkspace> {
        let repo = self.active_repo()?;
        repo.active_tab_id
            .as_ref()
            .and_then(|tab_id| repo.tabs.iter().find(|tab| tab.tab.id == *tab_id))
            .or_else(|| repo.tabs.first())
    }

    pub fn active_tab_id(&self) -> Option<&str> {
        self.active_tab().map(|tab| tab.tab.id.as_str())
    }

    pub fn active_session_id(&self) -> Option<&str> {
        self.active_tab()
            .and_then(|tab| tab.tab.active_session_id.as_deref())
    }

    pub fn set_active_repo(&mut self, repo_id: &str) {
        if self.repos.iter().any(|repo| repo.repo.id == repo_id) {
            self.active_repo_id = Some(repo_id.to_string());
            self.layout.active_repo_id = self.active_repo_id.clone();
        }
    }

    pub fn set_active_tab(&mut self, repo_id: &str, tab_id: &str) {
        let mut should_select_repo = false;
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            if repo.tabs.iter().any(|tab| tab.tab.id == tab_id) {
                repo.active_tab_id = Some(tab_id.to_string());
                should_select_repo = true;
            }
        }
        if should_select_repo {
            self.set_active_repo(repo_id);
        }
    }

    pub fn set_active_session(&mut self, repo_id: &str, tab_id: &str, session_id: &str) {
        let mut should_select_repo = false;
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            if let Some(tab) = repo.tabs.iter_mut().find(|tab| tab.tab.id == tab_id) {
                if tab.sessions.iter().any(|session| session.id == session_id) {
                    tab.tab.active_session_id = Some(session_id.to_string());
                    repo.active_tab_id = Some(tab_id.to_string());
                    should_select_repo = true;
                }
            }
        }
        if should_select_repo {
            self.set_active_repo(repo_id);
        }
    }

    pub fn add_repo(&mut self, repo: crate::models::Repo) {
        if self.repos.iter().any(|existing| existing.repo.id == repo.id) {
            return;
        }

        self.repos.push(RepoWorkspace {
            repo: repo.clone(),
            tabs: Vec::new(),
            active_tab_id: None,
        });
        sort_repos(&mut self.repos);
        self.set_active_repo(repo.id.as_str());
    }

    pub fn update_repo(&mut self, repo: crate::models::Repo) {
        if let Some(existing) = self.repos.iter_mut().find(|r| r.repo.id == repo.id) {
            existing.repo = repo;
            sort_repos(&mut self.repos);
        }
    }

    pub fn remove_repo(&mut self, repo_id: &str) {
        self.repos.retain(|repo| repo.repo.id != repo_id);
        sort_repos(&mut self.repos);
        self.active_repo_id = normalize_active_repo_id(&self.repos, self.active_repo_id.clone());
        self.layout.active_repo_id = self.active_repo_id.clone();
    }

    pub fn add_tab(&mut self, tab: TerminalTab) {
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == tab.repo_id) {
            if repo.tabs.iter().any(|existing| existing.tab.id == tab.id) {
                return;
            }
            repo.tabs.push(TabWorkspace {
                tab: tab.clone(),
                sessions: Vec::new(),
            });
            sort_tabs(&mut repo.tabs);
            repo.active_tab_id = Some(tab.id.clone());
            self.set_active_repo(tab.repo_id.as_str());
        }
    }

    pub fn close_tab(&mut self, repo_id: &str, tab_id: &str) {
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            repo.tabs.retain(|tab| tab.tab.id != tab_id);
            sort_tabs(&mut repo.tabs);
            repo.active_tab_id = normalize_active_tab_id(&repo.tabs, repo.active_tab_id.clone());
        }
    }

    pub fn rename_tab(&mut self, repo_id: &str, tab_id: &str, title: &str) {
        if let Some(tab) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.tabs.iter_mut().find(|tab| tab.tab.id == tab_id))
        {
            tab.tab.title = title.to_string();
        }
    }

    pub fn reorder_tabs(&mut self, repo_id: &str, tab_ids: &[String]) {
        let sort_order_by_tab_id = tab_ids
            .iter()
            .enumerate()
            .map(|(sort_order, tab_id)| (tab_id.as_str(), sort_order as i32))
            .collect::<HashMap<_, _>>();

        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            for tab in &mut repo.tabs {
                if let Some(sort_order) = sort_order_by_tab_id.get(tab.tab.id.as_str()) {
                    tab.tab.sort_order = *sort_order;
                }
            }
            sort_tabs(&mut repo.tabs);
        }
    }

    pub fn add_session(&mut self, session: TerminalSession) {
        let mut should_select = false;
        if let Some(tab) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == session.repo_id)
            .and_then(|repo| repo.tabs.iter_mut().find(|tab| tab.tab.id == session.tab_id))
        {
            if tab.sessions.iter().any(|existing| existing.id == session.id) {
                return;
            }
            tab.tab.active_session_id = Some(session.id.clone());
            tab.sessions.push(session.clone());
            sort_sessions(&mut tab.sessions);
            should_select = true;
        }
        if should_select {
            self.set_active_session(
                session.repo_id.as_str(),
                session.tab_id.as_str(),
                session.id.as_str(),
            );
        }
    }

    pub fn close_session(&mut self, repo_id: &str, tab_id: &str, session_id: &str) {
        if let Some(tab) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.tabs.iter_mut().find(|tab| tab.tab.id == tab_id))
        {
            tab.sessions.retain(|session| session.id != session_id);
            sort_sessions(&mut tab.sessions);
            tab.tab.active_session_id =
                normalize_active_session_id(&tab.sessions, tab.tab.active_session_id.clone());
        }
    }

    pub fn rename_session(&mut self, repo_id: &str, tab_id: &str, session_id: &str, title: &str) {
        if let Some(session) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.tabs.iter_mut().find(|tab| tab.tab.id == tab_id))
            .and_then(|tab| tab.sessions.iter_mut().find(|session| session.id == session_id))
        {
            session.title = title.to_string();
        }
    }

    pub fn reorder_sessions(&mut self, repo_id: &str, tab_id: &str, session_ids: &[String]) {
        let sort_order_by_session_id = session_ids
            .iter()
            .enumerate()
            .map(|(sort_order, session_id)| (session_id.as_str(), sort_order as i32))
            .collect::<HashMap<_, _>>();

        if let Some(tab) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| repo.tabs.iter_mut().find(|tab| tab.tab.id == tab_id))
        {
            for session in &mut tab.sessions {
                if let Some(sort_order) = sort_order_by_session_id.get(session.id.as_str()) {
                    session.sort_order = *sort_order;
                }
            }
            sort_sessions(&mut tab.sessions);
        }
    }

    pub fn reorder_repos(&mut self, repo_ids: &[String]) {
        let sort_order_by_repo_id = repo_ids
            .iter()
            .enumerate()
            .map(|(sort_order, repo_id)| (repo_id.as_str(), sort_order as i32))
            .collect::<HashMap<_, _>>();

        for repo in &mut self.repos {
            if let Some(sort_order) = sort_order_by_repo_id.get(repo.repo.id.as_str()) {
                repo.repo.sort_order = *sort_order;
            }
        }

        sort_repos(&mut self.repos);
    }
}

fn normalize_active_repo_id(
    repos: &[RepoWorkspace],
    active_repo_id: Option<String>,
) -> Option<String> {
    if repos.is_empty() {
        return None;
    }

    active_repo_id
        .filter(|active_repo_id| repos.iter().any(|repo| repo.repo.id == *active_repo_id))
        .or_else(|| repos.first().map(|repo| repo.repo.id.clone()))
}

fn normalize_active_tab_id(tabs: &[TabWorkspace], active_tab_id: Option<String>) -> Option<String> {
    if tabs.is_empty() {
        return None;
    }

    active_tab_id
        .filter(|active_tab_id| tabs.iter().any(|tab| tab.tab.id == *active_tab_id))
        .or_else(|| tabs.first().map(|tab| tab.tab.id.clone()))
}

fn normalize_active_session_id(
    sessions: &[TerminalSession],
    active_session_id: Option<String>,
) -> Option<String> {
    if sessions.is_empty() {
        return None;
    }

    active_session_id
        .filter(|active_session_id| {
            sessions
                .iter()
                .any(|session| session.id == *active_session_id)
        })
        .or_else(|| sessions.first().map(|session| session.id.clone()))
}

fn sort_tabs(tabs: &mut [TabWorkspace]) {
    tabs.sort_by(|left, right| {
        left.tab
            .sort_order
            .cmp(&right.tab.sort_order)
            .then_with(|| left.tab.title.cmp(&right.tab.title))
    });
}

fn sort_sessions(sessions: &mut [TerminalSession]) {
    sessions.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.title.cmp(&right.title))
    });
}

fn sort_repos(repos: &mut [RepoWorkspace]) {
    let mut group_min_sort_order = HashMap::new();
    for repo in repos.iter() {
        if let Some(group_id) = repo.repo.group_id.as_deref() {
            group_min_sort_order
                .entry(group_id.to_string())
                .and_modify(|min_sort_order: &mut i32| {
                    *min_sort_order = (*min_sort_order).min(repo.repo.sort_order);
                })
                .or_insert(repo.repo.sort_order);
        }
    }

    repos.sort_by(|left, right| {
        repo_sort_key(&group_min_sort_order, left).cmp(&repo_sort_key(&group_min_sort_order, right))
    });
}

fn repo_sort_key<'a>(
    group_min_sort_order: &'a HashMap<String, i32>,
    repo: &'a RepoWorkspace,
) -> (i32, i32, i32, &'a str) {
    let group_sort_order = repo
        .repo
        .group_id
        .as_deref()
        .and_then(|group_id| group_min_sort_order.get(group_id))
        .copied()
        .unwrap_or(repo.repo.sort_order);

    (
        group_sort_order,
        i32::from(!repo.repo.is_default),
        repo.repo.sort_order,
        repo.repo.name.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Repo, TerminalSession, TerminalTab};

    fn repo(
        id: &str,
        tabs: &[(&str, &[&str], Option<&str>)],
        active_tab_id: Option<&str>,
    ) -> RepoWorkspace {
        RepoWorkspace {
            repo: Repo {
                id: id.to_string(),
                name: id.to_string(),
                path: format!("/tmp/{id}"),
                sort_order: 0,
                group_id: None,
                is_default: false,
            },
            tabs: tabs
                .iter()
                .enumerate()
                .map(|(tab_index, (tab_id, sessions, active_session_id))| TabWorkspace {
                    tab: TerminalTab {
                        id: (*tab_id).to_string(),
                        repo_id: id.to_string(),
                        title: format!("Tab {}", tab_index + 1),
                        sort_order: tab_index as i32,
                        active_session_id: active_session_id.map(|id| id.to_string()),
                    },
                    sessions: sessions
                        .iter()
                        .enumerate()
                        .map(|(session_index, session_id)| TerminalSession {
                            id: (*session_id).to_string(),
                            repo_id: id.to_string(),
                            tab_id: (*tab_id).to_string(),
                            title: (*session_id).to_string(),
                            shell: None,
                            sort_order: session_index as i32,
                        })
                        .collect(),
                })
                .collect(),
            active_tab_id: active_tab_id.map(|id| id.to_string()),
        }
    }

    #[test]
    fn invalid_active_tab_falls_back_to_first_tab() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[("tab-1", &["session-1"], Some("session-1"))],
                Some("missing"),
            )],
            layout: AppLayout::default(),
        });

        assert_eq!(state.active_tab_id(), Some("tab-1"));
        assert_eq!(state.active_session_id(), Some("session-1"));
    }

    #[test]
    fn invalid_active_session_falls_back_to_first_session() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[("tab-1", &["session-1", "session-2"], Some("missing"))],
                Some("tab-1"),
            )],
            layout: AppLayout::default(),
        });

        assert_eq!(state.active_session_id(), Some("session-1"));
    }

    #[test]
    fn add_tab_selects_the_new_tab() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &[], None)],
            layout: AppLayout::default(),
        });

        state.add_tab(TerminalTab {
            id: "tab-1".to_string(),
            repo_id: "repo-1".to_string(),
            title: "Main".to_string(),
            sort_order: 0,
            active_session_id: None,
        });

        assert_eq!(state.active_tab_id(), Some("tab-1"));
    }

    #[test]
    fn add_session_selects_the_new_session_in_its_tab() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &[("tab-1", &["session-1"], Some("session-1"))], Some("tab-1"))],
            layout: AppLayout::default(),
        });

        state.add_session(TerminalSession {
            id: "session-2".to_string(),
            repo_id: "repo-1".to_string(),
            tab_id: "tab-1".to_string(),
            title: "session-2".to_string(),
            shell: None,
            sort_order: 1,
        });

        assert_eq!(state.active_tab_id(), Some("tab-1"));
        assert_eq!(state.active_session_id(), Some("session-2"));
    }

    #[test]
    fn closing_active_tab_falls_back_to_remaining_tab() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[
                    ("tab-1", &["session-1"], Some("session-1")),
                    ("tab-2", &["session-2"], Some("session-2")),
                ],
                Some("tab-2"),
            )],
            layout: AppLayout::default(),
        });

        state.close_tab("repo-1", "tab-2");

        assert_eq!(state.active_tab_id(), Some("tab-1"));
        assert_eq!(state.active_session_id(), Some("session-1"));
    }

    #[test]
    fn closing_active_session_falls_back_to_remaining_session() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[("tab-1", &["session-1", "session-2"], Some("session-2"))],
                Some("tab-1"),
            )],
            layout: AppLayout::default(),
        });

        state.close_session("repo-1", "tab-1", "session-2");

        assert_eq!(state.active_session_id(), Some("session-1"));
    }

    #[test]
    fn rename_tab_updates_the_saved_title() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &[("tab-1", &["session-1"], Some("session-1"))], Some("tab-1"))],
            layout: AppLayout::default(),
        });

        state.rename_tab("repo-1", "tab-1", "Logs");

        assert_eq!(state.repos[0].tabs[0].tab.title, "Logs");
    }

    #[test]
    fn rename_session_updates_the_saved_title() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &[("tab-1", &["session-1"], Some("session-1"))], Some("tab-1"))],
            layout: AppLayout::default(),
        });

        state.rename_session("repo-1", "tab-1", "session-1", "Logs");

        assert_eq!(state.repos[0].tabs[0].sessions[0].title, "Logs");
    }

    #[test]
    fn reorder_tabs_updates_tab_order_within_repo() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[
                    ("tab-1", &["session-1"], Some("session-1")),
                    ("tab-2", &["session-2"], Some("session-2")),
                    ("tab-3", &["session-3"], Some("session-3")),
                ],
                Some("tab-2"),
            )],
            layout: AppLayout::default(),
        });

        state.reorder_tabs(
            "repo-1",
            &[
                "tab-3".to_string(),
                "tab-1".to_string(),
                "tab-2".to_string(),
            ],
        );

        let tab_ids = state.repos[0]
            .tabs
            .iter()
            .map(|tab| tab.tab.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(tab_ids, vec!["tab-3", "tab-1", "tab-2"]);
        assert_eq!(state.repos[0].active_tab_id.as_deref(), Some("tab-2"));
    }

    #[test]
    fn reorder_sessions_updates_pane_order_within_tab() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo(
                "repo-1",
                &[("tab-1", &["session-1", "session-2", "session-3"], Some("session-2"))],
                Some("tab-1"),
            )],
            layout: AppLayout::default(),
        });

        state.reorder_sessions(
            "repo-1",
            "tab-1",
            &[
                "session-3".to_string(),
                "session-1".to_string(),
                "session-2".to_string(),
            ],
        );

        let session_ids = state.repos[0].tabs[0]
            .sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(session_ids, vec!["session-3", "session-1", "session-2"]);
        assert_eq!(
            state.repos[0].tabs[0].tab.active_session_id.as_deref(),
            Some("session-2")
        );
    }

    #[test]
    fn grouped_repos_keep_default_repo_first_within_group() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![
                RepoWorkspace {
                    repo: Repo {
                        id: "repo-1".to_string(),
                        name: "repo-1".to_string(),
                        path: "/tmp/repo-1".to_string(),
                        sort_order: 1,
                        group_id: Some("group-1".to_string()),
                        is_default: false,
                    },
                    tabs: Vec::new(),
                    active_tab_id: None,
                },
                RepoWorkspace {
                    repo: Repo {
                        id: "repo-2".to_string(),
                        name: "repo-2".to_string(),
                        path: "/tmp/repo-2".to_string(),
                        sort_order: 0,
                        group_id: Some("group-1".to_string()),
                        is_default: true,
                    },
                    tabs: Vec::new(),
                    active_tab_id: None,
                },
            ],
            layout: AppLayout::default(),
        });

        let repo_ids = state
            .repos
            .iter()
            .map(|repo| repo.repo.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(repo_ids, vec!["repo-2", "repo-1"]);
    }
}
