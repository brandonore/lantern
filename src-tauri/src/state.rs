use crate::config::UserConfig;
use crate::db::DbConn;
use crate::pty::PtyManager;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub pty_manager: PtyManager,
    pub db: DbConn,
    pub config: Mutex<UserConfig>,
    pub sidebar_width: Mutex<i32>,
    pub active_repo_id: Mutex<Option<String>>,
    pub git_poll_interval: Arc<AtomicU64>,
}
