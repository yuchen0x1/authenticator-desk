use authenticator_desk_lib::{
    accounts::{AccountInput, AccountService},
    security::run_security_audit,
    vault::{SecretString, VaultService},
};
use std::sync::Arc;

#[test]
fn audit_accepts_standard_totp_and_reports_bounded_plaintext_and_missing_backups() {
    let dir = std::env::temp_dir().join(format!("authdesk-security-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("vault.sqlite3");
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("audit-test"), &path)
        .unwrap();
    vault
        .unlock_vault(SecretString::from("audit-test"))
        .unwrap();
    let accounts = AccountService::new(vault.clone());
    let mut input = AccountInput::default();
    input.issuer = "GitHub".into();
    input.account_name = "alice".into();
    input.secret = Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into());
    accounts.create_account(input).unwrap();
    std::fs::write(dir.join("fixture.txt"), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
    std::fs::create_dir(dir.join("unrelated")).unwrap();
    std::fs::write(
        dir.join("unrelated").join("ignored.txt"),
        "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
    )
    .unwrap();
    let report = run_security_audit(&vault, &dir, None).unwrap();
    assert!(report.integrity.ok);
    assert_eq!(report.invalid_config_count, 0);
    assert_eq!(report.duplicate_count, 0);
    assert_eq!(report.plaintext_file_findings, vec!["fixture.txt"]);
    assert_eq!(report.backup_count, 0);
    assert_eq!(report.last_backup_at, None);
    assert_eq!(
        report.permission_findings,
        "not_checked_windows_acl_query_unavailable"
    );
    assert_eq!(report.time_status, "system_clock_only_not_calibrated");
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("GEZDG"));
    vault.lock_vault().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
