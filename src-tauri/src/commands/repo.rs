use crate::db;
use crate::error::LanternError;
use crate::git;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn repo_add(path: String, state: State<AppState>) -> Result<db::Repo, LanternError> {
    db::add_repo(&state.db, &path)
}

#[tauri::command]
pub fn repo_remove(id: String, state: State<AppState>) -> Result<(), LanternError> {
    db::remove_repo(&state.db, &id)
}

#[tauri::command]
pub fn repo_list(state: State<AppState>) -> Result<Vec<db::Repo>, LanternError> {
    db::list_repos(&state.db)
}

#[tauri::command]
pub fn repo_reorder(ids: Vec<String>, state: State<AppState>) -> Result<(), LanternError> {
    db::reorder_repos(&state.db, &ids)
}

#[tauri::command]
pub fn repo_get_all_git_info(
    state: State<AppState>,
) -> Result<Vec<(String, git::GitInfo)>, LanternError> {
    let repos = db::list_repos(&state.db)?;
    let results: Vec<(String, git::GitInfo)> = repos
        .iter()
        .map(|r| (r.id.clone(), git::git_info_for_path(&r.path)))
        .collect();
    Ok(results)
}
