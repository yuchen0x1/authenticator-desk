use crate::{AppError, AppResult};
use tauri_plugin_autostart::ManagerExt;
pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> AppResult<()> {
    // Isolated acceptance runs must never alter the user's real login registry.
    if std::env::var_os("AUTHDESK_DATA_DIR").is_some() {
        return if enabled {
            Err(AppError::InvalidParameters)
        } else {
            Ok(())
        };
    }
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|_| AppError::Storage)
}
pub fn is_enabled(app: &tauri::AppHandle) -> AppResult<bool> {
    app.autolaunch().is_enabled().map_err(|_| AppError::Storage)
}
