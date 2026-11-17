#[cfg(feature = "desktop")]
use crate::{AppResult, AppSnapshot, AppState};

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn app_snapshot(state: tauri::State<'_, AppState>) -> AppResult<AppSnapshot> {
    state.snapshot()
}
