use authenticator_desk_lib::desktop::{
    DesktopConfig, MainCloseAction, main_close_action, should_hide_main,
};

#[test]
fn main_close_exits_unless_close_to_tray_is_enabled_and_available() {
    // A quick panel may already exist but be hidden: normal main-window destruction
    // is not a substitute for explicitly exiting the application.
    for close_to_tray in [false, true] {
        for tray_available in [false, true] {
            for minimize_to_tray in [false, true] {
                let config = DesktopConfig {
                    close_to_tray,
                    minimize_to_tray,
                    ..DesktopConfig::default()
                };
                let expected = if close_to_tray && tray_available {
                    MainCloseAction::Hide
                } else {
                    MainCloseAction::Exit
                };
                assert_eq!(main_close_action(&config, tray_available), expected);
            }
        }
    }
}

#[test]
fn close_and_minimize_preferences_require_tray_capability() {
    let mut config = DesktopConfig::default();
    assert!(!should_hide_main(&config, false, false));
    config.close_to_tray = true;
    assert!(should_hide_main(&config, true, false));
    assert!(!should_hide_main(&config, false, false));
    config.close_to_tray = false;
    config.minimize_to_tray = true;
    assert!(should_hide_main(&config, true, true));
    assert!(!should_hide_main(&config, true, false));
    assert!(!should_hide_main(&config, false, true));
}

#[cfg(feature = "desktop")]
#[test]
fn shortcut_validation_requires_safe_modifiers_for_every_action() {
    use authenticator_desk_lib::{AppError, desktop::hotkeys};

    for text in [
        "A",
        "KeyA",
        "Digit1",
        "Space",
        "Enter",
        "F12",
        "Shift+A",
        "Shift+F12",
    ] {
        for action in 0..3 {
            let mut config = DesktopConfig::default();
            match action {
                0 => config.show_shortcut = text.into(),
                1 => config.quick_panel_shortcut = text.into(),
                _ => config.lock_shortcut = text.into(),
            }
            assert!(
                matches!(hotkeys::validate(&config), Err(AppError::InvalidInput)),
                "action={action}, shortcut={text}",
            );
        }
    }
}

#[cfg(feature = "desktop")]
#[test]
fn shortcut_validation_allows_safe_modifiers_and_disabled_actions() {
    use authenticator_desk_lib::desktop::hotkeys;

    for text in [
        "Ctrl+K",
        "Alt+K",
        "Super+K",
        "CommandOrControl+K",
        "Ctrl+Shift+K",
        "",
    ] {
        let config = DesktopConfig {
            show_shortcut: text.into(),
            quick_panel_shortcut: String::new(),
            lock_shortcut: String::new(),
            ..DesktopConfig::default()
        };
        assert!(hotkeys::validate(&config).is_ok(), "shortcut={text}");
    }
}

#[cfg(feature = "desktop")]
#[test]
fn shortcut_validation_rejects_collisions_and_invalid_accelerators() {
    use authenticator_desk_lib::desktop::hotkeys;
    let mut config = DesktopConfig::default();
    assert!(hotkeys::validate(&config).is_ok());
    config.lock_shortcut = config.show_shortcut.clone();
    assert!(hotkeys::validate(&config).is_err());
    config.lock_shortcut = "not a shortcut".into();
    assert!(hotkeys::validate(&config).is_err());
    config.lock_shortcut.clear();
    assert!(hotkeys::validate(&config).is_ok());
    config.lock_shortcut = "Control+Shift+KeyA".into();
    assert!(hotkeys::validate(&config).is_err());
    config.lock_shortcut = "Ctrl+".into();
    assert!(hotkeys::validate(&config).is_err());
    config.lock_shortcut = "A".repeat(101);
    assert!(hotkeys::validate(&config).is_err());
}
