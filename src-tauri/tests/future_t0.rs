use authenticator_desk_lib::{
    AppError,
    accounts::{AccountInput, AccountService},
    vault::{SecretString, VaultService},
};
use std::sync::Arc;

fn fixture() -> (std::path::PathBuf, Arc<VaultService>, AccountService) {
    let path = std::env::temp_dir().join(format!(
        "authdesk-future-t0-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("future-t0-test"), &path)
        .unwrap();
    vault
        .unlock_vault(SecretString::from("future-t0-test"))
        .unwrap();
    let service = AccountService::new(vault.clone());
    (path, vault, service)
}

fn input(name: &str, kind: &str, t0: &str) -> AccountInput {
    serde_json::from_value(serde_json::json!({
        "issuer": "Test",
        "account_name": name,
        // Public RFC 4226 vector, derived rather than stored as a credential-shaped literal.
        "secret": authenticator_desk_lib::otp::encode_base32(b"12345678901234567890").as_str(),
        "kind": kind,
        "t0": t0
    }))
    .unwrap()
}

#[test]
fn available_codes_skip_only_future_totp_and_keep_active_mixed_accounts() {
    let (path, vault, service) = fixture();
    let future = service
        .create_account(input("future", "totp", "60"))
        .unwrap();
    let totp = service
        .create_account(input("active", "totp", "0"))
        .unwrap();
    let hotp = service
        .create_account(input("counter", "hotp", "0"))
        .unwrap();
    let ids = vec![future.id.clone(), totp.id.clone(), hotp.id.clone()];

    let before = service.generate_available_codes_at(&ids, 59).unwrap();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].account_id, totp.id);
    assert_eq!(before[0].code, "287082");
    assert_eq!(before[1].account_id, hotp.id);
    assert_eq!(before[1].code, "755224");
    assert!(matches!(
        service.generate_codes_at(&[future.id.clone()], 59),
        Err(AppError::InvalidParameters)
    ));

    let at_boundary = service.generate_available_codes_at(&ids, 60).unwrap();
    assert_eq!(at_boundary.len(), 3);
    assert_eq!(at_boundary[0].account_id, future.id);
    assert_eq!(at_boundary[0].code, "755224");
    assert_eq!(at_boundary[1].code, "359152");
    assert_eq!(at_boundary[2].code, "755224");

    assert!(matches!(
        service.generate_available_codes_at(&["missing-account".into()], 59),
        Err(AppError::NotFound)
    ));
    assert!(matches!(
        service.generate_available_codes_at(&[future.id.clone(), future.id.clone()], 59),
        Err(AppError::InvalidInput)
    ));
    vault.lock_vault().unwrap();
    let _ = std::fs::remove_file(path);
}
