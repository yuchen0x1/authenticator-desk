use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use authenticator_desk_lib::AppError;
use authenticator_desk_lib::accounts::{AccountInput, AccountQuery, AccountService, AppSettings};
use authenticator_desk_lib::vault::{SecretString, VaultService, create_vault, integrity_check_at};

fn temp_path(label: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("authdesk-integration-{label}-{suffix}.sqlite3"))
}

// New lease tests use isolated directories so persistent lock sidecars are
// cleaned up only after every owning service has been dropped.
struct TestVault(PathBuf);

impl TestVault {
    fn new(label: &str) -> Self {
        let directory = temp_path(label).with_extension("dir");
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self) -> PathBuf {
        self.0.join("vault.sqlite3")
    }
}

impl Drop for TestVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn test_account() -> AccountInput {
    let mut input = AccountInput::default();
    input.issuer = "lease regression".into();
    input.account_name = "test account".into();
    input.secret = Some("JBSWY3DPEHPK3PXP".into());
    input.notes = "preserved through password rotation".into();
    input
}

fn run_lease_subprocess(path: &Path, mode: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "vault_lease_subprocess_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("AUTHDESK_VAULT_LEASE_TEST_PATH", path)
        .env("AUTHDESK_VAULT_LEASE_TEST_MODE", mode)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "lease subprocess ({mode}) failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("lease-probe-completed"));
}

#[test]
fn vault_lifecycle_and_plaintext_absence() {
    let path = temp_path("lifecycle");
    let password = "correct horse battery staple";
    create_vault(SecretString::from(password), &path).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(password.len())
            .any(|window| window == password.as_bytes())
    );

    let service = VaultService::new(Some(path.clone()));
    assert!(service.unlock_vault(SecretString::from("wrong")).is_err());
    let session = service.unlock_vault(SecretString::from(password)).unwrap();
    assert!(!session.is_invalidated());
    service.lock_vault().unwrap();
    assert!(session.is_invalidated());
    assert!(service.is_locked());
    assert!(integrity_check_at(&path).unwrap().ok);
    let _ = fs::remove_file(path);
}

#[test]
fn password_change_requires_current_password() {
    let path = temp_path("password");
    create_vault(SecretString::from("old-password"), &path).unwrap();
    let service = VaultService::new(Some(path.clone()));
    assert!(
        service
            .change_password(
                SecretString::from("wrong"),
                SecretString::from("new-password")
            )
            .is_err()
    );
    assert!(
        service
            .unlock_vault(SecretString::from("old-password"))
            .is_ok()
    );
    service
        .change_password(
            SecretString::from("old-password"),
            SecretString::from("new-password"),
        )
        .unwrap();
    assert!(
        service
            .unlock_vault(SecretString::from("new-password"))
            .is_ok()
    );
    let _ = fs::remove_file(path);
}

#[test]
fn creation_preserves_an_existing_unrelated_database() {
    let path = temp_path("existing-file");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE important (value TEXT); INSERT INTO important VALUES ('keep me');",
    )
    .unwrap();
    drop(conn);
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        create_vault(SecretString::from("new-password"), &path),
        Err(authenticator_desk_lib::AppError::Conflict)
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_file(path).unwrap();
}

