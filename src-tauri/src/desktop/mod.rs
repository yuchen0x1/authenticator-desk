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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainCloseAction {
    Hide,
    Exit,
}

pub fn main_close_action(config: &DesktopConfig, tray_available: bool) -> MainCloseAction {
    if should_hide_main(config, tray_available, false) {
        MainCloseAction::Hide
    } else {
        MainCloseAction::Exit
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
    // The vault is still locked here, so the encrypted settings cannot be read.
    // Start in the last chosen appearance instead of flashing the default theme.
    if let Ok(data_dir) = handle.state::<AppState>().data_dir() {
        let theme = read_theme_hint(&data_dir);
        apply_theme(handle, theme);
    }
    Ok(())
}

/// The window theme drives the title bar and WebView2's native widgets (the
/// `<select>` popup, default scrollbars). The page's own colors come from CSS
/// tokens, so this only mirrors the saved preference into the host window.
#[cfg(all(feature = "desktop", target_os = "windows"))]
pub fn apply_theme(app: &tauri::AppHandle, theme: &str) {
    let theme = if theme == "light" {
        tauri::Theme::Light
    } else {
        tauri::Theme::Dark
    };
    for window in app.webview_windows().values() {
        let _ = window.set_theme(Some(theme));
    }
}

#[cfg(all(feature = "desktop", not(target_os = "windows")))]
pub fn apply_theme(_app: &tauri::AppHandle, _theme: &str) {}

/// The appearance choice lives inside the encrypted settings, which cannot be read
/// while the vault is locked. This tiny non-secret hint file keeps the locked
/// window and its title bar in the appearance the user last chose. It holds one of
/// two words and no account data, so the plaintext scan (which matches known
/// secrets) never flags it.
pub const THEME_HINT_FILE: &str = "ui.json";

pub fn normalize_theme(theme: &str) -> &'static str {
    if theme == "light" {
        "light"
    } else {
        "dark"
    }
}

pub fn save_theme_hint(data_dir: &std::path::Path, theme: &str) -> std::io::Result<()> {
    use std::io::Write;
    let body = format!("{{\"theme\":\"{}\"}}\n", normalize_theme(theme));
    let temporary = data_dir.join(format!(".ui-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    let result = (|| {
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        drop(file);
        // Replace the directory entry rather than following an existing link.
        std::fs::rename(&temporary, data_dir.join(THEME_HINT_FILE))
    })();
    if result.is_err() { let _ = std::fs::remove_file(temporary); }
    result
}

pub fn read_theme_hint(data_dir: &std::path::Path) -> &'static str {
    #[derive(serde::Deserialize)]
    struct Hint {
        theme: String,
    }
    use std::io::Read;
    let read = || -> std::io::Result<Vec<u8>> {
        let path = data_dir.join(THEME_HINT_FILE);
        if !std::fs::symlink_metadata(&path)?.file_type().is_file() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.take(65).read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    read().ok()
        .filter(|bytes| bytes.len() <= 64)
        .and_then(|bytes| serde_json::from_slice::<Hint>(&bytes).ok())
        .map(|hint| normalize_theme(&hint.theme))
        .unwrap_or("dark")
}

#[cfg(feature = "desktop")]
pub fn show_main(app: &tauri::AppHandle) -> AppResult<()> {    let window = app.get_webview_window("main").ok_or(AppError::NotFound)?;
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
pub fn quit(app: &tauri::AppHandle) {
    if lock(app).is_err() {
        // Attempt both cleanups even if an earlier AppState::lock step failed.
        let state = app.state::<AppState>();
        let _ = state.vault.lock_vault();
        let _ = state.clipboard.clear_owned();
    }
    app.exit(0);
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
    let (close_action, hide_on_minimize) = {
        let state = app.state::<DesktopState>();
        match (state.config.read(), state.capabilities.read()) {
            (Ok(config), Ok(caps)) => (
                main_close_action(&config, caps.tray),
                should_hide_main(&config, caps.tray, true),
            ),
            _ => (MainCloseAction::Exit, false),
        }
    };
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            match close_action {
                MainCloseAction::Hide => {
                    let _ = window.hide();
                }
                // Closing only the main window leaves an existing hidden quick panel alive.
                MainCloseAction::Exit => quit(app),
            }
        }
        tauri::WindowEvent::Resized(_)
            if window.is_minimized().unwrap_or(false) && hide_on_minimize =>
        {
            let _ = window.hide();
        }
        _ => {}
    }
}

#[cfg(test)]
mod theme_hint_tests {
    use super::{read_theme_hint, save_theme_hint, THEME_HINT_FILE};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "authdesk-theme-hint-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trips_both_appearances() {
        let dir = temp_dir("round-trip");
        assert_eq!(read_theme_hint(&dir), "dark");
        save_theme_hint(&dir, "light").unwrap();
        assert_eq!(read_theme_hint(&dir), "light");
        save_theme_hint(&dir, "dark").unwrap();
        assert_eq!(read_theme_hint(&dir), "dark");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_and_oversized_content() {
        let dir = temp_dir("reject");
        std::fs::write(dir.join(THEME_HINT_FILE), "{\"theme\":\"sepia\"}").unwrap();
        assert_eq!(read_theme_hint(&dir), "dark");
        save_theme_hint(&dir, "sepia").unwrap();
        assert_eq!(read_theme_hint(&dir), "dark");
        std::fs::write(dir.join(THEME_HINT_FILE), "x".repeat(4096)).unwrap();
        assert_eq!(read_theme_hint(&dir), "dark");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
