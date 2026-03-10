use crate::db::{self, AppLayout};
use crate::error::LanternError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn state_save_layout(
    layout: AppLayout,
    state: State<AppState>,
) -> Result<(), LanternError> {
    // Keep AppState in sync so the window close handler has current values
    *state.sidebar_width.lock().unwrap() = layout.sidebar_width;
    *state.active_repo_id.lock().unwrap() = layout.active_repo_id.clone();
    db::save_layout(&state.db, &layout)
}

#[tauri::command]
pub fn state_load_layout(
    state: State<AppState>,
) -> Result<Option<AppLayout>, LanternError> {
    db::load_layout(&state.db)
}
