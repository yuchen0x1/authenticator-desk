use authenticator_desk_lib::{
    AppError,
    accounts::{AccountInput, AccountQuery, AccountService},
    vault::{SecretString, VaultService},
};
use std::sync::Arc;

fn fixture() -> (std::path::PathBuf, Arc<VaultService>, AccountService) {
    let path = std::env::temp_dir().join(format!(
        "authdesk-accounts-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("account-test"), &path)
        .unwrap();
    vault
        .unlock_vault(SecretString::from("account-test"))
        .unwrap();
    let accounts = AccountService::new(vault.clone());
    (path, vault, accounts)
}
fn input(name: &str) -> AccountInput {
    serde_json::from_value(serde_json::json!({"issuer":"GitHub","account_name":name,"secret":"GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"})).unwrap()
}

#[test]
fn encrypted_crud_search_categories_favorites_and_batch_operations() {
    let (path, vault, service) = fixture();
    let category = service.save_category(None, "Work", "#775cff").unwrap();
    let mut first = input("alice@example.com");
    first.category_id = Some(category.id.clone());
    first.tags = vec!["work".into()];
    first.notes = "sensitive account note".into();
    let alice = service.create_account(first).unwrap();
    let bob = service.create_account(input("bob@example.com")).unwrap();
    assert!(matches!(
        service.create_account(input("alice@example.com")),
        Err(AppError::Conflict)
    ));
    assert_eq!(
        service
            .list_accounts(AccountQuery {
                search: "ALICE".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        service
            .list_accounts(AccountQuery {
                tag: Some("work".into()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        service
            .list_accounts(AccountQuery {
                category_id: Some(category.id.clone()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    service.set_favorite(&alice.id, true).unwrap();
    assert_eq!(
        service
            .list_accounts(AccountQuery {
                favorite_only: true,
                ..Default::default()
            })
            .unwrap()[0]
            .id,
        alice.id
    );
    let mut edit = input("alice-renamed");
    edit.secret = None;
    service.update_account(&alice.id, edit).unwrap();
    assert_eq!(
        service.generate_codes_at(&[alice.id.clone()], 59).unwrap()[0].code,
        "287082"
    );
    service
        .reorder_accounts(&[bob.id.clone(), alice.id.clone()])
        .unwrap();
    assert_eq!(
        service.list_accounts(AccountQuery::default()).unwrap()[0].id,
        bob.id
    );
    service
        .move_accounts(&[bob.id.clone()], Some(category.id.clone()))
        .unwrap();
    service.delete_category(&category.id).unwrap();
    assert!(
        service
            .list_accounts(AccountQuery::default())
            .unwrap()
            .iter()
            .all(|a| a.category_id.is_none())
    );
    let encoded =
        serde_json::to_string(&service.list_accounts(AccountQuery::default()).unwrap()).unwrap();
    assert!(!encoded.contains("secret"));
    assert!(!encoded.contains("sensitive account note"));
    let bytes = std::fs::read(&path).unwrap();
    for value in [
        "GEZDGNBVGY3TQOJQ",
        "sensitive account note",
        "alice-renamed",
    ] {
        assert!(!bytes.windows(value.len()).any(|w| w == value.as_bytes()));
    }
    service.delete_accounts(&[alice.id, bob.id]).unwrap();
    assert!(
        service
            .list_accounts(AccountQuery::default())
            .unwrap()
            .is_empty()
    );
    vault.lock_vault().unwrap();
    assert!(matches!(
        service.list_accounts(AccountQuery::default()),
        Err(AppError::VaultLocked)
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn hotp_persists_monotonic_counter_and_rejects_lossy_or_overflow_values() {
    let (path, vault, service) = fixture();
    let mut item = input("hotp");
    item.kind = "hotp".into();
    let account = service.create_account(item).unwrap();
    assert_eq!(
        service.generate_codes_at(&[account.id.clone()], 0).unwrap()[0].code,
        "755224"
    );
    assert_eq!(service.advance_hotp(&account.id).unwrap().code, "287082");
    let mut backward = input("hotp");
    backward.kind = "hotp".into();
    backward.hotp_counter = "0".into();
    assert!(service.update_account(&account.id, backward).is_err());
    vault.lock_vault().unwrap();
    vault
        .unlock_vault(SecretString::from("account-test"))
        .unwrap();
    assert_eq!(
        service.generate_codes_at(&[account.id.clone()], 0).unwrap()[0].code,
        "287082"
    );
    let mut large = input("big");
    large.kind = "hotp".into();
    large.hotp_counter = u64::MAX.to_string();
    let large = service.create_account(large).unwrap();
    assert!(service.advance_hotp(&large.id).is_err());
    assert_eq!(
        service
            .account_details(&large.id)
            .unwrap()
            .account
            .hotp_counter,
        u64::MAX.to_string()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn settings_are_validated_persisted_and_activity_has_no_codes_or_secrets() {
    let (path, vault, service) = fixture();
    let mut settings = service.settings().unwrap();
    settings.locale = "ja".into();
    settings.clipboard_ttl_secs = 17;
    settings.idle_timeout_secs = 60;
    service.save_settings(settings.clone()).unwrap();
    assert_eq!(service.settings().unwrap().locale, "ja");
    settings.locale = "bad".into();
    assert!(service.save_settings(settings).is_err());
    let item = service.create_account(input("activity")).unwrap();
    service.record_copy(&item.id).unwrap();
    let stats = service.dashboard().unwrap();
    assert_eq!(stats.account_count, 1);
    assert_eq!(stats.copies_today, 1);
    let events = serde_json::to_string(&service.audit_events(100).unwrap()).unwrap();
    assert!(!events.contains("755224"));
    assert!(!events.contains("GEZDG"));
    vault.lock_vault().unwrap();
    vault
        .unlock_vault(SecretString::from("account-test"))
        .unwrap();
    assert_eq!(service.settings().unwrap().clipboard_ttl_secs, 17);
    let _ = std::fs::remove_file(path);
}
