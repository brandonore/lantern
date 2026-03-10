use crate::config::UserConfig;
use crate::db::DbConn;
use crate::pty::PtyManager;
use std::sync::Mutex;

pub struct AppState {
    pub pty_manager: PtyManager,
    pub db: DbConn,
    pub config: Mutex<UserConfig>,
}
