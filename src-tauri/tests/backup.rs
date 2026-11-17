use authenticator_desk_lib::{
    AppError,
    accounts::{AccountInput, AccountQuery, AccountService},
    backup::{BackupService, RestoreMode, RestoreSelection},
    vault::{SecretString, VaultService},
};
use std::{path::PathBuf, sync::Arc};

fn fixture() -> (PathBuf, Arc<VaultService>, AccountService, BackupService) {
    let dir = std::env::temp_dir().join(format!("authdesk-backup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("vault.sqlite3");
    let vault = Arc::new(VaultService::new(Some(path.clone())));
    vault
        .create_vault(SecretString::from("vault-test"), &path)
        .unwrap();
    vault
        .unlock_vault(SecretString::from("vault-test"))
        .unwrap();
    let accounts = AccountService::new(vault.clone());
    let backups = BackupService::new(vault.clone());
    (dir, vault, accounts, backups)
}
fn input(name: &str) -> AccountInput {
    let mut value = AccountInput::default();
    value.issuer = "GitHub".into();
    value.account_name = name.into();
    value.secret = Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into());
    value.notes = "private backup note".into();
    value
}
fn selection(indices: Vec<usize>, confirm: bool) -> RestoreSelection {
    RestoreSelection {
        account_indices: indices,
        confirm_replace: confirm,
    }
}

#[test]
fn encrypted_round_trip_merge_duplicates_and_safe_preview() {
    let (dir, vault, accounts, backup) = fixture();
    let category = accounts.save_category(None, "Work", "#775cff").unwrap();
    let mut alice = input("alice@example.com");
    alice.category_id = Some(category.id.clone());
    alice.tags = vec!["work".into()];
    let alice = accounts.create_account(alice).unwrap();
    accounts.create_account(input("bob@example.com")).unwrap();
    let path = dir.join("roundtrip.authdesk");
    backup
        .create_backup(SecretString::from("backup-pass"), &path)
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    for value in [
        "GEZDGNBVGY3TQOJQ",
        "private backup note",
        "alice@example.com",
    ] {
        assert!(!bytes.windows(value.len()).any(|w| w == value.as_bytes()));
    }
    accounts.delete_accounts(&[alice.id]).unwrap();
    let preview = backup
        .preview_restore(SecretString::from("backup-pass"), &path, RestoreMode::Merge)
        .unwrap();
    assert_eq!(preview.items[0].issuer, "GitHub");
    assert!(
        preview
            .items
            .iter()
            .any(|item| item.account_name == "alice@example.com")
    );
    assert_eq!(
        preview
            .items
            .iter()
            .filter(|item| item.status == "duplicate")
            .count(),
        1
    );
    let encoded = serde_json::to_string(&preview).unwrap();
    assert!(!encoded.contains("GEZDG"));
    assert!(!encoded.contains("private backup note"));
    let indices = preview
        .items
        .iter()
        .filter(|item| item.status == "new")
        .map(|item| item.index)
        .collect();
    let result = backup
        .apply_restore(
            uuid::Uuid::parse_str(&preview.preview_id).unwrap(),
            selection(indices, false),
        )
        .unwrap();
    assert_eq!(result.restored_accounts, 1);
    assert_eq!(
        accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .len(),
        2
    );
    let restored = accounts
        .list_accounts(AccountQuery::default())
        .unwrap()
        .into_iter()
        .find(|account| account.account_name == "alice@example.com")
        .unwrap();
    assert_eq!(restored.category_id.as_deref(), Some(category.id.as_str()));
    assert_eq!(restored.tags, vec!["work"]);
    assert_eq!(
        accounts.account_details(&restored.id).unwrap().notes,
        "private backup note"
    );
    vault.lock_vault().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rejects_wrong_password_tampering_truncation_and_unsupported_version() {
    let (dir, vault, accounts, backup) = fixture();
    accounts.create_account(input("alice")).unwrap();
    let path = dir.join("original.authdesk");
    backup
        .create_backup(SecretString::from("backup-pass"), &path)
        .unwrap();
    assert!(matches!(
        backup.preview_restore(SecretString::from("wrong"), &path, RestoreMode::Merge),
        Err(AppError::AuthenticationFailed)
    ));
    let original = std::fs::read(&path).unwrap();
    let mut tampered = original.clone();
    *tampered.last_mut().unwrap() ^= 1;
    let mut version = original.clone();
    version[8..10].copy_from_slice(&2u16.to_be_bytes());
    for (name, bytes) in [
        ("tampered", tampered),
        ("truncated", original[..original.len() - 1].to_vec()),
        ("version", version),
    ] {
        let path = dir.join(format!("{name}.authdesk"));
        std::fs::write(&path, bytes).unwrap();
        assert!(
            backup
                .preview_restore(SecretString::from("backup-pass"), &path, RestoreMode::Merge)
                .is_err()
        );
    }
    assert_eq!(
        accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .len(),
        1
    );
    vault.lock_vault().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn replace_requires_confirmation_and_keeps_all_backup_rows() {
    let (dir, vault, accounts, backup) = fixture();
    accounts.create_account(input("alice")).unwrap();
    let mut settings = accounts.settings().unwrap();
    settings.locale = "ja".into();
    accounts.save_settings(settings).unwrap();
    let path = dir.join("replace.authdesk");
    backup
        .create_backup(SecretString::from("backup-pass"), &path)
        .unwrap();
    accounts.create_account(input("extra")).unwrap();
    let mut settings = accounts.settings().unwrap();
    settings.locale = "en".into();
    accounts.save_settings(settings).unwrap();
    let preview = backup
        .preview_restore(
            SecretString::from("backup-pass"),
            &path,
            RestoreMode::Replace,
        )
        .unwrap();
    assert_eq!(preview.items[0].status, "new");
    let id = uuid::Uuid::parse_str(&preview.preview_id).unwrap();
    assert!(matches!(
        backup.apply_restore(id, selection(vec![0], false)),
        Err(AppError::InvalidInput)
    ));
    assert_eq!(
        accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        backup
            .apply_restore(id, selection(vec![0], true))
            .unwrap()
            .restored_accounts,
        1
    );
    let restored = accounts.list_accounts(AccountQuery::default()).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].account_name, "alice");
    assert_eq!(accounts.settings().unwrap().locale, "ja");
    let stale = backup
        .preview_restore(SecretString::from("backup-pass"), &path, RestoreMode::Merge)
        .unwrap();
    accounts.create_account(input("after-preview")).unwrap();
    assert!(matches!(
        backup.apply_restore(
            uuid::Uuid::parse_str(&stale.preview_id).unwrap(),
            selection(vec![], false)
        ),
        Err(AppError::Conflict)
    ));
    vault.lock_vault().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn automatic_backup_preserves_utf8_password_and_never_overwrites() {
    let (dir, vault, accounts, backup) = fixture();
    accounts.create_account(input("alice")).unwrap();
    let target = dir.join("automatic");
    backup
        .configure_auto_backup(true, target.clone(), SecretString::from("备份🔐密码"), 1, 1)
        .unwrap();
    let summary = backup.run_auto_backup(2_000_000_000).unwrap().unwrap();
    backup
        .preview_restore(
            SecretString::from("备份🔐密码"),
            &PathBuf::from(&summary.path),
            RestoreMode::Merge,
        )
        .unwrap();
    let before = std::fs::read(&summary.path).unwrap();
    assert!(
        backup
            .create_backup(SecretString::from("other"), &PathBuf::from(&summary.path))
            .is_err()
    );
    assert_eq!(std::fs::read(&summary.path).unwrap(), before);
    std::fs::write(target.join("unowned.authdesk"), b"keep").unwrap();
    backup.run_auto_backup(2_000_004_000).unwrap().unwrap();
    assert!(target.join("unowned.authdesk").exists());
    assert_eq!(backup.list_backups(&target).unwrap().len(), 1);
    let previous = backup.list_backups(&target).unwrap()[0].name.clone();
    backup
        .configure_auto_backup(
            true,
            target.join("unowned.authdesk"),
            SecretString::from("备份🔐密码"),
            1,
            1,
        )
        .unwrap();
    assert!(backup.run_auto_backup(2_000_008_000).is_err());
    assert!(target.join(previous).exists());
    backup
        .configure_auto_backup(false, PathBuf::new(), SecretString::from(""), 0, 0)
        .unwrap();
    assert!(!backup.auto_backup_status().unwrap().enabled);
    assert!(backup.run_auto_backup(2_000_008_000).unwrap().is_none());
    vault.lock_vault().unwrap();
    assert!(backup.auto_backup_status().unwrap().paused_locked);
    let _ = std::fs::remove_dir_all(dir);
}
