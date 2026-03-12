use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub sort_order: i32,
    pub group_id: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalSession {
    pub id: String,
    pub repo_id: String,
    pub title: String,
    pub shell: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppLayout {
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: i32,
    pub window_height: i32,
    pub window_maximized: bool,
    pub sidebar_width: i32,
    pub sidebar_collapsed: bool,
    pub active_repo_id: Option<String>,
    pub collapsed_group_ids: Vec<String>,
}

impl Default for AppLayout {
    fn default() -> Self {
        Self {
            window_x: None,
            window_y: None,
            window_width: 1200,
            window_height: 800,
            window_maximized: false,
            sidebar_width: 250,
            sidebar_collapsed: false,
            active_repo_id: None,
            collapsed_group_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NativeSplitOrientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NativeSplitState {
    pub visible_session_ids: Vec<String>,
    pub orientation: NativeSplitOrientation,
    pub divider_positions: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoWorkspace {
    pub repo: Repo,
    pub sessions: Vec<TerminalSession>,
    pub active_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub repos: Vec<RepoWorkspace>,
    pub layout: AppLayout,
}
