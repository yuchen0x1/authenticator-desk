mod crypto;
mod kdf;
mod session;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags, Transaction, params};
use uuid::Uuid;
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

pub(crate) type Invalidator = Arc<dyn Fn() + Send + Sync>;

/// An OS-held lease on a stable sidecar, not on SQLite's data/lock bytes.
/// Never unlink the sidecar: replacing it could let two owners lock different
/// files. A leftover sidecar is harmless; ownership is only the open file lock.
struct VaultLease {
    path: PathBuf,
    _file: fs::File,
}

fn acquire_vault_lease(
    path: &Path,
    current: Option<&Arc<VaultLease>>,
) -> AppResult<Arc<VaultLease>> {
    // Resolve symlink/relative spellings before choosing the sidecar, using a
    // canonical parent for creation. All later DB access uses this same path.
    let path = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::canonicalize(parent)
                .map_err(|_| AppError::Storage)?
                .join(path.file_name().ok_or(AppError::InvalidInput)?)
        }
        Err(_) => return Err(AppError::Storage),
    };
    if let Some(current) = current.filter(|lease| lease.path == path) {
        return Ok(Arc::clone(current));
    }
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(".lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(PathBuf::from(sidecar))
        .map_err(|_| AppError::Storage)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => AppError::Conflict,
        fs::TryLockError::Error(_) => AppError::Storage,
    })?;
    Ok(Arc::new(VaultLease { path, _file: file }))
}

struct VaultState {
    path: Option<PathBuf>,
    // Retained after successful create/unlock, even while locked or idle-expired,
    // until path reconfiguration or service drop. In-flight unlocks share the
    // lease so reconfiguration cannot release it before key verification ends.
    lease: Option<Arc<VaultLease>>,
    session: Option<SessionHandle>,
    generation: u64,
    revision: u64,
    idle: session::IdleState,
    invalidators: Vec<Invalidator>,
}

fn notify_invalidators(state: &VaultState) {
    for invalidator in &state.invalidators {
        invalidator();
    }
}

/// Invalidate every generation-bound capability (sessions, import previews,
/// backup previews) and announce the new epoch. Callers hold the state lock;
/// callbacks must only touch their own state and never re-enter the vault.
fn invalidate_generation(state: &mut VaultState) {
    state.generation = state.generation.saturating_add(1);
    state.revision = state.revision.saturating_add(1);
    notify_invalidators(state);
}

fn invalidate_revision(state: &mut VaultState) {
    state.revision = state.revision.saturating_add(1);
    notify_invalidators(state);
}

/// Drop an idle-expired session before any key use. Returns whether a live
/// session was expired by this call.
fn expire_if_needed(state: &mut VaultState) -> AppResult<bool> {
    if !state.idle.expired() {
        return Ok(false);
    }
    let Some(session) = state.session.take() else {
        return Ok(false);
    };
    session.invalidate()?;
    invalidate_generation(state);
    Ok(true)
}

/// Lazily reserves the canonical vault path on successful creation or unlock.
/// Ownership lasts until service drop or a locked `configure_path`, including
/// across explicit/idle locks. Other services/processes receive `Conflict`;
/// construction, status, and read-only integrity checks do not require a lease.
pub struct VaultService {
    state: Mutex<VaultState>,
}

impl Default for VaultService {
    fn default() -> Self {
        Self::new(None::<PathBuf>)
    }
}

impl VaultService {
    /// Keeps session validity, key use, and DB access under one lock so a lock or
    /// password change cannot race a write. Polling reads never reset idle time.
    pub(crate) fn with_database<T>(
        &self,
        mutation: bool,
        operation: impl FnOnce(&mut Connection, &[u8; 32], u64, u64) -> AppResult<T>,
    ) -> AppResult<T> {
        self.with_database_activity(mutation, mutation, operation)
    }

    /// Persists maintenance metadata without treating a timer as user activity.
    pub(crate) fn with_background_database<T>(
        &self,
        operation: impl FnOnce(&mut Connection, &[u8; 32], u64, u64) -> AppResult<T>,
    ) -> AppResult<T> {
        self.with_database_activity(true, false, operation)
    }

