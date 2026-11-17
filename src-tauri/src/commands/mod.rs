#[cfg(feature = "desktop")]
use crate::{AppResult, AppSnapshot, AppState};
#[cfg(feature = "desktop")]
use tauri::Emitter;

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn app_snapshot(state: tauri::State<'_, AppState>) -> AppResult<AppSnapshot> {
    state.snapshot()
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn vault_status(state: tauri::State<'_, AppState>) -> AppResult<crate::vault::VaultStatus> {
    Ok(state.vault.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn create_vault(
    state: tauri::State<'_, AppState>,
    password: String,
    path: String,
) -> AppResult<()> {
    state
        .vault
        .create_vault(crate::vault::SecretString::from(password), path)?;
    state.set_vault_state(true, true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn unlock_vault(
    state: tauri::State<'_, AppState>,
    password: String,
) -> AppResult<crate::vault::VaultStatus> {
    state
        .vault
        .unlock_vault(crate::vault::SecretString::from(password))?;
    state.set_vault_state(false, true)?;
    Ok(state.vault.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn lock_vault(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.vault.lock_vault()?;
    state.set_vault_state(true, true)?;
    app.emit("vault://locked", ())
        .map_err(|_| crate::AppError::Storage)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn integrity_check(
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::vault::IntegrityReport> {
    state.vault.integrity_check()
}
