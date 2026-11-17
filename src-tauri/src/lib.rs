pub mod commands;
mod error;
pub mod otp;
pub mod services;

use std::sync::RwLock;

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
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![commands::app_snapshot])
        .run(tauri::generate_context!())
        .expect("failed to run AUTHENTICATOR DESK");
}
