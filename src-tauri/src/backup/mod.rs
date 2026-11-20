mod format;
mod restore;

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    AppError, AppResult,
    accounts::{
        CategoryView,
        service::{
            audit, encrypt_json, ensure_unique, load_accounts, load_categories, load_settings,
            save_account,
        },
    },
    vault::{SecretString, VaultService, decrypt_record, encrypt_record, record_aad},
};
use format::{
    BackupAccount, BackupMetadata, BackupPayload, BackupSummary, decode_file, encode,
    write_new_atomic,
};

pub use format::BackupSummary as Summary;
pub use restore::{RestoreMode, RestorePreview, RestoreResult, RestoreSelection};

const MAX_PREVIEW_SECS: u64 = 300;
const MAX_BACKUPS: usize = 10_000;
const VAULT_ID_SETTING: &str = "backup_vault_id";

struct PendingRestore {
    id: Uuid,
    expires: Instant,
    generation: u64,
    revision: u64,
    mode: RestoreMode,
    payload: BackupPayload,
    statuses: Vec<String>,
}
impl PendingRestore {
    /// Drops the decrypted payload without using it. Callers must already hold
    /// the pending-state lock so no concurrent apply can observe a half state.
    fn discard(slot: &mut Option<Self>) {
        if let Some(pending) = slot.take() {
            drop(pending);
        }
    }
}

