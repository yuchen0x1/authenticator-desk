use crate::{AppError, AppResult};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn install(app: &tauri::AppHandle) -> AppResult<()> {
    let show = MenuItem::with_id(app, "show", "Show AUTHENTICATOR DESK", true, None::<&str>)
        .map_err(|_| AppError::Storage)?;
    let quick = MenuItem::with_id(app, "quick", "Favorite codes", true, None::<&str>)
        .map_err(|_| AppError::Storage)?;
    let lock = MenuItem::with_id(app, "lock", "Lock vault", true, None::<&str>)
        .map_err(|_| AppError::Storage)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|_| AppError::Storage)?;
    let menu =
        Menu::with_items(app, &[&show, &quick, &lock, &quit]).map_err(|_| AppError::Storage)?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or(AppError::NotFound)?;
    TrayIconBuilder::with_id("authenticator-desk")
        .icon(icon)
        .tooltip("AUTHENTICATOR DESK")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                let _ = super::show_main(app);
            }
            "quick" => {
                let _ = super::show_quick_panel(app);
            }
            "lock" => {
                let _ = super::lock(app);
            }
            "quit" => super::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = super::show_main(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|_| AppError::Storage)?;
    Ok(())
}
