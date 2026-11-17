mod crypto;
mod kdf;
mod session;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use zeroize::Zeroizing;

use crate::storage::{initialize_connection, migrations};
use crate::{AppError, AppResult};

pub use crypto::{decrypt_record, encrypt_record};
pub use kdf::{KdfHeader, SecretString};
pub use session::SessionHandle;

const CHECK_AAD: &[u8] = b"authenticator-desk:vault-check";
const CHECK_VALUE: &[u8] = b"AUTHDESK-VAULT-CHECK-V1";

pub(crate) fn record_aad(table: &str, id: &str, column: &str) -> Vec<u8> {
    format!("authenticator-desk:{table}:{id}:{column}").into_bytes()
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct VaultStatus {
    pub exists: bool,
    pub locked: bool,
    pub generation: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct IntegrityReport {
    pub ok: bool,
    pub user_version: u32,
}

struct VaultState {
    path: Option<PathBuf>,
    session: Option<SessionHandle>,
    generation: u64,
    revision: u64,
    idle: session::IdleState,
}

pub struct VaultService {
    state: Mutex<VaultState>,
}

impl Default for VaultService {
    fn default() -> Self {
        Self::new(None::<PathBuf>)
    }
}

impl VaultService {
    pub fn new(path: Option<impl Into<PathBuf>>) -> Self {
        Self {
            state: Mutex::new(VaultState {
                path: path.map(Into::into),
                session: None,
                generation: 0,
                revision: 0,
                idle: session::IdleState::new(Duration::from_secs(15 * 60)),
            }),
        }
    }

    pub fn configure_path(&self, path: impl Into<PathBuf>) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if state.session.is_some() {
            return Err(AppError::Conflict);
        }
        state.path = Some(path.into());
        Ok(())
    }

    pub fn create_vault(&self, password: SecretString, path: impl AsRef<Path>) -> AppResult<()> {
        create_vault(password, path.as_ref())?;
        self.configure_path(path.as_ref().to_path_buf())
    }

    pub fn unlock_vault(&self, password: SecretString) -> AppResult<SessionHandle> {
        let path = self
            .state
            .lock()
            .map_err(|_| AppError::Storage)?
            .path
            .clone()
            .ok_or(AppError::NotFound)?;
        let session = unlock_vault_at(password, &path)?;
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if let Some(previous) = state.session.as_ref() {
            previous.invalidate()?;
        }
        state.session = None;
        state.generation = state.generation.saturating_add(1);
        state.revision = state.revision.saturating_add(1);
        let session = SessionHandle::new(session, state.generation);
        state.idle.touch();
        state.session = Some(session.clone());
        Ok(session)
    }

    pub fn lock_vault(&self) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if let Some(session) = state.session.take() {
            session.invalidate()?;
        }
        state.generation = state.generation.saturating_add(1);
        state.revision = state.revision.saturating_add(1);
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.state
            .lock()
            .map(|state| {
                state
                    .session
                    .as_ref()
                    .map(|s| s.is_invalidated())
                    .unwrap_or(true)
            })
            .unwrap_or(true)
    }

    pub fn status(&self) -> VaultStatus {
        self.state
            .lock()
            .map(|state| VaultStatus {
                exists: state.path.as_ref().map(|p| p.exists()).unwrap_or(false),
                locked: state
                    .session
                    .as_ref()
                    .map(|s| s.is_invalidated())
                    .unwrap_or(true),
                generation: state.generation,
                revision: state.revision,
            })
            .unwrap_or(VaultStatus {
                exists: false,
                locked: true,
                generation: 0,
                revision: 0,
            })
    }

    pub fn touch(&self) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if state
            .session
            .as_ref()
            .map(|session| session.is_invalidated())
            .unwrap_or(true)
        {
            return Err(AppError::VaultLocked);
        }
        state.idle.touch();
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn encrypt_blob(
        &self,
        table: &str,
        id: &str,
        column: &str,
        plaintext: &[u8],
    ) -> AppResult<Vec<u8>> {
        let state = self.state.lock().map_err(|_| AppError::Storage)?;
        let session = state
            .session
            .as_ref()
            .filter(|session| !session.is_invalidated())
            .ok_or(AppError::VaultLocked)?;
        let key = session.key_copy()?;
        let aad = record_aad(table, id, column);
        encrypt_record(&key, &aad, plaintext)
    }

    #[allow(dead_code)]
    pub(crate) fn decrypt_blob(
        &self,
        table: &str,
        id: &str,
        column: &str,
        ciphertext: &[u8],
    ) -> AppResult<Zeroizing<Vec<u8>>> {
        let state = self.state.lock().map_err(|_| AppError::Storage)?;
        let session = state
            .session
            .as_ref()
            .filter(|session| !session.is_invalidated())
            .ok_or(AppError::VaultLocked)?;
        let key = session.key_copy()?;
        let aad = record_aad(table, id, column);
        decrypt_record(&key, &aad, ciphertext)
    }

    pub fn set_idle_timeout(&self, timeout: Duration) -> AppResult<()> {
        self.state
            .lock()
            .map_err(|_| AppError::Storage)?
            .idle
            .set_timeout(timeout);
        Ok(())
    }

    pub fn check_idle(&self) -> AppResult<bool> {
        let expired = self
            .state
            .lock()
            .map_err(|_| AppError::Storage)?
            .idle
            .expired();
        if expired && !self.is_locked() {
            self.lock_vault()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn integrity_check(&self) -> AppResult<IntegrityReport> {
        let path = self
            .state
            .lock()
            .map_err(|_| AppError::Storage)?
            .path
            .clone()
            .ok_or(AppError::NotFound)?;
        integrity_check_at(&path)
    }

    pub fn change_password(&self, current: SecretString, next: SecretString) -> AppResult<()> {
        if next.is_empty() {
            return Err(AppError::InvalidInput);
        }
        let path = {
            let state = self.state.lock().map_err(|_| AppError::Storage)?;
            if state
                .session
                .as_ref()
                .map(|session| session.is_invalidated())
                .unwrap_or(true)
            {
                return Err(AppError::VaultLocked);
            }
            state.path.clone().ok_or(AppError::NotFound)?
        };
        let new_key = change_password_at(&path, current, next)?;
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if let Some(session) = state.session.take() {
            session.invalidate()?;
        }
        state.generation = state.generation.saturating_add(1);
        state.revision = state.revision.saturating_add(1);
        let session = SessionHandle::new(new_key, state.generation);
        state.session = Some(session);
        state.idle.touch();
        Ok(())
    }

    pub fn session_generation(&self) -> AppResult<u64> {
        Ok(self.state.lock().map_err(|_| AppError::Storage)?.generation)
    }

    pub fn revision(&self) -> AppResult<u64> {
        Ok(self.state.lock().map_err(|_| AppError::Storage)?.revision)
    }
}

pub fn create_vault(password: SecretString, path: &Path) -> AppResult<()> {
    if password.is_empty() {
        return Err(AppError::InvalidInput);
    }
    if path.as_os_str().is_empty() {
        return Err(AppError::InvalidInput);
    }
    if path.exists() {
        let existing = Connection::open(path).map_err(|_| AppError::Storage)?;
        let has_meta: Option<String> = existing
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'vault_meta'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| AppError::Storage)?;
        if has_meta.is_some() {
            let present: Option<i64> = existing
                .query_row("SELECT 1 FROM vault_meta WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|_| AppError::Storage)?;
            if present.is_some() {
                return Err(AppError::Conflict);
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| AppError::Storage)?;
    }
    let mut conn = Connection::open(path).map_err(|_| AppError::Storage)?;
    initialize_connection(&mut conn)?;
    migrations::migrate(&mut conn)?;
    let header = KdfHeader::new_default()?;
    let key = header.derive_key(&password)?;
    let verification = encrypt_record(&key, CHECK_AAD, CHECK_VALUE)?;
    let encoded = header.encode()?;
    conn.execute("INSERT INTO vault_meta (id, format_version, kdf_header, verification, created_at) VALUES (1, 1, ?1, ?2, ?3)", params![encoded, verification, Utc::now().to_rfc3339()]).map_err(|_| AppError::Storage)?;
    Ok(())
}

fn unlock_vault_at(password: SecretString, path: &Path) -> AppResult<Zeroizing<[u8; 32]>> {
    let conn = Connection::open(path).map_err(|_| AppError::Storage)?;
    initialize_connection_readonly(&conn)?;
    let (header_bytes, verification): (Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT kdf_header, verification FROM vault_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::AuthenticationFailed)?;
    let header = KdfHeader::decode(&header_bytes).map_err(|_| AppError::AuthenticationFailed)?;
    let key = header
        .derive_key(&password)
        .map_err(|_| AppError::AuthenticationFailed)?;
    let marker = decrypt_record(&key, CHECK_AAD, &verification)
        .map_err(|_| AppError::AuthenticationFailed)?;
    if marker.as_slice() != CHECK_VALUE {
        return Err(AppError::AuthenticationFailed);
    }
    Ok(key)
}

pub fn integrity_check_at(path: &Path) -> AppResult<IntegrityReport> {
    let conn = Connection::open(path).map_err(|_| AppError::Storage)?;
    initialize_connection_readonly(&conn)?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    let user_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    Ok(IntegrityReport {
        ok: result == "ok",
        user_version,
    })
}

fn change_password_at(
    path: &Path,
    current: SecretString,
    next: SecretString,
) -> AppResult<Zeroizing<[u8; 32]>> {
    let mut conn = Connection::open(path).map_err(|_| AppError::Storage)?;
    initialize_connection(&mut conn)?;
    let (header_bytes, verification): (Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT kdf_header, verification FROM vault_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::AuthenticationFailed)?;
    let old_header =
        KdfHeader::decode(&header_bytes).map_err(|_| AppError::AuthenticationFailed)?;
    let old_key = old_header
        .derive_key(&current)
        .map_err(|_| AppError::AuthenticationFailed)?;
    decrypt_record(&old_key, CHECK_AAD, &verification)
        .map_err(|_| AppError::AuthenticationFailed)?;
    let new_header = KdfHeader::new_default()?;
    let new_key = new_header.derive_key(&next)?;
    let new_verification = encrypt_record(&new_key, CHECK_AAD, CHECK_VALUE)?;
    let tx = conn.transaction().map_err(|_| AppError::Storage)?;
    reencrypt_blob_rows(&tx, "accounts", "id", "secret_blob", &old_key, &new_key)?;
    reencrypt_blob_rows(&tx, "accounts", "id", "details_blob", &old_key, &new_key)?;
    reencrypt_blob_rows(&tx, "categories", "id", "details_blob", &old_key, &new_key)?;
    reencrypt_blob_rows(&tx, "settings", "key", "value_blob", &old_key, &new_key)?;
    reencrypt_blob_rows(
        &tx,
        "audit_events",
        "id",
        "details_blob",
        &old_key,
        &new_key,
    )?;
    tx.execute(
        "UPDATE vault_meta SET kdf_header = ?1, verification = ?2, format_version = 1 WHERE id = 1",
        params![new_header.encode()?, new_verification],
    )
    .map_err(|_| AppError::Storage)?;
    tx.commit().map_err(|_| AppError::Storage)?;
    Ok(new_key)
}

fn reencrypt_blob_rows(
    tx: &Transaction<'_>,
    table: &str,
    id_column: &str,
    blob_column: &str,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> AppResult<()> {
    let query = format!("SELECT {id_column}, {blob_column} FROM {table}");
    let mut statement = tx.prepare(&query).map_err(|_| AppError::Storage)?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row
                .get(0)
                .or_else(|_| row.get::<_, i64>(0).map(|value| value.to_string()))?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })
        .map_err(|_| AppError::Storage)?;
    let mut replacements = Vec::new();
    for row in rows {
        let (id, blob) = row.map_err(|_| AppError::Storage)?;
        let aad = record_aad(table, &id, blob_column);
        let plaintext = decrypt_record(old_key, &aad, &blob)?;
        let replacement = encrypt_record(new_key, &aad, &plaintext)?;
        replacements.push((id, replacement));
    }
    drop(statement);
    let update = format!("UPDATE {table} SET {blob_column} = ?1 WHERE {id_column} = ?2");
    for (id, replacement) in replacements {
        tx.execute(&update, params![replacement, id])
            .map_err(|_| AppError::Storage)?;
    }
    Ok(())
}

fn initialize_connection_readonly(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA secure_delete = ON;",
    )
    .map_err(|_| AppError::Storage)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rusqlite::{Connection, params};

    use super::{SecretString, VaultService, create_vault, integrity_check_at};
    use super::{decrypt_record, encrypt_record, record_aad};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("authdesk-{name}-{suffix}.sqlite3"))
    }

    #[test]
    fn first_run_unlock_lock_and_wrong_password() {
        let path = temp_path("lifecycle");
        create_vault(SecretString::from("correct"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        assert!(service.is_locked());
        assert!(service.unlock_vault(SecretString::from("wrong")).is_err());
        let handle = service.unlock_vault(SecretString::from("correct")).unwrap();
        assert!(!service.is_locked());
        service.lock_vault().unwrap();
        assert!(service.is_locked());
        assert!(handle.is_invalidated());
        service.unlock_vault(SecretString::from("correct")).unwrap();
        assert!(!service.is_locked());
        assert!(integrity_check_at(&path).unwrap().ok);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn password_change_preserves_wrong_password_failure() {
        let path = temp_path("password");
        create_vault(SecretString::from("old"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        assert!(matches!(
            service.change_password(SecretString::from("old"), SecretString::from("new")),
            Err(crate::AppError::VaultLocked)
        ));
        assert!(
            service
                .change_password(SecretString::from("bad"), SecretString::from("new"))
                .is_err()
        );
        assert!(service.unlock_vault(SecretString::from("old")).is_ok());
        service
            .change_password(SecretString::from("old"), SecretString::from("new"))
            .unwrap();
        assert!(service.unlock_vault(SecretString::from("old")).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn password_change_reencrypts_sensitive_rows_transactionally() {
        let path = temp_path("reencrypt");
        create_vault(SecretString::from("old"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        let old_session = service.unlock_vault(SecretString::from("old")).unwrap();
        let old_key = old_session.key_copy().unwrap();
        let secret_aad = record_aad("accounts", "account-1", "secret_blob");
        let details_aad = record_aad("accounts", "account-1", "details_blob");
        let secret_blob = encrypt_record(&old_key, &secret_aad, b"super-secret-token").unwrap();
        let details_blob =
            encrypt_record(&old_key, &details_aad, b"private account details").unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO accounts (id, secret_blob, details_blob, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params!["account-1", secret_blob, details_blob, "2026-11-17T00:00:00Z"],
        ).unwrap();
        drop(conn);

        service
            .change_password(SecretString::from("old"), SecretString::from("new"))
            .unwrap();
        let new_session = service.unlock_vault(SecretString::from("new")).unwrap();
        let new_key = new_session.key_copy().unwrap();
        let conn = Connection::open(&path).unwrap();
        let stored: Vec<u8> = conn
            .query_row(
                "SELECT secret_blob FROM accounts WHERE id = 'account-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(stored, secret_blob);
        assert!(decrypt_record(&old_key, &secret_aad, &stored).is_err());
        assert_eq!(
            decrypt_record(&new_key, &secret_aad, &stored)
                .unwrap()
                .as_slice(),
            b"super-secret-token"
        );
        let bytes = fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(b"super-secret-token".len())
                .any(|window| window == b"super-secret-token")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn idle_expiry_locks_without_refresh_touch() {
        let path = temp_path("idle");
        create_vault(SecretString::from("idle-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        service
            .unlock_vault(SecretString::from("idle-password"))
            .unwrap();
        service.set_idle_timeout(Duration::from_millis(1)).unwrap();
        std::thread::sleep(Duration::from_millis(4));
        assert!(service.check_idle().unwrap());
        assert!(service.is_locked());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn internal_record_helpers_encrypt_and_decrypt_only_when_unlocked() {
        let path = temp_path("record-helpers");
        create_vault(SecretString::from("record-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));

        assert!(matches!(
            service.decrypt_blob("accounts", "account-1", "secret_blob", b""),
            Err(crate::AppError::VaultLocked)
        ));

        service
            .unlock_vault(SecretString::from("record-password"))
            .unwrap();
        let encrypted = service
            .encrypt_blob("accounts", "account-1", "secret_blob", b"internal-secret")
            .unwrap();
        let decrypted = service
            .decrypt_blob("accounts", "account-1", "secret_blob", &encrypted)
            .unwrap();
        assert_eq!(decrypted.as_slice(), b"internal-secret");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn duplicate_unlock_invalidates_the_previous_session() {
        let path = temp_path("duplicate-unlock");
        create_vault(SecretString::from("unlock-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        let first = service
            .unlock_vault(SecretString::from("unlock-password"))
            .unwrap();
        let second = service
            .unlock_vault(SecretString::from("unlock-password"))
            .unwrap();

        assert!(first.is_invalidated());
        assert!(!second.is_invalidated());
        let _ = fs::remove_file(path);
    }
}