fn pending_is_invalid(
    pending: &PendingRestore,
    now: Instant,
    locked: bool,
    generation: u64,
    revision: u64,
) -> bool {
    pending.expires <= now
        || locked
        || generation != pending.generation
        || revision != pending.revision
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedAutoBackup {
    directory: PathBuf,
    name: String,
    created_at: i64,
    digest: Vec<u8>,
    vault_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutoBackupConfig {
    enabled: bool,
    directory: PathBuf,
    interval_hours: u32,
    retention: usize,
    password: String,
    last_run: i64,
    /// Stable identity of the vault that owns these automatic backups.
    /// Absent on configurations written by older builds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vault_id: Option<String>,
    /// Exact file hashes are recorded inside the encrypted vault settings.
    /// Old files without a record are never eligible for retention deletion.
    #[serde(default)]
    owned_backups: Vec<OwnedAutoBackup>,
}
impl Drop for AutoBackupConfig {
    fn drop(&mut self) {
        self.password.zeroize();
        if let Some(id) = self.vault_id.as_mut() {
            id.zeroize();
        }
        for backup in &mut self.owned_backups {
            backup.name.zeroize();
            backup.vault_id.zeroize();
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct BackupFileView {
    pub name: String,
    pub bytes: u64,
    pub modified_at: String,
    pub automatic: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct AutoBackupStatus {
    pub enabled: bool,
    pub paused_locked: bool,
    pub interval_hours: u32,
    pub retention: usize,
    pub last_run: Option<i64>,
}

pub struct BackupService {
    vault: Arc<VaultService>,
    pending: Arc<Mutex<Option<PendingRestore>>>,
    preview_epoch: Arc<AtomicU64>,
}
impl BackupService {
    pub fn new(vault: Arc<VaultService>) -> Self {
        let pending: Arc<Mutex<Option<PendingRestore>>> = Arc::new(Mutex::new(None));
        let preview_epoch = Arc::new(AtomicU64::new(0));
        let weak_pending = Arc::downgrade(&pending);
        let weak_epoch = Arc::downgrade(&preview_epoch);
        let _ = vault.register_invalidator(Arc::new(move || {
            if let Some(epoch) = weak_epoch.upgrade() {
                epoch.fetch_add(1, Ordering::AcqRel);
            }
            if let Some(pending) = weak_pending.upgrade() {
                let mut slot = pending.lock().unwrap_or_else(|error| error.into_inner());
                PendingRestore::discard(&mut slot);
            }
        }));
        Self {
            vault,
            pending,
            preview_epoch,
        }
    }
    pub fn clear(&self) -> AppResult<()> {
        self.preview_epoch.fetch_add(1, Ordering::AcqRel);
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        PendingRestore::discard(&mut pending);
        Ok(())
    }

    pub fn create_backup(
        &self,
        password: SecretString,
        destination: &Path,
    ) -> AppResult<BackupSummary> {
        self.vault.with_database(false, |conn, key, _, _| {
            let payload = snapshot(conn, key)?;
            format::check_counts(&payload)?;
            let bytes = encode(&payload, &password)?;
            // Keep the originating session valid until the encrypted backup is
            // atomically published. A concurrent lock cannot return before it.
            write_new_atomic(destination, &bytes)?;
            Ok(BackupSummary {
                path: destination.to_string_lossy().into_owned(),
                account_count: payload.accounts.len(),
                category_count: payload.categories.len(),
                bytes: bytes.len() as u64,
                created_at: payload.metadata.created_at.clone(),
            })
        })
    }

    pub fn preview_restore(
        &self,
        password: SecretString,
        source: &Path,
        mode: RestoreMode,
    ) -> AppResult<RestorePreview> {
        self.clear()?;
        let epoch = self.preview_epoch.load(Ordering::Acquire);
        let (origin_generation, origin_revision) = self
            .vault
            .with_database(false, |_, _, generation, revision| {
                Ok((generation, revision))
            })?;
        let decoded = decode_file(source, &password)?;
        validate_payload(&decoded.payload)?;
        let (generation, revision, statuses, existing_count) =
            self.vault.with_database(false, |conn, key, g, r| {
                if g != origin_generation || r != origin_revision {
                    return Err(AppError::Conflict);
                }
                // Replace is also the recovery path for damaged encrypted rows.
                // Only Merge needs to authenticate the outgoing accounts.
                let existing = if mode == RestoreMode::Merge {
                    load_accounts(conn, key)?
                } else {
                    Vec::new()
                };
                let existing_count = if mode == RestoreMode::Replace {
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
                        .map_err(|_| AppError::Storage)?;
                    usize::try_from(count).map_err(|_| AppError::Storage)?
                } else {
                    existing.len()
                };
                let mut existing_keys = std::collections::HashSet::new();
                if mode == RestoreMode::Merge {
                    for (account, secret) in &existing {
                        existing_keys.insert((
                            normalized_label(&account.issuer),
                            normalized_label(&account.account_name),
                            secret.as_slice(),
                        ));
                    }
                }
                let mut seen = std::collections::HashSet::new();
                let mut statuses = Vec::with_capacity(decoded.payload.accounts.len());
                for incoming in &decoded.payload.accounts {
                    let issuer = normalized_label(&incoming.data.issuer);
                    let account_name = normalized_label(&incoming.data.account_name);
                    let key = (issuer, account_name, incoming.secret.as_slice());
                    let duplicate_in_backup = !seen.insert((key.0.clone(), key.1.clone(), key.2));
                    let duplicate_existing =
                        mode == RestoreMode::Merge && existing_keys.contains(&key);
                    statuses.push(if duplicate_in_backup || duplicate_existing {
                        "duplicate".to_owned()
                    } else {
                        "new".to_owned()
                    });
                }
                Ok((g, r, statuses, existing_count))
            })?;
        let safe_account_names: Vec<_> = decoded
            .payload
            .accounts
            .iter()
            .map(|entry| (entry.data.issuer.clone(), entry.data.account_name.clone()))
            .collect();
        let preview = RestorePreview::from_decoded(
            &decoded.summary,
            mode,
            &safe_account_names,
            statuses.clone(),
            existing_count,
        );
        let pending = PendingRestore {
            id: Uuid::parse_str(&preview.preview_id).map_err(|_| AppError::Crypto)?,
            expires: Instant::now() + Duration::from_secs(MAX_PREVIEW_SECS),
            generation,
            revision,
            mode,
            payload: decoded.payload,
            statuses,
        };
        self.vault.with_database(false, |_, _, g, r| {
            let mut slot = self.pending.lock().map_err(|_| AppError::Storage)?;
            if pending_is_invalid(&pending, Instant::now(), false, g, r)
                || self.preview_epoch.load(Ordering::Acquire) != epoch
            {
                return Err(AppError::Conflict);
            }
            *slot = Some(pending);
            Ok(preview)
        })
    }

    pub fn apply_restore(
        &self,
        preview_id: Uuid,
        selection: RestoreSelection,
    ) -> AppResult<RestoreResult> {
        let (result, restored_timeout) = self.vault.with_database(true, |conn, key, generation, revision| {
            // Vault before pending: invalidation callbacks run under the vault
            // lock and never re-enter it. Successful validation consumes the
            // preview before starting the transaction.
            let (p, selected) = {
                let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
                if pending.as_ref().is_some_and(|p| {
                    pending_is_invalid(p, Instant::now(), false, generation, revision)
                }) {
                    PendingRestore::discard(&mut pending);
                }
                let p = pending.as_ref().filter(|p| p.id == preview_id).ok_or(AppError::Conflict)?;
                if p.mode == RestoreMode::Replace && !selection.confirm_replace {
                    return Err(AppError::InvalidInput);
                }
                if selection.account_indices.len() > p.payload.accounts.len() {
                    return Err(AppError::InvalidInput);
                }
                let mut selected = std::collections::HashSet::new();
                for n in selection.account_indices {
                    if p.statuses.get(n).is_none_or(|status| status != "new") || !selected.insert(n) {
                        return Err(AppError::InvalidInput);
                    }
                }
                (pending.take().ok_or(AppError::Conflict)?, selected)
            };
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            if p.mode == RestoreMode::Replace {
                tx.execute_batch("DELETE FROM accounts; DELETE FROM categories;")
                    .map_err(|_| AppError::Storage)?;
                tx.execute("DELETE FROM settings WHERE key != ?1", [VAULT_ID_SETTING])
                    .map_err(|_| AppError::Storage)?;
            }
            let mut category_map = std::collections::HashMap::new();
            for category in &p.payload.categories {
                let existing_categories = load_categories(&tx, key)?;
                if p.mode == RestoreMode::Merge {
                    if let Some(existing) = existing_categories.iter().find(|c| normalized_label(&c.name) == normalized_label(&category.name)) {
                        category_map.insert(category.id.clone(), existing.id.clone());
                        continue;
                    }
                }
                let id = if p.mode == RestoreMode::Merge && existing_categories.iter().any(|c| c.id == category.id) {
                    Uuid::new_v4().to_string()
                } else {
                    category.id.clone()
                };
                let restored = CategoryView {
                    id: id.clone(), name: category.name.clone(), color: category.color.clone(),
                };
                let encrypted = encrypt_json(key, "categories", &id, "details_blob", &restored)?;
                tx.execute(
                    "INSERT INTO categories(id,details_blob,created_at,updated_at) VALUES(?1,?2,?3,?3)",
                    params![id, encrypted, Utc::now().to_rfc3339()],
                ).map_err(|_| AppError::Storage)?;
                category_map.insert(category.id.clone(), id);
            }
            let mut restored_count = 0;
            for (index, entry) in p.payload.accounts.iter().enumerate() {
                if !selected.contains(&index) { continue; }
                let mut data = entry.data.clone();
                if let Some(category) = data.category_id.as_ref() {
                    data.category_id = category_map.get(category).cloned();
                }
                if p.mode == RestoreMode::Merge {
                    let collision = tx.query_row("SELECT 1 FROM accounts WHERE id=?1", [&data.id], |r| r.get::<_, i64>(0))
                        .optional().map_err(|_| AppError::Storage)?.is_some();
                    if collision { data.id = Uuid::new_v4().to_string(); }
                    let max = load_accounts(&tx, key)?
                        .iter()
                        .map(|(account, _)| account.sort_order)
                        .max()
                        .unwrap_or(-1);
                    data.sort_order = max.saturating_add(1);
                }
                data.updated_at = Utc::now().to_rfc3339();
                data.config(&entry.secret)?.validate()?;
                ensure_unique(&tx, key, &data, &entry.secret, None)?;
                save_account(&tx, key, &data, &entry.secret)?;
                restored_count += 1;
            }
            if p.mode == RestoreMode::Replace {
                let settings = &p.payload.settings;
                settings.validate()?;
                let plaintext = Zeroizing::new(serde_json::to_vec(settings).map_err(|_| AppError::InvalidInput)?);
                let encoded = encrypt_record(key, &record_aad("settings", "preferences", "value_blob"), &plaintext)?;
                tx.execute("INSERT INTO settings(key,value_blob,updated_at) VALUES('preferences',?1,?2)",
                    params![encoded, Utc::now().to_rfc3339()]).map_err(|_| AppError::Storage)?;
            }
            audit(&tx, key, "backup_restored", None)?;
            if p.expires <= Instant::now() {
                return Err(AppError::Conflict);
            }
            tx.commit().map_err(|_| AppError::Storage)?;
            let timeout = (p.mode == RestoreMode::Replace).then_some(p.payload.settings.idle_timeout_secs);
            Ok((RestoreResult { restored_accounts: restored_count, restored_categories: p.payload.categories.len(), mode: p.mode }, timeout))
        })?;
        if let Some(seconds) = restored_timeout {
            // A replace commits the restored preferences. Apply the restored
            // idle policy to the live session before returning; if that cannot
            // be guaranteed, relock so the vault is never left open under the
            // pre-restore policy.
            if self
                .vault
                .set_idle_timeout(Duration::from_secs(seconds))
                .is_err()
            {
                let _ = self.vault.lock_vault();
                return Err(AppError::Storage);
            }
        }
        Ok(result)
    }

    /// Cancels a pending preview and drops its decrypted payload.
    ///
    /// Idempotent: an unknown, already applied, or expired preview is simply
    /// discarded.
    pub fn cancel_preview(&self, id: Uuid) -> AppResult<()> {
        self.preview_epoch.fetch_add(1, Ordering::AcqRel);
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        if pending.as_ref().is_some_and(|p| p.id == id) {
            PendingRestore::discard(&mut pending);
        }
        Ok(())
    }

    /// Drops expired plaintext on a maintenance tick. Vault revision and lock
    /// changes independently invalidate it synchronously via the callback.
    pub fn cleanup_expired(&self) -> AppResult<()> {
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        if pending
            .as_ref()
            .is_some_and(|p| p.expires <= Instant::now())
        {
            PendingRestore::discard(&mut pending);
        }
        Ok(())
    }

    pub fn configure_auto_backup(
        &self,
        enabled: bool,
        directory: PathBuf,
        password: SecretString,
        interval_hours: u32,
        retention: usize,
    ) -> AppResult<AutoBackupStatus> {
        if !enabled {
            return self.vault.with_database(true, |conn, key, _, _| {
                let Some(mut config) = load_auto_config(conn, key)? else {
                    return Ok(AutoBackupStatus {
                        enabled: false,
                        paused_locked: false,
                        interval_hours: 24,
                        retention: 7,
                        last_run: None,
                    });
                };
                config.enabled = false;
                write_auto_config(conn, key, &config)?;
                Ok(AutoBackupStatus {
                    enabled: false,
                    paused_locked: false,
                    interval_hours: config.interval_hours,
                    retention: config.retention,
                    last_run: (config.last_run > 0).then_some(config.last_run),
                })
            });
        }
        if interval_hours == 0
            || interval_hours > 8760
            || retention == 0
            || retention > 100
            || directory.as_os_str().is_empty()
            || password.is_empty()
        {
            return Err(AppError::InvalidInput);
        }
        let password =
            String::from_utf8(password.as_bytes().to_vec()).map_err(|_| AppError::InvalidInput)?;
        self.vault.with_database(true, |conn, key, _, _| {
            let vault_id = get_or_create_vault_identity(conn, key)?;
            let mut existing = load_auto_config(conn, key)?;
            if existing
                .as_ref()
                .and_then(|config| config.vault_id.as_deref())
                .is_some_and(|id| id != vault_id)
            {
                return Err(AppError::Conflict);
            }
            let mut owned_backups = existing
                .as_mut()
                .map(|config| std::mem::take(&mut config.owned_backups))
                .unwrap_or_default();
            // Changing folders abandons retention in the old folder rather
            // than carrying deletion authority across a configuration change.
            owned_backups
                .retain(|backup| backup.directory == directory && backup.vault_id == vault_id);
            let config = AutoBackupConfig {
                enabled: true,
                directory: directory.clone(),
                interval_hours,
                retention,
                password,
                last_run: 0,
                vault_id: Some(vault_id),
                owned_backups,
            };
            write_auto_config(conn, key, &config)?;
            Ok(AutoBackupStatus {
                enabled: true,
                paused_locked: false,
                interval_hours,
                retention,
                last_run: None,
            })
        })
    }
    pub fn auto_backup_status(&self) -> AppResult<AutoBackupStatus> {
        match self.read_auto_config() {
            Ok(Some(c)) => Ok(AutoBackupStatus {
                enabled: c.enabled,
                paused_locked: false,
                interval_hours: c.interval_hours,
                retention: c.retention,
                last_run: (c.last_run > 0).then_some(c.last_run),
            }),
            Ok(None) => Ok(AutoBackupStatus {
                enabled: false,
                paused_locked: false,
                interval_hours: 24,
                retention: 7,
                last_run: None,
            }),
            Err(AppError::VaultLocked) => Ok(AutoBackupStatus {
                enabled: false,
                paused_locked: true,
                interval_hours: 24,
                retention: 7,
                last_run: None,
            }),
            Err(e) => Err(e),
        }
    }
    pub fn configured_directory(&self) -> AppResult<Option<PathBuf>> {
        Ok(self
            .read_auto_config()?
            .map(|config| config.directory.clone()))
    }
    pub fn run_auto_backup(&self, now: i64) -> AppResult<Option<BackupSummary>> {
        let Some((config, expected_generation, expected_revision)) =
            self.read_auto_config_with_state()?
        else {
            return Ok(None);
        };
        if !config.enabled || !backup_due(&config, now) {
            return Ok(None);
        }
        if now < 0 {
            return Err(AppError::InvalidInput);
        }
        self.vault
            .with_background_database(|conn, key, generation, revision| {
                if generation != expected_generation || revision != expected_revision {
                    return Err(AppError::Conflict);
                }
                let Some(mut latest) = load_auto_config(conn, key)? else {
                    return Ok(None);
                };
                if !latest.enabled || !backup_due(&latest, now) {
                    return Ok(None);
                }
                if !same_auto_config(&config, &latest) {
                    return Err(AppError::Conflict);
                }
                if latest.owned_backups.len() >= MAX_BACKUPS {
                    return Err(AppError::InvalidInput);
                }
                let vault_id = get_or_create_vault_identity(conn, key)?;
                if latest
                    .vault_id
                    .as_deref()
                    .is_some_and(|configured_id| configured_id != vault_id)
                {
                    return Err(AppError::Conflict);
                }
                latest.vault_id = Some(vault_id.clone());

                let mut payload = snapshot(conn, key)?;
                payload.vault_id = Some(vault_id.clone());
                // Check count and byte bounds before publishing a file or invoking
                // retention, including for data modified since an older backup.
                format::check_counts(&payload)?;
                let password = SecretString::from(latest.password.clone());
                let bytes = encode(&payload, &password)?;
                let name = format!("authdesk-auto-{now}-{}.authdesk", Uuid::new_v4());
                let path = latest.directory.join(&name);
                write_new_atomic(&path, &bytes)?;
                let digest = Sha256::digest(&bytes).to_vec();
                latest.owned_backups.push(OwnedAutoBackup {
                    directory: latest.directory.clone(),
                    name: name.clone(),
                    created_at: now,
                    digest,
                    vault_id: vault_id.clone(),
                });
                latest.last_run = now;
                apply_retention(
                    &latest.directory,
                    latest.retention,
                    &vault_id,
                    &mut latest.owned_backups,
                )?;
                write_auto_config(conn, key, &latest)?;
                Ok(Some(BackupSummary {
                    path: path.to_string_lossy().into_owned(),
                    account_count: payload.accounts.len(),
                    category_count: payload.categories.len(),
                    bytes: bytes.len() as u64,
                    created_at: payload.metadata.created_at.clone(),
                }))
            })
    }
    fn read_auto_config(&self) -> AppResult<Option<AutoBackupConfig>> {
        self.vault
            .with_database(false, |conn, key, _, _| load_auto_config(conn, key))
    }
    fn read_auto_config_with_state(&self) -> AppResult<Option<(AutoBackupConfig, u64, u64)>> {
        self.vault
            .with_database(false, |conn, key, generation, revision| {
                Ok(load_auto_config(conn, key)?.map(|config| (config, generation, revision)))
            })
    }
    pub fn list_backups(&self, directory: &Path) -> AppResult<Vec<BackupFileView>> {
        let mut result = Vec::new();
        if !directory.exists() {
            return Ok(result);
        }
        for item in fs::read_dir(directory).map_err(|_| AppError::Storage)? {
            let item = item.map_err(|_| AppError::Storage)?;
            let path = item.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| AppError::Storage)?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || path.extension().and_then(|v| v.to_str()) != Some("authdesk")
            {
                continue;
            }
            let mut f = fs::File::open(&path).map_err(|_| AppError::Storage)?;
            let mut magic = [0u8; 8];
            if f.read_exact(&mut magic).is_err() || &magic != format::MAGIC {
                continue;
            }
            result.push(BackupFileView {
                name: item.file_name().to_string_lossy().into_owned(),
                bytes: meta.len(),
                modified_at: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_default(),
                automatic: is_owned_auto_backup_name(&item.file_name().to_string_lossy()),
            });
            if result.len() > MAX_BACKUPS {
                return Err(AppError::InvalidInput);
            }
        }
        result.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(result)
    }
}
fn load_auto_config(
    conn: &rusqlite::Connection,
    key: &[u8; 32],
) -> AppResult<Option<AutoBackupConfig>> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value_blob FROM settings WHERE key='auto_backup'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| AppError::Storage)?;
    blob.map(|bytes| {
        let plaintext = decrypt_record(
            key,
            &record_aad("settings", "auto_backup", "value_blob"),
            &bytes,
        )?;
        serde_json::from_slice(&plaintext).map_err(|_| AppError::InvalidInput)
    })
    .transpose()
}

fn get_or_create_vault_identity(conn: &rusqlite::Connection, key: &[u8; 32]) -> AppResult<String> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value_blob FROM settings WHERE key=?1",
            [VAULT_ID_SETTING],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::Storage)?;
    if let Some(blob) = blob {
        let plaintext = decrypt_record(
            key,
            &record_aad("settings", VAULT_ID_SETTING, "value_blob"),
            &blob,
        )?;
        let identity = String::from_utf8(plaintext.to_vec()).map_err(|_| AppError::InvalidInput)?;
        if Uuid::parse_str(&identity).is_err() {
            return Err(AppError::InvalidInput);
        }
        return Ok(identity);
    }

    let identity = Uuid::new_v4().to_string();
    let encoded = encrypt_record(
        key,
        &record_aad("settings", VAULT_ID_SETTING, "value_blob"),
        identity.as_bytes(),
    )?;
    conn.execute(
        "INSERT INTO settings(key,value_blob,updated_at) VALUES(?1,?2,?3)",
        params![VAULT_ID_SETTING, encoded, Utc::now().to_rfc3339()],
    )
    .map_err(|_| AppError::Storage)?;
    Ok(identity)
}

fn backup_due(config: &AutoBackupConfig, now: i64) -> bool {
    config.last_run <= 0
        || now.saturating_sub(config.last_run) >= i64::from(config.interval_hours) * 3600
}

fn same_auto_config(left: &AutoBackupConfig, right: &AutoBackupConfig) -> bool {
    left.enabled == right.enabled
        && left.directory == right.directory
        && left.interval_hours == right.interval_hours
        && left.retention == right.retention
        && left.password == right.password
        && left.last_run == right.last_run
        && left.vault_id == right.vault_id
        && owned_backups_equal(&left.owned_backups, &right.owned_backups)
}

fn owned_backups_equal(left: &[OwnedAutoBackup], right: &[OwnedAutoBackup]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.directory == b.directory
                && a.name == b.name
                && a.created_at == b.created_at
                && a.digest == b.digest
                && a.vault_id == b.vault_id
        })
}

