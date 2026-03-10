use crate::db;
use crate::error::LanternError;
use crate::git;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn repo_add(path: String, state: State<AppState>) -> Result<db::Repo, LanternError> {
    let repo = db::add_repo(&state.db, &path)?;

    // Auto-detect worktree siblings: if this path is part of a worktree family
    // and a sibling is already tracked, assign the same group_id
    if let Some(wt_info) = git::detect_worktree_info(&path) {
        let sibling_paths: Vec<String> = wt_info
            .entries
            .iter()
            .filter(|e| e.path != path)
            .map(|e| e.path.clone())
            .collect();

        if let Ok(Some(existing_group_id)) = db::find_group_id_by_paths(&state.db, &sibling_paths) {
            let is_main = wt_info.entries.iter().any(|e| e.path == path && e.is_main);
            db::set_repo_group(&state.db, &repo.id, &existing_group_id, is_main)?;
            // Re-fetch to return updated data
            let repos = db::list_repos(&state.db)?;
            if let Some(updated) = repos.into_iter().find(|r| r.id == repo.id) {
                return Ok(updated);
            }
        }
    }

    Ok(repo)
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

#[tauri::command]
pub fn repo_detect_worktrees(path: String) -> Result<Option<git::WorktreeInfo>, LanternError> {
    Ok(git::detect_worktree_info(&path))
}

#[tauri::command]
pub fn repo_add_with_worktrees(
    path: String,
    state: State<AppState>,
) -> Result<Vec<db::Repo>, LanternError> {
    let wt_info = git::detect_worktree_info(&path);

    let entries = match wt_info {
        Some(info) => info.entries,
        None => {
            // No worktrees, just add as standalone
            let repo = db::add_repo(&state.db, &path)?;
            return Ok(vec![repo]);
        }
    };

    // Check if any siblings already exist in DB, reuse their group_id
    let all_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let group_id = db::find_group_id_by_paths(&state.db, &all_paths)?
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut added = Vec::new();
    for entry in &entries {
        // Skip paths already in DB
        match db::add_repo_grouped(&state.db, &entry.path, Some(&group_id), entry.is_main) {
            Ok(repo) => added.push(repo),
            Err(LanternError::RepoAlreadyExists(_)) => continue,
            Err(LanternError::PathNotFound(_)) => continue,
            Err(e) => return Err(e),
        }
    }

    Ok(added)
}
