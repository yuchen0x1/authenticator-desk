#[cfg(feature = "desktop")]
use crate::{AppResult, AppSnapshot, AppState};
use std::path::{Path, PathBuf};

#[cfg(feature = "desktop")]
use tauri::{Emitter, Manager};

#[allow(dead_code)]
pub(crate) fn resolve_vault_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("vault.sqlite3")
}

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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    password: String,
) -> AppResult<()> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| crate::AppError::Storage)?;
    let path = resolve_vault_path(&app_data_dir);
    state
        .vault
        .create_vault(crate::vault::SecretString::from(password), &path)?;
    state.set_vault_state(true, true)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::resolve_vault_path;

    #[test]
    fn backend_vault_path_is_fixed_under_app_data_dir() {
        let app_data = Path::new(r"C:\Users\tester\AppData\Local\AUTHENTICATOR DESK");
        assert_eq!(
            resolve_vault_path(app_data),
            PathBuf::from(app_data).join("vault.sqlite3")
        );
    }
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