fn apply_retention(
    directory: &Path,
    retention: usize,
    vault_id: &str,
    records: &mut Vec<OwnedAutoBackup>,
) -> AppResult<()> {
    if retention == 0 || records.len() > MAX_BACKUPS {
        return Err(AppError::InvalidInput);
    }

    let mut verified = Vec::new();
    let mut preserve = Vec::new();
    for record in std::mem::take(records) {
        if record.directory != directory
            || record.vault_id != vault_id
            || !is_owned_auto_backup_name(&record.name)
            || record.digest.len() != 32
        {
            preserve.push(record);
            continue;
        }
        let path = directory.join(&record.name);
        match file_digest_if_regular(&path) {
            Some(digest) if digest == record.digest => verified.push(record),
            None if !path.exists() => {
                // Nothing is deleted. Forgetting a missing record cannot turn
                // any extant foreign path into an owned file.
            }
            _ => preserve.push(record),
        }
    }
    verified.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.name.cmp(&left.name))
    });

    for (index, record) in verified.into_iter().enumerate() {
        if index < retention {
            preserve.push(record);
            continue;
        }
        let path = directory.join(&record.name);
        if !remove_verified_backup(&path, &record.digest) {
            preserve.push(record);
        }
    }
    *records = preserve;
    Ok(())
}

