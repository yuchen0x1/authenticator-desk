use super::{DesktopCapabilities, DesktopConfig, DesktopState};
use crate::{AppError, AppResult, AppState};
use tauri::Manager;

#[tauri::command]
pub fn desktop_capabilities(
    state: tauri::State<'_, DesktopState>,
) -> AppResult<DesktopCapabilities> {
    state
        .capabilities
        .read()
        .map(|v| v.clone())
        .map_err(|_| AppError::Storage)
}
#[tauri::command]
pub fn desktop_config(state: tauri::State<'_, DesktopState>) -> AppResult<DesktopConfig> {
    state
        .config
        .read()
        .map(|v| v.clone())
        .map_err(|_| AppError::Storage)
}
#[tauri::command]
pub fn save_desktop_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    config: DesktopConfig,
) -> AppResult<()> {
    if app.state::<AppState>().vault.is_locked() {
        return Err(AppError::VaultLocked);
    }
    super::hotkeys::validate(&config)?;
    let old = state.config.read().map_err(|_| AppError::Storage)?.clone();
    if let Err(error) = super::hotkeys::register(&app, &config) {
        let _ = super::hotkeys::register(&app, &old);
        return Err(error);
    }
    if let Err(error) = super::startup::set_enabled(&app, config.startup) {
        let _ = super::hotkeys::register(&app, &old);
        return Err(error);
    }
    let path = app.state::<AppState>().data_dir()?.join("desktop.json");
    let result = (|| {
        std::fs::create_dir_all(path.parent().ok_or(AppError::Storage)?)
            .map_err(|_| AppError::Storage)?;
        std::fs::write(
            &path,
            serde_json::to_vec(&config).map_err(|_| AppError::Storage)?,
        )
        .map_err(|_| AppError::Storage)
    })();
    if let Err(error) = result {
        let _ = super::hotkeys::register(&app, &old);
        let _ = super::startup::set_enabled(&app, old.startup);
        return Err(error);
    }
    *state.config.write().map_err(|_| AppError::Storage)? = config;
    Ok(())
}
#[tauri::command]
pub fn show_quick_panel(app: tauri::AppHandle) -> AppResult<()> {
    super::show_quick_panel(&app)
}

#[tauri::command]
pub fn quick_panel_accounts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<crate::accounts::AccountView>> {
    state.accounts.list_accounts(crate::accounts::AccountQuery {
        favorite_only: true,
        ..Default::default()
    })
}

#[tauri::command]
pub fn pick_qr_image(
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<crate::import::ImportPreview>> {
    if state.vault.is_locked() {
        return Err(AppError::VaultLocked);
    }
    let settings = state.accounts.settings()?;
    let Some(values) = crate::qr::pick_qr_image(&settings.locale)? else {
        return Ok(None);
    };
    state.imports.preview_import(values.join("\n")).map(Some)
}

#[tauri::command]
pub fn test_notification(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> AppResult<()> {
    if !state
        .config
        .read()
        .map_err(|_| AppError::Storage)?
        .notifications
    {
        return Err(AppError::InvalidParameters);
    }
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title("AUTHENTICATOR DESK")
        .body("Desktop notifications are enabled.")
        .show()
        .map_err(|_| AppError::Storage)
}
