use authenticator_desk_lib::desktop::{DesktopConfig, should_hide_main};
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
}