/// Verify and delete through the same handle; verifying then unlinking a path
/// would let a concurrent rename substitute an unowned file in between.
#[cfg(windows)]
fn remove_verified_backup(path: &Path, expected_digest: &[u8]) -> bool {
    use std::os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, RawHandle},
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const DELETE: u32 = 0x0001_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_DISPOSITION_INFO_CLASS: i32 = 4;

    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: u8,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            file: RawHandle,
            class: i32,
            info: *const FileDispositionInfo,
            size: u32,
        ) -> i32;
    }

    let Ok(file) = fs::OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return false;
    }
    if digest_reader(&file, metadata.len()).as_deref() != Some(expected_digest) {
        return false;
    }

    // SAFETY: the live file handle grants DELETE access and denies other
    // handles write/delete sharing. The repr(C) buffer matches the Win32
    // FILE_DISPOSITION_INFO layout and lives through the call.
    let info = FileDispositionInfo { delete_file: 1 };
    unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FILE_DISPOSITION_INFO_CLASS,
            &info,
            size_of::<FileDispositionInfo>() as u32,
        ) != 0
    }
}

#[cfg(not(windows))]
fn remove_verified_backup(_path: &Path, _expected_digest: &[u8]) -> bool {
    // No portable compare-and-unlink primitive: keep the file rather than
    // deleting an unauthenticated substitution at its former path.
    false
}

