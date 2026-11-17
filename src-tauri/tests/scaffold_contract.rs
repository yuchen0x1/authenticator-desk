use authenticator_desk_lib::vault::{SecretString, create_vault};
use authenticator_desk_lib::{AppError, AppSnapshot, AppState};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn starts_with_a_locked_vault_snapshot() {
    let snapshot = AppState::default()
        .snapshot()
        .expect("snapshot is available");

    assert_eq!(
        snapshot,
        AppSnapshot {
            locked: true,
            vault_exists: false,
            locale: "zh-CN".to_owned(),
            theme: "dark".to_owned(),
        }
    );
}

#[test]
fn startup_binds_existing_vault_from_backend_app_data_dir() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let app_data_dir = std::env::temp_dir().join(format!("authdesk-startup-{suffix}"));
    std::fs::create_dir_all(&app_data_dir).unwrap();
    create_vault(
        SecretString::from("startup-password"),
        &app_data_dir.join("vault.sqlite3"),
    )
    .unwrap();

    let restarted = AppState::default();
    restarted.bind_app_data_dir(&app_data_dir).unwrap();
    assert!(restarted.snapshot().unwrap().vault_exists);
    assert!(restarted.vault.status().exists);
    assert!(restarted.vault.status().locked);
    assert!(restarted.vault.integrity_check().unwrap().ok);
    restarted
        .vault
        .unlock_vault(SecretString::from("startup-password"))
        .unwrap();
    assert!(!restarted.snapshot().unwrap().locked);
    restarted.vault.lock_vault().unwrap();
    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[test]
fn serializes_only_stable_error_codes_and_safe_messages() {
    let cases = [
        (
            AppError::VaultLocked,
            "vault_locked",
            "The vault is locked.",
        ),
        (
            AppError::InvalidInput,
            "invalid_input",
            "The provided input is invalid.",
        ),
        (
            AppError::AuthenticationFailed,
            "authentication_failed",
            "Authentication failed.",
        ),
        (
            AppError::Storage,
            "storage",
            "The local vault could not be accessed.",
        ),
        (
            AppError::Crypto,
            "crypto",
            "The secure operation could not be completed.",
        ),
        (
            AppError::Conflict,
            "conflict",
            "The requested change conflicts with existing data.",
        ),
        (
            AppError::NotFound,
            "not_found",
            "The requested item was not found.",
        ),
    ];

    for (error, code, message) in cases {
        let payload = serde_json::to_value(error).expect("application errors serialize");
        assert_eq!(
            payload,
            serde_json::json!({ "code": code, "message": message })
        );
    }
}
