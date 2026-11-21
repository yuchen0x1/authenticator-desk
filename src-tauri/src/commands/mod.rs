#[cfg(feature = "desktop")]
use crate::{AppResult, AppSnapshot, AppState};
use std::path::{Path, PathBuf};

#[cfg(feature = "desktop")]
use tauri::Emitter;

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
pub fn create_vault(state: tauri::State<'_, AppState>, password: String) -> AppResult<()> {
    let app_data_dir = state.data_dir()?;
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
    fn native_import_read_is_bounded_before_and_during_read() {
        let path =
            std::env::temp_dir().join(format!("authdesk-bounded-import-{}", uuid::Uuid::new_v4()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(crate::import::MAX_IMPORT_BYTES as u64 + 1)
            .unwrap();
        drop(file);
        assert!(matches!(
            super::read_import_file(&path),
            Err(crate::AppError::InvalidInput)
        ));
        std::fs::write(&path, b"synthetic fixture").unwrap();
        assert_eq!(
            super::read_import_file(&path).unwrap().as_slice(),
            b"synthetic fixture"
        );
        std::fs::remove_file(path).unwrap();
    }

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
    let settings = match state.accounts.settings() {
        Ok(settings) => settings,
        Err(error) => {
            let _ = state.lock();
            return Err(error);
        }
    };
    if let Err(error) = state
        .vault
        .set_idle_timeout(std::time::Duration::from_secs(settings.idle_timeout_secs))
    {
        let _ = state.lock();
        return Err(error);
    }
    state.set_vault_state(false, true)?;
    Ok(state.vault.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn lock_vault(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.lock()?;
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

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_accounts(
    state: tauri::State<'_, AppState>,
    query: crate::accounts::AccountQuery,
) -> AppResult<Vec<crate::accounts::AccountView>> {
    state.accounts.list_accounts(query)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn account_details(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<crate::accounts::AccountDetails> {
    state.accounts.account_details(&id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn create_account(
    state: tauri::State<'_, AppState>,
    input: crate::accounts::AccountInput,
) -> AppResult<crate::accounts::AccountView> {
    state.accounts.create_account(input)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn update_account(
    state: tauri::State<'_, AppState>,
    id: String,
    input: crate::accounts::AccountInput,
) -> AppResult<crate::accounts::AccountView> {
    state.accounts.update_account(&id, input)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn delete_accounts(state: tauri::State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    state.accounts.delete_accounts(&ids)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn set_favorite(
    state: tauri::State<'_, AppState>,
    id: String,
    favorite: bool,
) -> AppResult<()> {
    state.accounts.set_favorite(&id, favorite)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn move_accounts(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    category_id: Option<String>,
) -> AppResult<()> {
    state.accounts.move_accounts(&ids, category_id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn reorder_accounts(state: tauri::State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    state.accounts.reorder_accounts(&ids)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn generate_codes(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<Vec<crate::accounts::CodeView>> {
    state.visible_codes(&ids)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn advance_hotp(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<crate::accounts::CodeView> {
    state.advance_visible_hotp(&id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_categories(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<crate::accounts::CategoryView>> {
    state.accounts.list_categories()
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn save_category(
    state: tauri::State<'_, AppState>,
    id: Option<String>,
    name: String,
    color: String,
) -> AppResult<crate::accounts::CategoryView> {
    state.accounts.save_category(id, &name, &color)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn delete_category(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    state.accounts.delete_category(&id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::accounts::AppSettings> {
    let settings = state.accounts.settings()?;
    // Unlock and hydration read the saved preferences here, so this is where a
    // launched app first learns whether the host window should be light or dark.
    crate::desktop::apply_theme(&app, &settings.theme);
    // Re-seed the locked-screen hint from the authoritative saved value.
    if let Ok(data_dir) = state.data_dir() {
        let _ = crate::desktop::save_theme_hint(&data_dir, &settings.theme);
    }
    Ok(settings)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: crate::accounts::AppSettings,
) -> AppResult<()> {
    state.save_preferences(settings.clone())?;
    // Keep the host window theme (title bar, native popups, default scrollbars)
    // in step with the saved preference.
    crate::desktop::apply_theme(&app, &settings.theme);
    // Mirror the choice into the non-secret hint so the next launch's locked
    // screen already uses it.
    if let Ok(data_dir) = state.data_dir() {
        let _ = crate::desktop::save_theme_hint(&data_dir, &settings.theme);
    }
    app.emit("settings://changed", ())
        .map_err(|_| crate::AppError::Storage)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn audit_events(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> AppResult<Vec<crate::accounts::AuditEvent>> {
    state.accounts.audit_events(limit.unwrap_or(100))
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn record_copy(state: tauri::State<'_, AppState>, account_id: String) -> AppResult<()> {
    state.accounts.record_copy(&account_id)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn dashboard(state: tauri::State<'_, AppState>) -> AppResult<crate::accounts::Dashboard> {
    state.accounts.dashboard()
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn platform_templates() -> Vec<crate::accounts::PlatformTemplate> {
    crate::accounts::platform_templates()
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn preview_import(
    state: tauri::State<'_, AppState>,
    text: String,
) -> AppResult<crate::import::ImportPreview> {
    state.imports.preview_import(text)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn preview_import_file(
    state: tauri::State<'_, AppState>,
    payload_b64: String,
) -> AppResult<crate::import::ImportPreview> {
    let payload = crate::import::base64::decode_base64(&payload_b64)?;
    state.imports.preview_file(payload.to_vec())
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn commit_import(
    state: tauri::State<'_, AppState>,
    preview_id: String,
    selection: Vec<usize>,
) -> AppResult<usize> {
    state.imports.commit_import(&preview_id, &selection)
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn cancel_import_preview(
    state: tauri::State<'_, AppState>,
    preview_id: String,
) -> AppResult<()> {
    state.imports.cancel_preview(&preview_id)
}

#[cfg(any(feature = "desktop", test))]
fn read_import_file(path: &Path) -> crate::AppResult<zeroize::Zeroizing<Vec<u8>>> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|_| crate::AppError::Storage)?;
    let metadata = file.metadata().map_err(|_| crate::AppError::Storage)?;
    let limit = crate::import::MAX_IMPORT_BYTES;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(crate::AppError::InvalidInput);
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| crate::AppError::Storage)?;
    if bytes.len() > limit {
        return Err(crate::AppError::InvalidInput);
    }
    Ok(bytes)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn pick_import_file(
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<crate::import::ImportPreview>> {
    if state.vault.is_locked() {
        return Err(crate::AppError::VaultLocked);
    }
    let Some(path) = rfd::FileDialog::new()
        .add_filter(
            "Authenticator export",
            &["json", "txt", "xml", "aes", "bin", "csv"],
        )
        .pick_file()
    else {
        return Ok(None);
    };
    let mut bytes = read_import_file(&path)?;
    state
        .imports
        .preview_file(std::mem::take(&mut *bytes))
        .map(Some)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn activity_touch(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.vault.touch()
}
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn change_password(
    state: tauri::State<'_, AppState>,
    current: String,
    next: String,
) -> AppResult<()> {
    state.imports.clear()?;
    state.backups.clear()?;
    state.vault.change_password(
        crate::vault::SecretString::from(current),
        crate::vault::SecretString::from(next),
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_create(
    state: tauri::State<'_, AppState>,
    password: String,
) -> AppResult<Option<crate::backup::Summary>> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("AUTHENTICATOR DESK backup", &["authdesk"])
        .set_file_name("authenticator-backup.authdesk")
        .save_file()
    else {
        return Ok(None);
    };
    state
        .backups
        .create_backup(crate::vault::SecretString::from(password), &path)
        .map(Some)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_preview_restore(
    state: tauri::State<'_, AppState>,
    password: String,
    mode: crate::backup::RestoreMode,
) -> AppResult<Option<crate::backup::RestorePreview>> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("AUTHENTICATOR DESK backup", &["authdesk"])
        .pick_file()
    else {
        return Ok(None);
    };
    state
        .backups
        .preview_restore(crate::vault::SecretString::from(password), &path, mode)
        .map(Some)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_apply_restore(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    preview_id: String,
    selection: crate::backup::RestoreSelection,
) -> AppResult<crate::backup::RestoreResult> {
    let preview_id =
        uuid::Uuid::parse_str(&preview_id).map_err(|_| crate::AppError::InvalidInput)?;
    let result = state.backups.apply_restore(preview_id, selection)?;
    state.imports.clear()?;
    if result.mode == crate::backup::RestoreMode::Replace {
        state.lock()?;
        app.emit("vault://locked", ())
            .map_err(|_| crate::AppError::Storage)?;
    }
    Ok(result)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_cancel_preview(
    state: tauri::State<'_, AppState>,
    preview_id: String,
) -> AppResult<()> {
    let id = uuid::Uuid::parse_str(&preview_id).map_err(|_| crate::AppError::InvalidInput)?;
    state.backups.cancel_preview(id)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_auto_status(
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::backup::AutoBackupStatus> {
    state.backups.auto_backup_status()
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_configure_auto(
    state: tauri::State<'_, AppState>,
    enabled: bool,
    password: String,
    interval_hours: u32,
    retention: usize,
) -> AppResult<Option<crate::backup::AutoBackupStatus>> {
    let directory = if enabled {
        let Some(path) = rfd::FileDialog::new().pick_folder() else {
            return Ok(None);
        };
        path
    } else {
        state.data_dir()?
    };
    state
        .backups
        .configure_auto_backup(
            enabled,
            directory,
            crate::vault::SecretString::from(password),
            interval_hours,
            retention,
        )
        .map(Some)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn backup_list(
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<Vec<crate::backup::BackupFileView>>> {
    let Some(directory) = rfd::FileDialog::new().pick_folder() else {
        return Ok(None);
    };
    state.backups.list_backups(&directory).map(Some)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn security_audit(
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::security::SecurityAudit> {
    let backup_directory = state.backups.configured_directory()?;
    crate::security::run_security_audit(
        &state.vault,
        &state.data_dir()?,
        backup_directory.as_deref(),
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn copy_code(
    state: tauri::State<'_, AppState>,
    account_id: String,
    expected_code: Option<String>,
) -> AppResult<crate::clipboard::ClipboardToken> {
    let _action = state
        .desktop_action
        .lock()
        .map_err(|_| crate::AppError::Storage)?;
    state.vault.touch()?;
    let settings = state.accounts.settings()?;
    let codes = state.accounts.generate_codes(&[account_id.clone()])?;
    let code = codes.first().ok_or(crate::AppError::NotFound)?;
    if expected_code
        .as_ref()
        .is_some_and(|expected| expected != &code.code)
    {
        return Err(crate::AppError::Conflict);
    }
    let token = state
        .clipboard
        .copy(&code.code, settings.clipboard_ttl_secs as u32)?;
    if let Err(error) = state.accounts.record_copy(&account_id) {
        let _ = state.clipboard.clear_token(&token.token);
        return Err(error);
    }
    Ok(token)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> AppResult<()> {
    crate::desktop::show_main(&app)
}
