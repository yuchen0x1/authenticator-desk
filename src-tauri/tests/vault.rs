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
