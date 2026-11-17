use super::DesktopConfig;
use crate::{AppError, AppResult};
use std::str::FromStr;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn validate(config: &DesktopConfig) -> AppResult<()> {
    let mut ids = Vec::new();
    for text in [
        &config.show_shortcut,
        &config.quick_panel_shortcut,
        &config.lock_shortcut,
    ] {
        if text.is_empty() {
            continue;
        }
        if text.len() > 100 {
            return Err(AppError::InvalidInput);
        }
        let shortcut = Shortcut::from_str(text).map_err(|_| AppError::InvalidInput)?;
        if ids.contains(&shortcut.id()) {
            return Err(AppError::InvalidInput);
        }
        ids.push(shortcut.id());
    }
    Ok(())
}

pub fn register(app: &tauri::AppHandle, config: &DesktopConfig) -> AppResult<()> {
    validate(config)?;
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|_| AppError::Storage)?;
    for (action, text) in [
        (0, &config.show_shortcut),
        (1, &config.quick_panel_shortcut),
        (2, &config.lock_shortcut),
    ] {
        if text.is_empty() {
            continue;
        }
        let result = manager.on_shortcut(text.as_str(), move |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let _ = match action {
                0 => super::show_main(app),
                1 => super::show_quick_panel(app),
                _ => super::lock(app),
            };
        });
        if result.is_err() {
            let _ = manager.unregister_all();
            return Err(AppError::Conflict);
        }
    }
    Ok(())
}
