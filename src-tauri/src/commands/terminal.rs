use crate::db;
use crate::error::LanternError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn terminal_create(
    repo_id: String,
    state: State<AppState>,
) -> Result<db::TerminalSession, LanternError> {
    // Get repo path for cwd
    let repos = db::list_repos(&state.db)?;
    let _repo = repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| LanternError::RepoNotFound(repo_id.clone()))?;

    let config = state.config.lock().unwrap();
    let shell = config.default_shell.clone();
    drop(config);

    // Count existing sessions for title
    let existing = db::list_sessions(&state.db, &repo_id)?;
    let title = format!("Terminal {}", existing.len() + 1);

    let session = db::create_session(&state.db, &repo_id, &title, Some(&shell))?;
    Ok(session)
}

#[tauri::command]
pub fn terminal_list(
    repo_id: String,
    state: State<AppState>,
) -> Result<Vec<db::TerminalSession>, LanternError> {
    db::list_sessions(&state.db, &repo_id)
}

#[tauri::command]
pub fn terminal_close(session_id: String, state: State<AppState>) -> Result<(), LanternError> {
    // Kill PTY if running
    state.pty_manager.close(&session_id)?;
    // Remove from DB
    db::close_session(&state.db, &session_id)
}

#[tauri::command]
pub fn terminal_rename(
    session_id: String,
    title: String,
    state: State<AppState>,
) -> Result<(), LanternError> {
    db::rename_session(&state.db, &session_id, &title)
}

#[tauri::command]
pub fn terminal_set_active(
    repo_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<(), LanternError> {
    db::set_active_tab(&state.db, &repo_id, &session_id)
}

#[tauri::command]
pub fn terminal_get_active(
    repo_id: String,
    state: State<AppState>,
) -> Result<Option<String>, LanternError> {
    db::get_active_tab(&state.db, &repo_id)
}
