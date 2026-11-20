use authenticator_desk_lib::{AppError, AppState, accounts::AccountInput, vault::SecretString};

#[test]
fn hidden_codes_are_not_returned_and_hotp_is_not_advanced() {
    let directory = std::env::temp_dir().join(format!("authdesk-privacy-{}", uuid::Uuid::new_v4()));
    let state = AppState::default();
    state.bind_app_data_dir(&directory).unwrap();
    state
        .vault
        .create_vault(
            SecretString::from("privacy-test-password"),
            &directory.join("vault.sqlite3"),
        )
        .unwrap();
    state
        .vault
        .unlock_vault(SecretString::from("privacy-test-password"))
        .unwrap();
    let input: AccountInput = serde_json::from_value(serde_json::json!({
        "issuer":"Privacy fixture", "account_name":"demo", "secret":"GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", "kind":"hotp", "hotp_counter":"0"
    })).unwrap();
    let account = state.accounts.create_account(input).unwrap();
    let ids = vec![account.id.clone()];
    assert_eq!(state.visible_codes(&ids).unwrap().len(), 1);

    let mut settings = state.accounts.settings().unwrap();
    settings.hide_codes = true;
    state.save_preferences(settings.clone()).unwrap();
    assert!(state.visible_codes(&ids).unwrap().is_empty());
    assert!(matches!(
        state.advance_visible_hotp(&account.id),
        Err(AppError::InvalidParameters)
    ));
    assert_eq!(
        state
            .accounts
            .account_details(&account.id)
            .unwrap()
            .account
            .hotp_counter,
        "0"
    );
    // Native protected-copy can still generate without returning plaintext through the display API.
    assert_eq!(state.accounts.generate_codes(&ids).unwrap().len(), 1);

    settings.hide_codes = false;
    state.save_preferences(settings).unwrap();
    assert_eq!(
        state.advance_visible_hotp(&account.id).unwrap().account_id,
        account.id
    );
    state.lock().unwrap();
    assert!(matches!(
        state.visible_codes(&ids),
        Err(AppError::VaultLocked)
    ));
    drop(state);
    std::fs::remove_dir_all(directory).unwrap();
}