fn file_digest_if_regular(path: &Path) -> Option<Vec<u8>> {
    let before = fs::symlink_metadata(path).ok()?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.len() > format::MAX_BACKUP_BYTES
    {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || opened.len() != before.len() {
        return None;
    }
    let digest = digest_reader(&file, before.len())?;
    let after = fs::symlink_metadata(path).ok()?;
    if !after.is_file() || after.file_type().is_symlink() || after.len() != before.len() {
        return None;
    }
    Some(digest)
}

fn digest_reader(reader: impl Read, expected_len: u64) -> Option<Vec<u8>> {
    if expected_len > format::MAX_BACKUP_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    reader
        .take(format::MAX_BACKUP_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != expected_len {
        return None;
    }
    Some(Sha256::digest(&bytes).to_vec())
}

fn write_auto_config(
    conn: &rusqlite::Connection,
    key: &[u8; 32],
    config: &AutoBackupConfig,
) -> AppResult<()> {
    if config.owned_backups.len() > MAX_BACKUPS {
        return Err(AppError::InvalidInput);
    }
    let raw = Zeroizing::new(serde_json::to_vec(config).map_err(|_| AppError::InvalidInput)?);
    let blob = encrypt_record(
        key,
        &record_aad("settings", "auto_backup", "value_blob"),
        &raw,
    )?;
    conn.execute(
        "INSERT INTO settings(key,value_blob,updated_at) VALUES('auto_backup',?1,?2) ON CONFLICT(key) DO UPDATE SET value_blob=excluded.value_blob,updated_at=excluded.updated_at",
        params![blob, Utc::now().to_rfc3339()],
    ).map_err(|_| AppError::Storage)?;
    Ok(())
}
fn is_owned_auto_backup_name(name: &str) -> bool {
    name.starts_with("authdesk-auto-")
        && name.ends_with(".authdesk")
        && name.len() < 128
        && name[14..name.len() - 9]
            .split_once('-')
            .is_some_and(|(ts, id)| {
                ts.bytes().all(|b| b.is_ascii_digit()) && Uuid::parse_str(id).is_ok()
            })
}

/// Mirrors `accounts::service::ensure_unique` exactly: Unicode-aware Rust
/// lowercase expansion, with no trimming or ASCII-only comparison.
fn normalized_label(label: &str) -> String {
    label.to_lowercase()
}

fn snapshot(conn: &rusqlite::Connection, key: &[u8; 32]) -> AppResult<BackupPayload> {
    let accounts = load_accounts(conn, key)?
        .into_iter()
        .map(|(data, secret)| BackupAccount {
            data,
            secret: secret.to_vec(),
        })
        .collect();
    Ok(BackupPayload {
        accounts,
        categories: load_categories(conn, key)?,
        settings: load_settings(conn, key)?,
        metadata: BackupMetadata {
            application: "AUTHENTICATOR DESK".into(),
            created_at: Utc::now().to_rfc3339(),
        },
        vault_id: None,
    })
}
fn validate_payload(payload: &BackupPayload) -> AppResult<()> {
    format::check_counts(payload)?;
    payload.settings.validate()?;
    let mut ids = std::collections::HashSet::new();
    for category in &payload.categories {
        if category.id.is_empty()
            || category.name.trim().is_empty()
            || category.name.len() > format::MAX_CATEGORY_NAME_CHARS
            || category.color.len() != 7
            || !category.color.starts_with('#')
            || !category.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(category.id.clone())
        {
            return Err(AppError::InvalidInput);
        }
    }
    let catids: std::collections::HashSet<&str> =
        payload.categories.iter().map(|c| c.id.as_str()).collect();
    ids.clear();
    for entry in &payload.accounts {
        if entry.data.id.is_empty()
            || !ids.insert(entry.data.id.clone())
            || entry.secret.is_empty()
            || entry.secret.len() > 1024
            || entry.data.issuer.len() > format::MAX_ISSUER_CHARS
            || entry.data.account_name.trim().is_empty()
            || entry.data.account_name.len() > format::MAX_ACCOUNT_NAME_CHARS
            || entry.data.tags.len() > format::MAX_TAGS
            || entry
                .data
                .tags
                .iter()
                .any(|tag| tag.is_empty() || tag.len() > format::MAX_TAG_CHARS)
            || entry.data.notes.len() > format::MAX_NOTES_CHARS
            || !["totp", "hotp"].contains(&entry.data.kind.as_str())
            || !(6..=8).contains(&entry.data.digits)
            || !(1..=86400).contains(&entry.data.period_secs)
            || entry
                .data
                .category_id
                .as_deref()
                .is_some_and(|id| !catids.contains(id))
        {
            return Err(AppError::InvalidInput);
        }
        entry.data.config(&entry.secret)?.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BackupMetadata, BackupPayload, BackupService, PendingRestore, RestoreMode,
        pending_is_invalid,
    };
    use crate::{accounts::AppSettings, vault::VaultService};
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    fn pending(expires: Instant) -> PendingRestore {
        PendingRestore {
            id: Uuid::new_v4(),
            expires,
            generation: 4,
            revision: 9,
            mode: RestoreMode::Merge,
            payload: BackupPayload {
                accounts: Vec::new(),
                categories: Vec::new(),
                settings: AppSettings::default(),
                metadata: BackupMetadata {
                    application: "AUTHENTICATOR DESK".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                },
                vault_id: None,
            },
            statuses: Vec::new(),
        }
    }

    #[test]
    fn preview_invalidation_uses_expiry_generation_revision_and_lock_without_sleeping() {
        let now = Instant::now();
        let unexpired = pending(now + Duration::from_secs(60));
        assert!(!pending_is_invalid(&unexpired, now, false, 4, 9));
        assert!(pending_is_invalid(&unexpired, now, true, 4, 9));
        assert!(pending_is_invalid(&unexpired, now, false, 5, 9));
        assert!(pending_is_invalid(&unexpired, now, false, 4, 10));

        let expired = pending(now - Duration::from_secs(1));
        assert!(pending_is_invalid(&expired, now, false, 4, 9));
    }

    #[test]
    fn cleanup_drops_expired_plaintext_payload_deterministically() {
        let service = BackupService::new(Arc::new(VaultService::default()));
        let expired = pending(Instant::now() - Duration::from_secs(1));
        *service.pending.lock().unwrap() = Some(expired);
        service.cleanup_expired().unwrap();
        assert!(service.pending.lock().unwrap().is_none());
    }

    #[test]
    fn vault_callbacks_drop_pending_payload_synchronously_on_lock_and_revision() {
        let vault = Arc::new(VaultService::default());
        let service = BackupService::new(vault.clone());
        *service.pending.lock().unwrap() = Some(pending(Instant::now() + Duration::from_secs(60)));
        vault.lock_vault().unwrap();
        assert!(service.pending.lock().unwrap().is_none());

        *service.pending.lock().unwrap() = Some(pending(Instant::now() + Duration::from_secs(60)));
        vault.configure_path("unused-backup-test-path").unwrap();
        assert!(service.pending.lock().unwrap().is_none());
    }
}
