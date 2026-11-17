use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use authenticator_desk_lib::vault::{SecretString, VaultService, create_vault, integrity_check_at};

fn temp_path(label: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("authdesk-integration-{label}-{suffix}.sqlite3"))
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
