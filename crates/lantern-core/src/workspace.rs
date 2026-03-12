use crate::db::{self, DbConn};
use crate::error::LanternError;
use crate::models::{AppLayout, RepoWorkspace, TerminalSession, WorkspaceSnapshot};
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
                let sessions = db::list_sessions(conn, &repo.id)?;
                let persisted_active_session_id = db::get_active_tab(conn, &repo.id)?;
                let active_session_id =
                    normalize_active_session_id(&sessions, persisted_active_session_id);

                if let Some(active_session_id) = active_session_id.as_deref() {
                    db::set_active_tab(conn, &repo.id, active_session_id)?;
                }

                Ok(RepoWorkspace {
                    repo,
                    sessions,
                    active_session_id,
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
            repo.active_session_id =
                normalize_active_session_id(&repo.sessions, repo.active_session_id.clone());
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

    pub fn active_session_id(&self) -> Option<&str> {
        self.active_repo()
            .and_then(|repo| repo.active_session_id.as_deref())
    }

    pub fn set_active_repo(&mut self, repo_id: &str) {
        if self.repos.iter().any(|repo| repo.repo.id == repo_id) {
            self.active_repo_id = Some(repo_id.to_string());
            self.layout.active_repo_id = self.active_repo_id.clone();
        }
    }

    pub fn set_active_session(&mut self, repo_id: &str, session_id: &str) {
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            if repo.sessions.iter().any(|session| session.id == session_id) {
                repo.active_session_id = Some(session_id.to_string());
                self.set_active_repo(repo_id);
            }
        }
    }

    pub fn add_repo(&mut self, repo: crate::models::Repo) {
        if self
            .repos
            .iter()
            .any(|existing| existing.repo.id == repo.id)
        {
            return;
        }

        self.repos.push(RepoWorkspace {
            repo: repo.clone(),
            sessions: Vec::new(),
            active_session_id: None,
        });
        sort_repos(&mut self.repos);
        self.set_active_repo(repo.id.as_str());
    }

    pub fn remove_repo(&mut self, repo_id: &str) {
        self.repos.retain(|repo| repo.repo.id != repo_id);
        sort_repos(&mut self.repos);
        self.active_repo_id = normalize_active_repo_id(&self.repos, self.active_repo_id.clone());
        self.layout.active_repo_id = self.active_repo_id.clone();
    }

    pub fn add_session(&mut self, session: TerminalSession) {
        if let Some(repo) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == session.repo_id)
        {
            repo.active_session_id = Some(session.id.clone());
            repo.sessions.push(session.clone());
            repo.sessions
                .sort_by(|left, right| left.sort_order.cmp(&right.sort_order));
            self.set_active_repo(session.repo_id.as_str());
        }
    }

    pub fn close_session(&mut self, repo_id: &str, session_id: &str) {
        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            repo.sessions.retain(|session| session.id != session_id);
            repo.active_session_id =
                normalize_active_session_id(&repo.sessions, repo.active_session_id.clone());
        }
    }

    pub fn rename_session(&mut self, repo_id: &str, session_id: &str, title: &str) {
        if let Some(session) = self
            .repos
            .iter_mut()
            .find(|repo| repo.repo.id == repo_id)
            .and_then(|repo| {
                repo.sessions
                    .iter_mut()
                    .find(|session| session.id == session_id)
            })
        {
            session.title = title.to_string();
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

    pub fn reorder_sessions(&mut self, repo_id: &str, session_ids: &[String]) {
        let sort_order_by_session_id = session_ids
            .iter()
            .enumerate()
            .map(|(sort_order, session_id)| (session_id.as_str(), sort_order as i32))
            .collect::<HashMap<_, _>>();

        if let Some(repo) = self.repos.iter_mut().find(|repo| repo.repo.id == repo_id) {
            for session in &mut repo.sessions {
                if let Some(sort_order) = sort_order_by_session_id.get(session.id.as_str()) {
                    session.sort_order = *sort_order;
                }
            }
            repo.sessions
                .sort_by(|left, right| left.sort_order.cmp(&right.sort_order));
        }
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

fn normalize_active_session_id(
    sessions: &[crate::models::TerminalSession],
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
        .and_then(|group_id| group_min_sort_order.get(group_id).copied())
        .unwrap_or(repo.repo.sort_order);

    (
        group_sort_order,
        if repo.repo.is_default { 0 } else { 1 },
        repo.repo.sort_order,
        repo.repo.name.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Repo, TerminalSession};

    fn repo(id: &str, sessions: &[&str], active_session_id: Option<&str>) -> RepoWorkspace {
        RepoWorkspace {
            repo: Repo {
                id: id.to_string(),
                name: id.to_string(),
                path: format!("/tmp/{id}"),
                sort_order: 0,
                group_id: None,
                is_default: false,
            },
            sessions: sessions
                .iter()
                .enumerate()
                .map(|(index, session_id)| TerminalSession {
                    id: (*session_id).to_string(),
                    repo_id: id.to_string(),
                    title: format!("Terminal {}", index + 1),
                    shell: Some("/bin/bash".to_string()),
                    sort_order: index as i32,
                })
                .collect(),
            active_session_id: active_session_id.map(|id| id.to_string()),
        }
    }

    #[test]
    fn invalid_active_repo_falls_back_to_first_repo() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1"], Some("tab-1"))],
            layout: AppLayout {
                active_repo_id: Some("missing".to_string()),
                ..AppLayout::default()
            },
        });

        assert_eq!(state.active_repo_id.as_deref(), Some("repo-1"));
    }

    #[test]
    fn invalid_active_session_falls_back_to_first_session() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1", "tab-2"], Some("missing"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        assert_eq!(state.active_session_id(), Some("tab-1"));
    }

    #[test]
    fn empty_session_list_stays_unselected() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &[], None)],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        assert_eq!(state.active_session_id(), None);
    }

    #[test]
    fn add_session_selects_the_new_session() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1"], Some("tab-1"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.add_session(TerminalSession {
            id: "tab-2".to_string(),
            repo_id: "repo-1".to_string(),
            title: "Terminal 2".to_string(),
            shell: Some("/bin/bash".to_string()),
            sort_order: 1,
        });

        assert_eq!(state.active_session_id(), Some("tab-2"));
        assert_eq!(state.active_repo().unwrap().sessions.len(), 2);
    }

    #[test]
    fn closing_active_session_falls_back_to_remaining_session() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1", "tab-2"], Some("tab-2"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.close_session("repo-1", "tab-2");

        assert_eq!(state.active_session_id(), Some("tab-1"));
    }

    #[test]
    fn closing_last_session_clears_active_session() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1"], Some("tab-1"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.close_session("repo-1", "tab-1");

        assert_eq!(state.active_session_id(), None);
        assert!(state.active_repo().unwrap().sessions.is_empty());
    }

    #[test]
    fn add_repo_selects_the_new_repo() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1"], Some("tab-1"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.add_repo(Repo {
            id: "repo-2".to_string(),
            name: "repo-2".to_string(),
            path: "/tmp/repo-2".to_string(),
            sort_order: 1,
            group_id: None,
            is_default: false,
        });

        assert_eq!(state.active_repo_id.as_deref(), Some("repo-2"));
        assert_eq!(state.repos.len(), 2);
    }

    #[test]
    fn removing_active_repo_falls_back_to_first_remaining_repo() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![
                repo("repo-1", &["tab-1"], Some("tab-1")),
                repo("repo-2", &["tab-2"], Some("tab-2")),
            ],
            layout: AppLayout {
                active_repo_id: Some("repo-2".to_string()),
                ..AppLayout::default()
            },
        });

        state.remove_repo("repo-2");

        assert_eq!(state.active_repo_id.as_deref(), Some("repo-1"));
        assert_eq!(state.repos.len(), 1);
    }

    #[test]
    fn rename_session_updates_the_saved_title() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1"], Some("tab-1"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.rename_session("repo-1", "tab-1", "Logs");

        assert_eq!(state.repos[0].sessions[0].title, "Logs");
    }

    #[test]
    fn grouped_repos_sort_together_with_default_first() {
        let state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![
                RepoWorkspace {
                    repo: Repo {
                        id: "standalone".to_string(),
                        name: "standalone".to_string(),
                        path: "/tmp/standalone".to_string(),
                        sort_order: 0,
                        group_id: None,
                        is_default: false,
                    },
                    sessions: Vec::new(),
                    active_session_id: None,
                },
                RepoWorkspace {
                    repo: Repo {
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
                RepoWorkspace {
                    repo: Repo {
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
            ],
            layout: AppLayout::default(),
        });

        let repo_ids = state
            .repos
            .iter()
            .map(|repo| repo.repo.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(repo_ids, vec!["standalone", "main", "feature"]);
    }

    #[test]
    fn reorder_repos_updates_sort_order_and_respects_grouping() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![
                RepoWorkspace {
                    repo: Repo {
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
                    repo: Repo {
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
                    repo: Repo {
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
            ],
            layout: AppLayout::default(),
        });

        state.reorder_repos(&[
            "main".to_string(),
            "feature".to_string(),
            "repo-1".to_string(),
        ]);

        let repo_ids = state
            .repos
            .iter()
            .map(|repo| repo.repo.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(repo_ids, vec!["main", "feature", "repo-1"]);
        assert_eq!(state.repos[0].repo.sort_order, 0);
        assert_eq!(state.repos[1].repo.sort_order, 1);
        assert_eq!(state.repos[2].repo.sort_order, 2);
    }

    #[test]
    fn reorder_sessions_updates_tab_order_within_repo() {
        let mut state = WorkspaceState::from_snapshot(WorkspaceSnapshot {
            repos: vec![repo("repo-1", &["tab-1", "tab-2", "tab-3"], Some("tab-2"))],
            layout: AppLayout {
                active_repo_id: Some("repo-1".to_string()),
                ..AppLayout::default()
            },
        });

        state.reorder_sessions(
            "repo-1",
            &[
                "tab-3".to_string(),
                "tab-1".to_string(),
                "tab-2".to_string(),
            ],
        );

        let session_ids = state.repos[0]
            .sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(session_ids, vec!["tab-3", "tab-1", "tab-2"]);
        assert_eq!(state.repos[0].active_session_id.as_deref(), Some("tab-2"));
    }
}