#[test]
fn concurrent_creation_installs_exactly_one_password() {
    let path = temp_path("parallel-create");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            create_vault(SecretString::from("first-password"), &path)
        });
        let second = scope.spawn(|| {
            barrier.wait();
            create_vault(SecretString::from("second-password"), &path)
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(results.0.is_ok(), results.1.is_ok());
    let loser = if results.0.is_ok() {
        results.1
    } else {
        results.0
    };
    assert!(matches!(
        loser,
        Err(authenticator_desk_lib::AppError::Conflict)
    ));
    let service = VaultService::new(Some(path.clone()));
    let first = service
        .unlock_vault(SecretString::from("first-password"))
        .is_ok();
    let second = service
        .unlock_vault(SecretString::from("second-password"))
        .is_ok();
    assert_ne!(first, second);
    service.lock_vault().unwrap();
    fs::remove_file(path).unwrap();
}

#[test]
fn read_operations_never_create_missing_vault_files() {
    let path = temp_path("missing-read");
    assert!(integrity_check_at(&path).is_err());
    assert!(!path.exists());
    let service = VaultService::new(Some(path.clone()));
    assert!(
        service
            .unlock_vault(SecretString::from("password"))
            .is_err()
    );
    assert!(!path.exists());
    assert!(!service.status().exists);
}

#[test]
fn unsupported_vault_versions_are_rejected_without_rewriting() {
    for pragma in [
        "PRAGMA user_version=999",
        "UPDATE vault_meta SET format_version=999",
    ] {
        let path = temp_path("future-format");
        create_vault(SecretString::from("format-password"), &path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(pragma).unwrap();
        drop(conn);
        let original = fs::read(&path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        assert!(
            service
                .unlock_vault(SecretString::from("format-password"))
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn independent_services_cannot_corrupt_password_rotation() {
    let temporary = TestVault::new("independent-writers");
    let path = temporary.path();
    create_vault(SecretString::from("old-password"), &path).unwrap();
    let owner = Arc::new(VaultService::new(Some(path.clone())));
    let contender = Arc::new(VaultService::new(Some(path.clone())));
    let owner_accounts = AccountService::new(owner.clone());
    let contender_accounts = AccountService::new(contender.clone());
    owner
        .unlock_vault(SecretString::from("old-password"))
        .unwrap();
    let account = owner_accounts.create_account(test_account()).unwrap();
    let preferences = AppSettings {
        locale: "en".into(),
        ..AppSettings::default()
    };
    owner_accounts.save_settings(preferences).unwrap();

    // B must never cache A's old key, even before A rotates the password.
    let conflict = contender.unlock_vault(SecretString::from("old-password"));
    assert!(matches!(conflict, Err(AppError::Conflict)));
    assert_eq!(
        serde_json::to_value(conflict.err().unwrap()).unwrap(),
        serde_json::json!({
            "code": "conflict",
            "message": "The requested change conflicts with existing data."
        })
    );
    owner
        .change_password(
            SecretString::from("old-password"),
            SecretString::from("new-password"),
        )
        .unwrap();
    let rotated = fs::read(&path).unwrap();
    for password in ["old-password", "new-password"] {
        assert!(matches!(
            contender.unlock_vault(SecretString::from(password)),
            Err(AppError::Conflict)
        ));
    }
    assert!(matches!(
        contender_accounts.save_settings(AppSettings::default()),
        Err(AppError::VaultLocked)
    ));
    assert!(matches!(
        contender_accounts.create_account(test_account()),
        Err(AppError::VaultLocked)
    ));
    assert_eq!(fs::read(&path).unwrap(), rotated);
    assert!(contender.status().exists);
    assert!(contender.status().locked);
    assert!(contender.integrity_check().unwrap().ok);

    drop(owner_accounts);
    drop(owner);
    let old_password_result = contender.unlock_vault(SecretString::from("old-password"));
    assert!(
        matches!(
            old_password_result.as_ref(),
            Err(AppError::AuthenticationFailed)
        ),
        "expected the rotated password to reject authentication, got error: {:?}",
        old_password_result.as_ref().err()
    );
    contender
        .unlock_vault(SecretString::from("new-password"))
        .unwrap();
    assert_eq!(contender_accounts.settings().unwrap().locale, "en");
    assert_eq!(
        contender_accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contender_accounts
            .account_details(&account.id)
            .unwrap()
            .notes,
        "preserved through password rotation"
    );
    contender_accounts
        .save_settings(AppSettings::default())
        .unwrap();
    assert_eq!(contender_accounts.settings().unwrap().locale, "zh-CN");
}

#[test]
fn service_lease_survives_lock_and_idle_expiry_until_reconfigured() {
    let temporary = TestVault::new("lease-lifetime");
    let path = temporary.path();
    let owner = VaultService::new(Some(path.clone()));
    let contender = VaultService::new(Some(path.clone()));
    owner
        .create_vault(SecretString::from("password"), &path)
        .unwrap();
    assert!(owner.status().locked);
    assert!(contender.status().exists);
    assert!(contender.integrity_check().unwrap().ok);
    assert!(matches!(
        contender.unlock_vault(SecretString::from("password")),
        Err(AppError::Conflict)
    ));
    owner.unlock_vault(SecretString::from("password")).unwrap();
    owner.lock_vault().unwrap();
    assert!(matches!(
        contender.unlock_vault(SecretString::from("password")),
        Err(AppError::Conflict)
    ));
    owner.unlock_vault(SecretString::from("password")).unwrap();
    owner.set_idle_timeout(Duration::from_millis(1)).unwrap();
    std::thread::sleep(Duration::from_millis(4));
    assert!(owner.check_idle().unwrap());
    assert!(matches!(
        contender.unlock_vault(SecretString::from("password")),
        Err(AppError::Conflict)
    ));

    let other_path = temporary.0.join("other.sqlite3");
    owner.configure_path(&other_path).unwrap();
    contender
        .unlock_vault(SecretString::from("password"))
        .unwrap();
    // The lease is per vault, not a process-wide singleton.
    owner
        .create_vault(SecretString::from("other-password"), &other_path)
        .unwrap();
    owner
        .unlock_vault(SecretString::from("other-password"))
        .unwrap();
    assert!(!contender.is_locked());
}

#[test]
fn failed_passwords_do_not_reserve_or_create_a_vault() {
    let temporary = TestVault::new("failed-password-lease");
    let path = temporary.path();
    let service = VaultService::new(Some(path.clone()));
    let initial = service.status();
    assert!(matches!(
        service.create_vault(SecretString::from(""), &path),
        Err(AppError::InvalidInput)
    ));
    assert!(matches!(
        create_vault(SecretString::from(""), &path),
        Err(AppError::InvalidInput)
    ));
    assert!(matches!(
        service.unlock_vault(SecretString::from("password")),
        Err(AppError::NotFound)
    ));
    assert_eq!(service.status(), initial);
    assert_eq!(fs::read_dir(&temporary.0).unwrap().count(), 0);

    create_vault(SecretString::from("password"), &path).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        service.unlock_vault(SecretString::from("wrong-password")),
        Err(AppError::AuthenticationFailed)
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    let next = VaultService::new(Some(path.clone()));
    let retained_session = next.unlock_vault(SecretString::from("password")).unwrap();
    drop(next);
    // A retained SessionHandle must not keep a dropped service's lease alive.
    service
        .unlock_vault(SecretString::from("password"))
        .unwrap();
    drop(retained_session);
}

#[test]
fn concurrent_service_creation_retains_only_the_winners_lease() {
    let temporary = TestVault::new("service-parallel-create");
    let path = temporary.path();
    let first = VaultService::new(Some(path.clone()));
    let second = VaultService::new(Some(path.clone()));
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            first.create_vault(SecretString::from("first-password"), &path)
        });
        let second = scope.spawn(|| {
            barrier.wait();
            second.create_vault(SecretString::from("second-password"), &path)
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(results.0.is_ok(), results.1.is_ok());
    let (winner, loser, password, error) = if results.0.is_ok() {
        (&first, &second, "first-password", results.1)
    } else {
        (&second, &first, "second-password", results.0)
    };
    assert!(matches!(error, Err(AppError::Conflict)));
    assert!(winner.status().locked);
    assert!(matches!(
        loser.unlock_vault(SecretString::from(password)),
        Err(AppError::Conflict)
    ));
    winner.unlock_vault(SecretString::from(password)).unwrap();
}

#[test]
fn password_rotation_lease_excludes_a_separate_process() {
    let temporary = TestVault::new("subprocess-lease");
    let path = temporary.path();
    let owner = Arc::new(VaultService::new(Some(path.clone())));
    owner
        .create_vault(SecretString::from("old-password"), &path)
        .unwrap();
    owner
        .unlock_vault(SecretString::from("old-password"))
        .unwrap();
    let accounts = AccountService::new(owner.clone());
    accounts.create_account(test_account()).unwrap();
    accounts
        .save_settings(AppSettings {
            locale: "en".into(),
            ..AppSettings::default()
        })
        .unwrap();
    let original = fs::read(&path).unwrap();
    run_lease_subprocess(&path, "conflict");
    assert_eq!(fs::read(&path).unwrap(), original);

    owner
        .change_password(
            SecretString::from("old-password"),
            SecretString::from("new-password"),
        )
        .unwrap();
    let rotated = fs::read(&path).unwrap();
    run_lease_subprocess(&path, "conflict");
    assert_eq!(fs::read(&path).unwrap(), rotated);
    drop(accounts);
    drop(owner);
    run_lease_subprocess(&path, "write");

    // The child's process exit releases its OS lease; its new-key data remains
    // decryptable to another independently constructed service.
    let reopened = Arc::new(VaultService::new(Some(path.clone())));
    reopened
        .unlock_vault(SecretString::from("new-password"))
        .unwrap();
    let accounts = AccountService::new(reopened);
    assert_eq!(accounts.settings().unwrap().locale, "ja");
    assert_eq!(
        accounts
            .list_accounts(AccountQuery::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
#[ignore = "invoked only by the parent lease regression with an isolated temporary vault"]
fn vault_lease_subprocess_helper() {
    let path = PathBuf::from(std::env::var_os("AUTHDESK_VAULT_LEASE_TEST_PATH").unwrap());
    let mode = std::env::var("AUTHDESK_VAULT_LEASE_TEST_MODE").unwrap();
    let vault = Arc::new(VaultService::new(Some(path)));
    let accounts = AccountService::new(vault.clone());
    assert!(vault.status().exists);
    assert!(vault.status().locked);
    assert!(vault.integrity_check().unwrap().ok);
    match mode.as_str() {
        "conflict" => {
            for password in ["old-password", "new-password"] {
                assert!(matches!(
                    vault.unlock_vault(SecretString::from(password)),
                    Err(AppError::Conflict)
                ));
            }
            assert!(matches!(
                accounts.save_settings(AppSettings::default()),
                Err(AppError::VaultLocked)
            ));
            assert!(matches!(
                accounts.create_account(test_account()),
                Err(AppError::VaultLocked)
            ));
        }
        "write" => {
            assert!(matches!(
                vault.unlock_vault(SecretString::from("old-password")),
                Err(AppError::AuthenticationFailed)
            ));
            vault
                .unlock_vault(SecretString::from("new-password"))
                .unwrap();
            assert_eq!(accounts.settings().unwrap().locale, "en");
            assert_eq!(
                accounts
                    .list_accounts(AccountQuery::default())
                    .unwrap()
                    .len(),
                1
            );
            accounts
                .save_settings(AppSettings {
                    locale: "ja".into(),
                    ..AppSettings::default()
                })
                .unwrap();
        }
        _ => panic!("unexpected lease subprocess mode"),
    }
    println!("lease-probe-completed");
}
