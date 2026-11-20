pub mod accounts;
pub mod backup;
pub mod clipboard;
pub mod commands;
pub mod desktop;
mod error;
pub mod import;
pub mod otp;
pub mod qr;
pub mod security;
pub mod services;
pub mod storage;
#[cfg(all(feature = "desktop", windows))]
mod update;
pub mod vault;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

#[cfg(feature = "desktop")]
use tauri::Manager;

pub use error::{AppError, AppResult};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppSnapshot {
    pub locked: bool,
    pub vault_exists: bool,
    pub locale: String,
    pub theme: String,
}

pub struct AppState {
    snapshot: RwLock<AppSnapshot>,
    pub vault: Arc<vault::VaultService>,
    pub accounts: Arc<accounts::AccountService>,
    pub imports: Arc<import::ImportService>,
    pub backups: Arc<backup::BackupService>,
    pub clipboard: clipboard::ClipboardService,
    pub(crate) desktop_action: Mutex<()>,
    app_data_dir: RwLock<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        let vault = Arc::new(vault::VaultService::default());
        let accounts = Arc::new(accounts::AccountService::new(vault.clone()));
        let imports = Arc::new(import::ImportService::new(accounts.clone()));
        let backups = Arc::new(backup::BackupService::new(vault.clone()));
        Self {
            snapshot: RwLock::new(AppSnapshot {
                locked: true,
                vault_exists: false,
                locale: "zh-CN".to_owned(),
                theme: "dark".to_owned(),
            }),
            vault,
            accounts,
            imports,
            backups,
            clipboard: clipboard::ClipboardService::default(),
            desktop_action: Mutex::new(()),
            app_data_dir: RwLock::new(None),
        }
    }
}

impl AppState {
    pub fn bind_app_data_dir(&self, app_data_dir: &Path) -> AppResult<()> {
        self.vault
            .configure_path(commands::resolve_vault_path(app_data_dir))?;
        *self.app_data_dir.write().map_err(|_| AppError::Storage)? =
            Some(app_data_dir.to_path_buf());
        Ok(())
    }

    pub fn data_dir(&self) -> AppResult<PathBuf> {
        self.app_data_dir
            .read()
            .map_err(|_| AppError::Storage)?
            .clone()
            .ok_or(AppError::NotFound)
    }

    pub fn visible_codes(&self, ids: &[String]) -> AppResult<Vec<accounts::CodeView>> {
        let _action = self.desktop_action.lock().map_err(|_| AppError::Storage)?;
        if self.accounts.settings()?.hide_codes {
            return Ok(Vec::new());
        }
        self.accounts.generate_available_codes(ids)
    }

    pub fn advance_visible_hotp(&self, id: &str) -> AppResult<accounts::CodeView> {
        let _action = self.desktop_action.lock().map_err(|_| AppError::Storage)?;
        if self.accounts.settings()?.hide_codes {
            return Err(AppError::InvalidParameters);
        }
        self.accounts.advance_hotp(id)
    }

    pub fn save_preferences(&self, settings: accounts::AppSettings) -> AppResult<()> {
        let _action = self.desktop_action.lock().map_err(|_| AppError::Storage)?;
        self.accounts.save_settings(settings)
    }

    pub fn lock(&self) -> AppResult<()> {
        let _action = self.desktop_action.lock().map_err(|_| AppError::Storage)?;
        self.vault.lock_vault()?;
        self.imports.clear()?;
        self.backups.clear()?;
        let _ = self.clipboard.clear_owned();
        Ok(())
    }

    pub fn maintenance_tick(&self) -> AppResult<bool> {
        let _action = self.desktop_action.lock().map_err(|_| AppError::Storage)?;
        let expired = self.vault.check_idle()?;
        if self.vault.is_locked() {
            self.imports.clear()?;
            self.backups.clear()?;
            let _ = self.clipboard.clear_owned();
        } else {
            self.imports.cleanup_expired()?;
            self.backups.cleanup_expired()?;
        }
        Ok(expired)
    }

    pub fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.snapshot
            .read()
            .map(|snapshot| {
                let status = self.vault.status();
                AppSnapshot {
                    locked: status.locked,
                    vault_exists: status.exists,
                    locale: snapshot.locale.clone(),
                    theme: snapshot.theme.clone(),
                }
            })
            .map_err(|_| AppError::Storage)
    }

    pub fn set_vault_state(&self, locked: bool, vault_exists: bool) -> AppResult<()> {
        let mut snapshot = self.snapshot.write().map_err(|_| AppError::Storage)?;
        snapshot.locked = locked;
        snapshot.vault_exists = vault_exists;
        Ok(())
    }
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = desktop::configure(tauri::Builder::default());
    #[cfg(all(feature = "signed-updater", windows))]
    let builder = update::configure(builder);
    builder
        .manage(AppState::default())
        .on_window_event(desktop::handle_window_event)
        .setup(|app| {
            let app_data_dir = std::env::var_os("AUTHDESK_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            app.state::<AppState>().bind_app_data_dir(&app_data_dir)?;
            desktop::setup(app)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::Emitter;
                let mut last_generation = 0;
                let mut seconds = 0_u32;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let state = handle.state::<AppState>();
                    let _ = state.maintenance_tick();
                    let status = state.vault.status();
                    if status.locked && status.generation != last_generation {
                        if let Some(window) = handle.get_webview_window("quick-panel") {
                            let _ = window.hide();
                        }
                        let _ = handle.emit("vault://locked", ());
                    }
                    last_generation = status.generation;
                    seconds += 1;
                    if seconds >= 60 {
                        seconds = 0;
                        if !status.locked {
                            let _ = state
                                .backups
                                .run_auto_backup(chrono::Utc::now().timestamp());
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_snapshot,
            commands::create_vault,
            commands::unlock_vault,
            commands::vault_status,
            commands::lock_vault,
            commands::integrity_check,
            commands::list_accounts,
            commands::account_details,
            commands::create_account,
            commands::update_account,
            commands::delete_accounts,
            commands::set_favorite,
            commands::move_accounts,
            commands::reorder_accounts,
            commands::generate_codes,
            commands::advance_hotp,
            commands::list_categories,
            commands::save_category,
            commands::delete_category,
            commands::settings,
            commands::save_settings,
            commands::audit_events,
            commands::record_copy,
            commands::dashboard,
            commands::platform_templates,
            commands::preview_import,
            commands::preview_import_file,
            commands::pick_import_file,
            commands::commit_import,
            commands::cancel_import_preview,
            commands::activity_touch,
            commands::change_password,
            commands::backup_create,
            commands::backup_preview_restore,
            commands::backup_apply_restore,
            commands::backup_cancel_preview,
            commands::backup_auto_status,
            commands::backup_configure_auto,
            commands::backup_list,
            commands::security_audit,
            commands::copy_code,
            commands::show_main_window,
            desktop::commands::desktop_capabilities,
            desktop::commands::desktop_config,
            desktop::commands::save_desktop_config,
            desktop::commands::show_quick_panel,
            desktop::commands::quick_panel_accounts,
            desktop::commands::pick_qr_image,
            desktop::commands::test_notification,
            #[cfg(windows)]
            update::update_version,
            #[cfg(windows)]
            update::check_update,
            #[cfg(windows)]
            update::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AUTHENTICATOR DESK");
}
