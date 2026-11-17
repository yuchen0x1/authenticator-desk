use authenticator_desk_lib::import::{
    AegisAdapter, AndOtpAdapter, EnteAdapter, FreeOtpAdapter, GoogleMigrationAdapter,
    ImportService, MigrationAdapter, OtpAuthListAdapter, TwoFasAdapter,
};
use authenticator_desk_lib::{
    AppError,
    accounts::{AccountInput, AccountQuery, AccountService},
    vault::{SecretString, VaultService},
};
use std::{path::PathBuf, sync::Arc};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/import")
            .join(name),
    )
    .expect("fixture file exists")
}

fn harness() -> (PathBuf, Arc<VaultService>, Arc<AccountService>, ImportService) {
    let path = std::env::temp_dir().join(format!(
        "authdesk-migration-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("migration-test"), &path)
        .unwrap();
    vault.unlock_vault(SecretString::from("migration-test")).unwrap();
    let accounts = Arc::new(AccountService::new(vault.clone()));
    let imports = ImportService::new(accounts.clone());
    (path, vault, accounts, imports)
}

fn input(issuer: &str, account: &str, secret: &str) -> AccountInput {
    serde_json::from_value(serde_json::json!({
        "issuer": issuer,
        "account_name": account,
        "secret": secret,
    }))
    .unwrap()
}

#[test]
fn google_migration_adapter_maps_batch_parameters() {
    let adapter = GoogleMigrationAdapter;
    let preview = MigrationAdapter::preview(&adapter, &fixture("google_migration_multi.txt"), None)
        .unwrap();
    assert_eq!(preview.format, "google_migration");
    assert_eq!(preview.items.len(), 3);
    let alice = preview.items[0].input.as_ref().unwrap();
    assert_eq!(alice.issuer, "Demo");
    assert_eq!(alice.account_name, "alice@example.com");
    assert_eq!(alice.kind, "totp");
    assert_eq!(alice.algorithm, "SHA256");
    assert_eq!(alice.digits, 6);
    let hotp = preview.items[1].input.as_ref().unwrap();
    assert_eq!(hotp.kind, "hotp");
    assert_eq!(hotp.hotp_counter, "5");
    let sha512 = preview.items[2].input.as_ref().unwrap();
    assert_eq!(sha512.algorithm, "SHA512");
    assert_eq!(sha512.digits, 8);
    assert!(adapter.detect(b"otpauth-migration://offline?data=x"));
    assert!(!adapter.detect(b"otpauth://totp/Demo"));
}

#[test]
fn rejects_malformed_google_migration_payload() {
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(fixture("google_migration_malformed.txt")),
        Err(AppError::InvalidInput)
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn aegis_adapter_maps_entries_and_flags_steam_unsupported() {
    let adapter = AegisAdapter;
    let preview =
        MigrationAdapter::preview(&adapter, &fixture("aegis_plain.json"), None).unwrap();
    assert_eq!(preview.format, "aegis");
    assert_eq!(preview.items.len(), 4);
    let alice = preview.items[0].input.as_ref().unwrap();
    assert_eq!(alice.account_name, "alice@example.com");
    assert_eq!(alice.issuer, "Demo");
    assert_eq!(alice.algorithm, "SHA1");
    assert_eq!(alice.digits, 6);
    assert_eq!(alice.period_secs, 30);
    let bob = preview.items[1].input.as_ref().unwrap();
    assert_eq!(bob.algorithm, "SHA256");
    assert_eq!(bob.digits, 8);
    assert_eq!(bob.period_secs, 60);
    let carol = preview.items[2].input.as_ref().unwrap();
    assert_eq!(carol.kind, "hotp");
    assert_eq!(carol.hotp_counter, "7");
    let steam = &preview.items[3];
    assert!(steam.input.is_none());
    assert_eq!(steam.error_code.as_deref(), Some("unsupported_steam"));
    assert_eq!(steam.account_name.as_deref(), Some("steam@example.com"));
    assert!(!adapter.detect(&fixture("twofas_plain.json")));
}

#[test]
fn rejects_encrypted_aegis_with_and_without_password() {
    let (path, _vault, _accounts, imports) = harness();
    let bytes = fixture("aegis_encrypted.json");
    assert!(matches!(
        imports.preview_file(bytes.clone()),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let adapter = AegisAdapter;
    let password = SecretString::from("supplied-password");
    assert!(matches!(
        MigrationAdapter::preview(&adapter, &bytes, Some(&password)),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn twofas_adapter_applies_defaults_and_rejects_steam_tokens() {
    let adapter = TwoFasAdapter;
    let preview =
        MigrationAdapter::preview(&adapter, &fixture("twofas_plain.json"), None).unwrap();
    assert_eq!(preview.format, "twofas");
    assert_eq!(preview.items.len(), 3);
    let defaults = preview.items[0].input.as_ref().unwrap();
    assert_eq!(defaults.issuer, "Demo");
    assert_eq!(defaults.account_name, "alice@example.com");
    assert_eq!(defaults.algorithm, "SHA1");
    assert_eq!(defaults.digits, 6);
    assert_eq!(defaults.period_secs, 30);
    assert_eq!(defaults.kind, "totp");
    let hotp = preview.items[1].input.as_ref().unwrap();
    assert_eq!(hotp.kind, "hotp");
    assert_eq!(hotp.algorithm, "SHA512");
    assert_eq!(hotp.digits, 8);
    assert_eq!(hotp.period_secs, 60);
    assert_eq!(hotp.hotp_counter, "3");
    let steam = &preview.items[2];
    assert!(steam.input.is_none());
    assert_eq!(steam.error_code.as_deref(), Some("unsupported_token_type"));
    assert!(adapter.detect(&fixture("twofas_plain.json")));
    assert!(!adapter.detect(&fixture("aegis_plain.json")));
}

#[test]
fn rejects_encrypted_twofas_backup() {
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(fixture("twofas_encrypted.json")),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn andotp_adapter_maps_entries_and_rejects_aes_envelope() {
    let adapter = AndOtpAdapter;
    let plain = MigrationAdapter::preview(&adapter, &fixture("andotp_plain.json"), None).unwrap();
    assert_eq!(plain.format, "andotp");
    assert_eq!(plain.items.len(), 3);
    let alice = plain.items[0].input.as_ref().unwrap();
    assert_eq!(alice.account_name, "alice@example.com");
    assert_eq!(alice.tags, vec!["work".to_owned(), "imported".to_owned()]);
    let bob = plain.items[1].input.as_ref().unwrap();
    assert_eq!(bob.kind, "hotp");
    assert_eq!(bob.algorithm, "SHA256");
    assert_eq!(bob.hotp_counter, "9");
    let steam = &plain.items[2];
    assert_eq!(steam.error_code.as_deref(), Some("unsupported_steam"));
    assert!(adapter.detect(&fixture("andotp_encrypted.aes")));
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(fixture("andotp_encrypted.aes")),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn rejects_freeotp_backups_as_keystore_bound() {
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(fixture("freeotp_backup.xml")),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    assert!(matches!(
        imports.preview_file(fixture("freeotp_serialized.bin")),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let adapter = FreeOtpAdapter;
    let password = SecretString::from("supplied-password");
    assert!(matches!(
        MigrationAdapter::preview(&adapter, &fixture("freeotp_backup.xml"), Some(&password)),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn rejects_encrypted_ente_export_and_reads_plain_list() {
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(fixture("ente_encrypted.json")),
        Err(AppError::UnsupportedEncryptedFormat)
    ));
    let adapter = EnteAdapter;
    assert!(adapter.detect(&fixture("ente_encrypted.json")));
    assert!(!adapter.detect(&fixture("aegis_plain.json")));
    let plain = imports.preview_file(fixture("ente_plain.txt")).unwrap();
    assert_eq!(plain.format, "otpauth_list");
    assert_eq!(plain.items.len(), 2);
    assert!(plain.items.iter().all(|item| item.status == "new"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn marks_invalid_rows_in_generic_lists() {
    let (path, _vault, _accounts, imports) = harness();
    let adapter = OtpAuthListAdapter;
    assert!(adapter.detect(&fixture("otpauth_mixed.txt")));
    let preview = imports.preview_file(fixture("otpauth_mixed.txt")).unwrap();
    assert_eq!(preview.items.len(), 2);
    assert_eq!(preview.items[0].status, "new");
    assert_eq!(preview.items[1].status, "invalid");
    assert_eq!(
        preview.items[1].error_code.as_deref(),
        Some("invalid_otpauth")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn classifies_duplicates_and_commits_selection_transactionally() {
    let (path, _vault, accounts, imports) = harness();
    accounts
        .create_account(input("Demo", "alice@example.com", "JBSWY3DPEHPK3PXP"))
        .unwrap();
    let preview = imports.preview_file(fixture("aegis_plain.json")).unwrap();
    let statuses: Vec<&str> = preview.items.iter().map(|item| item.status.as_str()).collect();
    assert_eq!(statuses, vec!["duplicate", "new", "new", "invalid"]);
    let committed = imports.commit_import(&preview.preview_id, &[1, 2]).unwrap();
    assert_eq!(committed, 2);
    let all = accounts.list_accounts(AccountQuery::default()).unwrap();
    assert_eq!(all.len(), 3);
    let bob = all.iter().find(|a| a.account_name == "bob@example.com").unwrap();
    assert_eq!(bob.algorithm, "SHA256");
    assert_eq!(bob.digits, 8);
    assert!(imports.commit_import(&preview.preview_id, &[1]).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn rejects_unknown_and_oversized_files() {
    let (path, _vault, _accounts, imports) = harness();
    assert!(matches!(
        imports.preview_file(b"hello world".to_vec()),
        Err(AppError::InvalidInput)
    ));
    assert!(matches!(
        imports.preview_file(vec![b'x'; 2 * 1024 * 1024 + 1]),
        Err(AppError::InvalidInput)
    ));
    let _ = std::fs::remove_file(&path);
}