    fn with_database_activity<T>(
        &self,
        mutation: bool,
        user_activity: bool,
        operation: impl FnOnce(&mut Connection, &[u8; 32], u64, u64) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if expire_if_needed(&mut state)? {
            return Err(AppError::VaultLocked);
        }
        let key = state
            .session
            .as_ref()
            .filter(|s| !s.is_invalidated())
            .ok_or(AppError::VaultLocked)?
            .key_copy()?;
        let path = state.path.as_ref().ok_or(AppError::NotFound)?;
        let mut conn =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|_| AppError::Storage)?;
        initialize_connection(&mut conn)?;
        let result = operation(&mut conn, &key, state.generation, state.revision)?;
        if mutation {
            invalidate_revision(&mut state);
        }
        if user_activity {
            state.idle.touch();
        }
        Ok(result)
    }

    pub fn new(path: Option<impl Into<PathBuf>>) -> Self {
        Self {
            state: Mutex::new(VaultState {
                path: path.map(Into::into),
                lease: None,
                session: None,
                generation: 0,
                revision: 0,
                idle: session::IdleState::new(Duration::from_secs(15 * 60)),
                invalidators: Vec::new(),
            }),
        }
    }

    /// Register a non-reentrant callback invoked whenever the vault
    /// generation or revision changes. Import previews use this to drop
    /// retained plaintext immediately on lock, password change, or mutation.
    pub(crate) fn register_invalidator(&self, invalidator: Invalidator) -> AppResult<()> {
        self.state
            .lock()
            .map_err(|_| AppError::Storage)?
            .invalidators
            .push(invalidator);
        Ok(())
    }

    pub fn configure_path(&self, path: impl Into<PathBuf>) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        expire_if_needed(&mut state)?;
        if state.session.is_some() {
            return Err(AppError::Conflict);
        }
        state.path = Some(path.into());
        state.lease = None;
        invalidate_revision(&mut state);
        Ok(())
    }

    pub fn create_vault(&self, password: SecretString, path: impl AsRef<Path>) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        expire_if_needed(&mut state)?;
        if state.session.is_some() {
            return Err(AppError::Conflict);
        }
        let lease = create_vault_inner(password, path.as_ref(), |_| Ok(()))?;
        state.path = Some(lease.path.clone());
        state.lease = Some(lease);
        invalidate_revision(&mut state);
        Ok(())
    }

    /// A competing service/process returns `Conflict` without reading a key or
    /// modifying the vault. Status and read-only integrity checks need no lease.
    pub fn unlock_vault(&self, password: SecretString) -> AppResult<SessionHandle> {
        let (lease, revision) = {
            let state = self.state.lock().map_err(|_| AppError::Storage)?;
            let path = state.path.as_ref().ok_or(AppError::NotFound)?;
            if !path.exists() {
                return Err(AppError::NotFound);
            }
            (
                acquire_vault_lease(path, state.lease.as_ref())?,
                state.revision,
            )
        };
        // Keep a newly acquired lease local until authentication succeeds. A
        // failed password or revision check must not reserve the vault forever.
        let session = unlock_vault_at(password, &lease.path)?;
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if state.revision != revision {
            return Err(AppError::Conflict);
        }
        if let Some(previous) = state.session.as_ref() {
            previous.invalidate()?;
        }
        state.path = Some(lease.path.clone());
        state.lease = Some(lease);
        state.session = None;
        invalidate_generation(&mut state);
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
        invalidate_generation(&mut state);
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
        if expire_if_needed(&mut state)? {
            return Err(AppError::VaultLocked);
        }
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
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        expire_if_needed(&mut state)?;
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
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        expire_if_needed(&mut state)?;
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
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if expire_if_needed(&mut state)? {
            return Err(AppError::VaultLocked);
        }
        state.idle.set_timeout(timeout);
        Ok(())
    }

    pub fn check_idle(&self) -> AppResult<bool> {
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        expire_if_needed(&mut state)
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
        let mut state = self.state.lock().map_err(|_| AppError::Storage)?;
        if expire_if_needed(&mut state)? {
            return Err(AppError::VaultLocked);
        }
        let path = {
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
        if let Some(session) = state.session.take() {
            session.invalidate()?;
        }
        invalidate_generation(&mut state);
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

/// Removes an in-progress vault file and its SQLite sidecars on every failure
/// path. The final path is only linked after initialization succeeds.
struct TempVaultFile {
    path: PathBuf,
}

impl Drop for TempVaultFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar: OsString = self.path.as_os_str().to_owned();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

fn reserve_temp_vault(path: &Path) -> AppResult<(PathBuf, TempVaultFile)> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or(AppError::InvalidInput)?;
    for _ in 0..8 {
        let mut candidate = OsString::from(".");
        candidate.push(name);
        candidate.push(format!(".tmp-{}", Uuid::new_v4()));
        let candidate = parent.join(candidate);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                drop(file);
                return Ok((candidate.clone(), TempVaultFile { path: candidate }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AppError::Storage),
        }
    }
    Err(AppError::Storage)
}

/// Standalone creation holds a lease through atomic installation only. Use the
/// service method to retain ownership between creation and the first unlock.
pub fn create_vault(password: SecretString, path: &Path) -> AppResult<()> {
    create_vault_inner(password, path, |_| Ok(())).map(|_| ())
}

fn create_vault_inner(
    password: SecretString,
    path: &Path,
    before_metadata: impl FnOnce(&Path) -> AppResult<()>,
) -> AppResult<Arc<VaultLease>> {
    if password.is_empty() {
        return Err(AppError::InvalidInput);
    }
    if path.as_os_str().is_empty() {
        return Err(AppError::InvalidInput);
    }
    if path.file_name().is_none() {
        return Err(AppError::InvalidInput);
    }
    if path.exists() {
        return Err(AppError::Conflict);
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|_| AppError::Storage)?;
    }
    let lease = acquire_vault_lease(path, None)?;
    let path = &lease.path;
    let header = KdfHeader::new_default()?;
    let key = header.derive_key(&password)?;
    let verification = encrypt_record(&key, CHECK_AAD, CHECK_VALUE)?;
    let encoded = header.encode()?;
    let (temporary, guard) = reserve_temp_vault(path)?;
    {
        let mut conn = Connection::open_with_flags(&temporary, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| AppError::Storage)?;
        initialize_connection(&mut conn)?;
        migrations::migrate(&mut conn)?;
        before_metadata(&temporary)?;
        conn.execute("INSERT INTO vault_meta (id, format_version, kdf_header, verification, created_at) VALUES (1, 1, ?1, ?2, ?3)", params![encoded, verification, Utc::now().to_rfc3339()]).map_err(|_| AppError::Storage)?;
    }
    // A hard link is the portable no-overwrite atomic install: exactly one
    // concurrent creator wins, and a pre-existing destination is untouched.
    fs::hard_link(&temporary, path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            AppError::Conflict
        } else {
            AppError::Storage
        }
    })?;
    drop(guard);
    Ok(lease)
}

