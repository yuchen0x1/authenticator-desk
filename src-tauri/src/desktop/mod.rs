use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
pub mod commands;
#[cfg(feature = "desktop")]
pub mod hotkeys;
#[cfg(feature = "desktop")]
pub mod startup;
#[cfg(feature = "desktop")]
pub mod tray;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DesktopConfig {
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub startup: bool,
    pub notifications: bool,
    pub show_shortcut: String,
    pub quick_panel_shortcut: String,
    pub lock_shortcut: String,
}
impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            close_to_tray: false,
            minimize_to_tray: false,
            startup: false,
            notifications: false,
            show_shortcut: "Ctrl+Shift+A".into(),
            quick_panel_shortcut: "Ctrl+Shift+Q".into(),
            lock_shortcut: "Ctrl+Shift+L".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DesktopCapabilities {
    pub tray: bool,
    pub shortcuts: bool,
    pub startup: bool,
    pub notifications: bool,
    pub clipboard: bool,
    pub qr_image: bool,
    pub camera: bool,
    pub camera_reason: String,
    pub warnings: Vec<String>,
}

pub fn should_hide_main(config: &DesktopConfig, tray_available: bool, minimized: bool) -> bool {
    tray_available
        && if minimized {
            config.minimize_to_tray
        } else {
            config.close_to_tray
        }
}

#[cfg(feature = "desktop")]
use crate::{AppError, AppResult, AppState};
#[cfg(feature = "desktop")]
use std::sync::RwLock;
#[cfg(feature = "desktop")]
use tauri::{Emitter, Manager};

#[cfg(feature = "desktop")]
#[derive(Default)]
pub struct DesktopState {
    pub config: RwLock<DesktopConfig>,
    pub capabilities: RwLock<DesktopCapabilities>,
}

#[cfg(feature = "desktop")]
pub fn configure(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .manage(DesktopState::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("AUTHENTICATOR DESK")
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
}

#[cfg(feature = "desktop")]
pub fn setup(app: &mut tauri::App) -> AppResult<()> {
    let handle = app.handle();
    let config_path = handle.state::<AppState>().data_dir()?.join("desktop.json");
    let config = std::fs::read(&config_path)
        .ok()
        .filter(|b| b.len() <= 16384)
        .and_then(|b| serde_json::from_slice::<DesktopConfig>(&b).ok())
        .unwrap_or_default();
    let state = handle.state::<DesktopState>();
    let mut capabilities = DesktopCapabilities {
        clipboard: cfg!(windows),
        qr_image: true,
        camera: false,
        camera_reason: "Camera capture is not implemented; choose a QR image instead.".into(),
        ..Default::default()
    };
    capabilities.tray = tray::install(handle).is_ok();
    if !capabilities.tray {
        capabilities.warnings.push("tray_unavailable".into());
    }
    capabilities.shortcuts = hotkeys::register(handle, &config).is_ok();
    if !capabilities.shortcuts {
        capabilities
            .warnings
            .push("shortcut_registration_failed".into());
    }
    capabilities.startup =
        std::env::var_os("AUTHDESK_DATA_DIR").is_none() && startup::is_enabled(handle).is_ok();
    use tauri_plugin_notification::NotificationExt;
    capabilities.notifications = matches!(
        handle.notification().permission_state(),
        Ok(tauri_plugin_notification::PermissionState::Granted)
    );
    *state.config.write().map_err(|_| AppError::Storage)? = config;
    *state.capabilities.write().map_err(|_| AppError::Storage)? = capabilities;
    Ok(())
}

#[cfg(feature = "desktop")]
pub fn show_main(app: &tauri::AppHandle) -> AppResult<()> {
    let window = app.get_webview_window("main").ok_or(AppError::NotFound)?;
    window.unminimize().map_err(|_| AppError::Storage)?;
    window.show().map_err(|_| AppError::Storage)?;
    window.set_focus().map_err(|_| AppError::Storage)
}

#[cfg(feature = "desktop")]
pub fn show_quick_panel(app: &tauri::AppHandle) -> AppResult<()> {
    if app.state::<AppState>().vault.is_locked() {
        return show_main(app);
    }
    let window = match app.get_webview_window("quick-panel") {
        Some(window) => window,
        None => tauri::WebviewWindowBuilder::new(
            app,
            "quick-panel",
            tauri::WebviewUrl::App("index.html?quick-panel=1".into()),
        )
        .title("AUTHENTICATOR DESK — Favorites")
        .inner_size(420.0, 560.0)
        .min_inner_size(340.0, 360.0)
        .resizable(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .build()
        .map_err(|_| AppError::Storage)?,
    };
    window.show().map_err(|_| AppError::Storage)?;
    window.set_focus().map_err(|_| AppError::Storage)
}

#[cfg(feature = "desktop")]
pub fn lock(app: &tauri::AppHandle) -> AppResult<()> {
    app.state::<AppState>().lock()?;
    if let Some(window) = app.get_webview_window("quick-panel") {
        let _ = window.hide();
    }
    app.emit("vault://locked", ())
        .map_err(|_| AppError::Storage)
}

#[cfg(feature = "desktop")]
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == "quick-panel" {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.hide();
        }
        return;
    }
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    let state = app.state::<DesktopState>();
    let (Ok(config), Ok(caps)) = (state.config.read(), state.capabilities.read()) else {
        return;
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. }
            if should_hide_main(&config, caps.tray, false) =>
        {
            api.prevent_close();
            let _ = window.hide();
        }
        tauri::WindowEvent::Resized(_)
            if window.is_minimized().unwrap_or(false)
                && should_hide_main(&config, caps.tray, true) =>
        {
            let _ = window.hide();
        }
        _ => {}
    }
}
