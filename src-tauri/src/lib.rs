pub mod commands;
mod error;
pub mod otp;
pub mod services;
pub mod storage;
pub mod vault;

use std::sync::{Arc, RwLock};

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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snapshot: RwLock::new(AppSnapshot {
                locked: true,
                vault_exists: false,
                locale: "zh-CN".to_owned(),
                theme: "dark".to_owned(),
            }),
            vault: Arc::new(vault::VaultService::default()),
        }
    }
}

impl AppState {
    pub fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
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
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_snapshot,
            commands::create_vault,
            commands::unlock_vault,
            commands::vault_status,
            commands::lock_vault,
            commands::integrity_check,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AUTHENTICATOR DESK");
}
