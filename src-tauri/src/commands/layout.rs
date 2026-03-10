use crate::db::{self, AppLayout};
use crate::error::LanternError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn state_save_layout(
    layout: AppLayout,
    state: State<AppState>,
) -> Result<(), LanternError> {
    db::save_layout(&state.db, &layout)
}

#[tauri::command]
pub fn state_load_layout(
    state: State<AppState>,
) -> Result<Option<AppLayout>, LanternError> {
    db::load_layout(&state.db)
}