fn unlock_vault_at(password: SecretString, path: &Path) -> AppResult<Zeroizing<[u8; 32]>> {
    if !path.exists() {
        return Err(AppError::NotFound);
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| AppError::Storage)?;
    initialize_connection_readonly(&conn)?;
    let schema_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    if schema_version != migrations::CURRENT_SCHEMA_VERSION {
        return Err(AppError::Storage);
    }
    let (format_version, header_bytes, verification): (u32, Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT format_version, kdf_header, verification FROM vault_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| AppError::AuthenticationFailed)?;
    if format_version != 1 {
        return Err(AppError::Storage);
    }
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
    if !path.exists() {
        return Err(AppError::NotFound);
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| AppError::Storage)?;
    initialize_connection_readonly(&conn)?;
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    if version != migrations::CURRENT_SCHEMA_VERSION {
        return Err(AppError::Storage);
    }
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
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| AppError::Storage)?;
    initialize_connection(&mut conn)?;
    let (format_version, header_bytes, verification): (u32, Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT format_version, kdf_header, verification FROM vault_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| AppError::AuthenticationFailed)?;
    let schema_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    if schema_version != migrations::CURRENT_SCHEMA_VERSION || format_version != 1 {
        return Err(AppError::Storage);
    }
    let old_header =
        KdfHeader::decode(&header_bytes).map_err(|_| AppError::AuthenticationFailed)?;
    let old_key = old_header
        .derive_key(&current)
        .map_err(|_| AppError::AuthenticationFailed)?;
    let marker = decrypt_record(&old_key, CHECK_AAD, &verification)
        .map_err(|_| AppError::AuthenticationFailed)?;
    if marker.as_slice() != CHECK_VALUE {
        return Err(AppError::AuthenticationFailed);
    }
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
    fn activity_cannot_revive_an_expired_session() {
        let path = temp_path("late-activity");
        create_vault(SecretString::from("idle-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        let session = service
            .unlock_vault(SecretString::from("idle-password"))
            .unwrap();
        service.set_idle_timeout(Duration::from_millis(1)).unwrap();
        std::thread::sleep(Duration::from_millis(4));
        assert!(matches!(service.touch(), Err(crate::AppError::VaultLocked)));
        assert!(session.is_invalidated());
        assert!(service.is_locked());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn background_mutations_preserve_idle_deadline_and_invalidate_previews() {
        let path = temp_path("background-idle");
        create_vault(SecretString::from("idle-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        service
            .unlock_vault(SecretString::from("idle-password"))
            .unwrap();
        let revision = service.revision().unwrap();
        // Leave enough headroom for slower hosted runners to open and initialize
        // SQLite before asserting that background work does not extend the deadline.
        service.set_idle_timeout(Duration::from_secs(1)).unwrap();
        service
            .with_background_database(|conn, _, _, _| {
                conn.execute("UPDATE vault_meta SET created_at=created_at", [])
                    .map_err(|_| crate::AppError::Storage)?;
                Ok(())
            })
            .unwrap();
        assert_eq!(service.revision().unwrap(), revision + 1);
        std::thread::sleep(Duration::from_millis(1100));
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
    fn password_change_cannot_revive_an_idle_expired_session() {
        let path = temp_path("privileged-idle");
        create_vault(SecretString::from("idle-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        let session = service
            .unlock_vault(SecretString::from("idle-password"))
            .unwrap();
        let original = fs::read(&path).unwrap();
        service.set_idle_timeout(Duration::from_millis(1)).unwrap();
        std::thread::sleep(Duration::from_millis(4));
        assert!(matches!(
            service.change_password(
                SecretString::from("idle-password"),
                SecretString::from("next")
            ),
            Err(crate::AppError::VaultLocked)
        ));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(session.is_invalidated());
        assert!(service.is_locked());
        assert!(
            service
                .unlock_vault(SecretString::from("idle-password"))
                .is_ok()
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn record_helpers_and_timeout_changes_cannot_revive_expired_sessions() {
        let path = temp_path("expired-record-helpers");
        create_vault(SecretString::from("idle-password"), &path).unwrap();
        let service = VaultService::new(Some(path.clone()));
        for operation in 0..3 {
            service.set_idle_timeout(Duration::from_secs(900)).unwrap();
            let session = service
                .unlock_vault(SecretString::from("idle-password"))
                .unwrap();
            let ciphertext = service
                .encrypt_blob("accounts", "id", "secret_blob", b"secret")
                .unwrap();
            service.set_idle_timeout(Duration::from_millis(1)).unwrap();
            std::thread::sleep(Duration::from_millis(4));
            let result = match operation {
                0 => service
                    .encrypt_blob("accounts", "id", "secret_blob", b"secret")
                    .map(|_| ()),
                1 => service
                    .decrypt_blob("accounts", "id", "secret_blob", &ciphertext)
                    .map(|_| ()),
                _ => service.set_idle_timeout(Duration::ZERO),
            };
            assert!(matches!(result, Err(crate::AppError::VaultLocked)));
            assert!(session.is_invalidated());
            assert!(service.is_locked());
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn os_lease_releases_after_last_owner_and_leaves_a_reusable_sidecar() {
        let path = temp_path("file-lease");
        let lease = super::acquire_vault_lease(&path, None).unwrap();
        assert!(!path.exists());
        let shared = super::acquire_vault_lease(&path, Some(&lease)).unwrap();
        assert!(std::sync::Arc::ptr_eq(&lease, &shared));
        assert!(matches!(
            super::acquire_vault_lease(&path, None),
            Err(crate::AppError::Conflict)
        ));
        drop(lease);
        assert!(matches!(
            super::acquire_vault_lease(&path, None),
            Err(crate::AppError::Conflict)
        ));
        drop(shared);
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(".lock");
        let sidecar = std::path::PathBuf::from(sidecar);
        assert!(sidecar.exists());
        drop(super::acquire_vault_lease(&path, None).unwrap());
        fs::remove_file(sidecar).unwrap();
    }

    #[test]
    fn failed_initialization_leaves_no_final_or_temporary_vault_file() {
        let path = temp_path("failed-init");
        assert!(matches!(
            super::create_vault_inner(SecretString::from("failed-password"), &path, |_| {
                assert!(matches!(
                    super::acquire_vault_lease(&path, None),
                    Err(crate::AppError::Conflict)
                ));
                Err(crate::AppError::Storage)
            }),
            Err(crate::AppError::Storage)
        ));
        assert!(!path.exists());
        let parent = path.parent().unwrap();
        let prefix = format!(".{}.tmp-", path.file_name().unwrap().to_string_lossy());
        assert!(
            fs::read_dir(parent)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().starts_with(&prefix))
        );
        create_vault(SecretString::from("retry-password"), &path).unwrap();
        assert!(super::unlock_vault_at(SecretString::from("retry-password"), &path).is_ok());
        fs::remove_file(path).unwrap();
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
