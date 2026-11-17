use authenticator_desk_lib::{
    accounts::{AccountQuery, AccountService},
    import::{ImportService, parse_otpauth},
    vault::{SecretString, VaultService},
};
use std::sync::Arc;
const URI: &str = "otpauth://totp/GitHub:alice%40example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=GitHub";
#[test]
fn parses_standard_parameters_and_rejects_ambiguous_or_malformed_uris() {
    let parsed = parse_otpauth(URI).unwrap();
    assert_eq!(parsed.issuer, "GitHub");
    assert_eq!(parsed.account_name, "alice@example.com");
    for algorithm in ["SHA1", "SHA256", "SHA512"] {
        let uri = format!("{URI}&algorithm={algorithm}&digits=8&period=60");
        let item = parse_otpauth(&uri).unwrap();
        assert_eq!(item.algorithm, algorithm);
        assert_eq!(item.period_secs, 60);
    }
    let hotp =
        parse_otpauth("otpauth://hotp/Example:counter?secret=MY&counter=18446744073709551615")
            .unwrap();
    assert_eq!(hotp.hotp_counter, u64::MAX.to_string());
    for uri in [
        "https://totp/X?secret=MY",
        "otpauth://totp/X",
        "otpauth://totp/X?secret=invalid",
        "otpauth://totp/X?secret=MY&secret=MY",
        "otpauth://totp/A:X?secret=MY&issuer=B",
        "otpauth://hotp/X?secret=MY",
        "otpauth://totp/X?secret=MY&period=0",
        "otpauth://totp/X?secret=MY&digits=5",
        "otpauth://totp/X?secret=MY&unknown=x",
        "otpauth://totp/X%ZZ?secret=MY",
    ] {
        assert!(
            parse_otpauth(uri).is_err(),
            "unexpectedly accepted malformed URI"
        );
    }
}
#[test]
fn preview_redacts_secrets_and_commit_is_session_bound_atomic_and_duplicate_aware() {
    let path =
        std::env::temp_dir().join(format!("authdesk-import-{}.sqlite3", uuid::Uuid::new_v4()));
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("import-test"), &path)
        .unwrap();
    vault
        .unlock_vault(SecretString::from("import-test"))
        .unwrap();
    let accounts = Arc::new(AccountService::new(vault.clone()));
    let imports = ImportService::new(accounts.clone());
    let preview = imports
        .preview_import(format!("{URI}\n{URI}\nnot-a-uri"))
        .unwrap();
    assert_eq!(
        preview
            .items
            .iter()
            .map(|i| i.status.as_str())
            .collect::<Vec<_>>(),
        vec!["new", "duplicate", "invalid"]
    );
    assert!(!serde_json::to_string(&preview).unwrap().contains("GEZDG"));
    assert!(imports.commit_import(&preview.preview_id, &[0, 2]).is_err());
    assert!(
        accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .is_empty()
    );
    let preview = imports.preview_import(URI.into()).unwrap();
    assert_eq!(imports.commit_import(&preview.preview_id, &[0]).unwrap(), 1);
    let preview = imports.preview_import(URI.into()).unwrap();
    assert_eq!(preview.items[0].status, "duplicate");
    let another = URI.replace("alice", "bob");
    let stale = imports.preview_import(another.clone()).unwrap();
    accounts
        .set_favorite(
            &accounts.list_accounts(AccountQuery::default()).unwrap()[0].id,
            true,
        )
        .unwrap();
    assert!(imports.commit_import(&stale.preview_id, &[0]).is_err());
    let locked = imports.preview_import(another).unwrap();
    vault.lock_vault().unwrap();
    vault
        .unlock_vault(SecretString::from("import-test"))
        .unwrap();
    assert!(imports.commit_import(&locked.preview_id, &[0]).is_err());
    let _ = std::fs::remove_file(path);
}
