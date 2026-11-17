use authenticator_desk_lib::{AppError, AppSnapshot, AppState};

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
