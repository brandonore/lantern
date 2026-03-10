use crate::db;
use crate::error::LanternError;
use crate::git;
use crate::pty::TerminalOutput;
use crate::state::AppState;
use tauri::{ipc::Channel, State};

#[tauri::command]
pub fn terminal_write(
    session_id: String,
    data: Vec<u8>,
    state: State<AppState>,
) -> Result<(), LanternError> {
    state.pty_manager.write(&session_id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    session_id: String,
    cols: u16,
    rows: u16,
    state: State<AppState>,
) -> Result<(), LanternError> {
    state.pty_manager.resize(&session_id, cols, rows)
}

#[tauri::command]
pub fn terminal_subscribe(
    session_id: String,
    channel: Channel<TerminalOutput>,
    state: State<AppState>,
) -> Result<(), LanternError> {
    // Get the session info to find shell and cwd
    let repos = db::list_repos(&state.db)?;
    let sessions_all: Vec<db::TerminalSession> = repos
        .iter()
        .flat_map(|r| db::list_sessions(&state.db, &r.id).unwrap_or_default())
        .collect();

    let session = sessions_all
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| LanternError::SessionNotFound(session_id.clone()))?;

    let repo = repos
        .iter()
        .find(|r| r.id == session.repo_id)
        .ok_or_else(|| LanternError::RepoNotFound(session.repo_id.clone()))?;

    let config = state.config.lock().unwrap();
    let shell = session
        .shell
        .clone()
        .unwrap_or_else(|| config.default_shell.clone());
    drop(config);

    // Only spawn if not already running
    if !state.pty_manager.session_exists(&session_id) {
        state.pty_manager.spawn(
            &session_id,
            &shell,
            &repo.path,
            80,
            24,
            Box::new(move |output| {
                let _ = channel.send(output);
            }),
        )?;
    }

    Ok(())
}

#[tauri::command]
pub fn terminal_get_foreground_process(
    session_id: String,
    state: State<AppState>,
) -> Result<Option<git::ProcessInfo>, LanternError> {
    match state.pty_manager.get_pid(&session_id) {
        Some(pid) => Ok(git::get_foreground_process(pid)),
        None => Ok(None),
    }
}
