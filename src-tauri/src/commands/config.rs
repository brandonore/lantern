use crate::config::UserConfig;
use crate::error::LanternError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn config_get(state: State<AppState>) -> Result<UserConfig, LanternError> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

#[tauri::command]
pub fn config_update(
    patch: serde_json::Value,
    state: State<AppState>,
) -> Result<UserConfig, LanternError> {
    let mut config = state.config.lock().unwrap();
    config.merge_patch(patch);
    config.save()?;
    Ok(config.clone())
}
