mod format;
mod restore;

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
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

struct PendingRestore {
    id: Uuid,
    expires: Instant,
    generation: u64,
    revision: u64,
    mode: RestoreMode,
    payload: BackupPayload,
    statuses: Vec<String>,
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
}
impl Drop for AutoBackupConfig {
    fn drop(&mut self) {
        self.password.zeroize();
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
    pending: Mutex<Option<PendingRestore>>,
}
impl BackupService {
    pub fn new(vault: Arc<VaultService>) -> Self {
        Self {
            vault,
            pending: Mutex::new(None),
        }
    }
    pub fn clear(&self) -> AppResult<()> {
        *self.pending.lock().map_err(|_| AppError::Storage)? = None;
        Ok(())
    }

    pub fn create_backup(
        &self,
        password: SecretString,
        destination: &Path,
    ) -> AppResult<BackupSummary> {
        let payload = self
            .vault
            .with_database(false, |conn, key, _, _| snapshot(conn, key))?;
        let bytes = encode(&payload, &password)?;
        write_new_atomic(destination, &bytes)?;
        Ok(BackupSummary {
            path: destination.to_string_lossy().into_owned(),
            account_count: payload.accounts.len(),
            category_count: payload.categories.len(),
            bytes: bytes.len() as u64,
            created_at: payload.metadata.created_at.clone(),
        })
    }

    pub fn preview_restore(
        &self,
        password: SecretString,
        source: &Path,
        mode: RestoreMode,
    ) -> AppResult<RestorePreview> {
        let decoded = decode_file(source, &password)?;
        validate_payload(&decoded.payload)?;
        let (generation, revision, statuses, existing_count) =
            self.vault.with_database(false, |conn, key, g, r| {
                let existing = load_accounts(conn, key)?;
                let mut statuses = Vec::with_capacity(decoded.payload.accounts.len());
                for incoming in &decoded.payload.accounts {
                    let duplicate = existing.iter().any(|(a, s)| {
                        a.issuer.eq_ignore_ascii_case(&incoming.data.issuer)
                            && a.account_name
                                .eq_ignore_ascii_case(&incoming.data.account_name)
                            && s.as_slice() == incoming.secret.as_slice()
                    });
                    statuses.push(if mode == RestoreMode::Replace || !duplicate {
                        "new".to_owned()
                    } else {
                        "duplicate".to_owned()
                    });
                }
                Ok((g, r, statuses, existing.len()))
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
        *self.pending.lock().map_err(|_| AppError::Storage)? = Some(pending);
        Ok(preview)
    }

    pub fn apply_restore(
        &self,
        preview_id: Uuid,
        selection: RestoreSelection,
    ) -> AppResult<RestoreResult> {
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        let p = pending
            .as_ref()
            .filter(|p| p.id == preview_id && p.expires > Instant::now())
            .ok_or(AppError::Conflict)?;
        if p.mode == RestoreMode::Replace && !selection.confirm_replace {
            return Err(AppError::InvalidInput);
        }
        if selection.account_indices.len() > p.payload.accounts.len()
            || selection.account_indices.iter().enumerate().any(|(i, n)| {
                *n >= p.payload.accounts.len()
                    || selection.account_indices[..i].contains(n)
                    || p.statuses[*n] != "new"
            })
        {
            return Err(AppError::InvalidInput);
        }
        let result = self.vault.with_database(true, |conn, key, generation, revision| {
            if generation != p.generation || revision != p.revision {
                return Err(AppError::Conflict);
            }
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            if p.mode == RestoreMode::Replace {
                tx.execute_batch("DELETE FROM accounts; DELETE FROM categories; DELETE FROM settings;")
                    .map_err(|_| AppError::Storage)?;
            }
            let mut category_map = std::collections::HashMap::new();
            for category in &p.payload.categories {
                let existing_categories = load_categories(&tx, key)?;
                if p.mode == RestoreMode::Merge {
                    if let Some(existing) = existing_categories.iter().find(|c| c.name.eq_ignore_ascii_case(&category.name)) {
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
            let selected: std::collections::HashSet<usize> = selection.account_indices.iter().copied().collect();
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
            tx.commit().map_err(|_| AppError::Storage)?;
            Ok(RestoreResult { restored_accounts: restored_count, restored_categories: p.payload.categories.len(), mode: p.mode })
        });
        *pending = None;
        result
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
            let Some(mut config) = self.read_auto_config()? else {
                return Ok(AutoBackupStatus {
                    enabled: false,
                    paused_locked: false,
                    interval_hours: 24,
                    retention: 7,
                    last_run: None,
                });
            };
            config.enabled = false;
            self.save_auto_config(&config)?;
            return Ok(AutoBackupStatus {
                enabled: false,
                paused_locked: false,
                interval_hours: config.interval_hours,
                retention: config.retention,
                last_run: (config.last_run > 0).then_some(config.last_run),
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
        let config = AutoBackupConfig {
            enabled,
            directory,
            interval_hours,
            retention,
            password: String::from_utf8(password.as_bytes().to_vec())
                .map_err(|_| AppError::InvalidInput)?,
            last_run: 0,
        };
        self.save_auto_config(&config)?;
        Ok(AutoBackupStatus {
            enabled,
            paused_locked: false,
            interval_hours,
            retention,
            last_run: None,
        })
    }
    fn save_auto_config(&self, config: &AutoBackupConfig) -> AppResult<()> {
        self.vault
            .with_database(true, |conn, key, _, _| write_auto_config(conn, key, config))
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
        let Some(mut config) = self.read_auto_config()? else {
            return Ok(None);
        };
        if !config.enabled {
            return Ok(None);
        };
        if config.last_run > 0
            && now.saturating_sub(config.last_run) < i64::from(config.interval_hours) * 3600
        {
            return Ok(None);
        };
        let password = SecretString::from(config.password.clone());
        let path = config
            .directory
            .join(format!("authdesk-auto-{now}-{}.authdesk", Uuid::new_v4()));
        let summary = self.create_backup(password, &path)?;
        config.last_run = now;
        self.vault
            .with_background_database(|conn, key, _, _| write_auto_config(conn, key, &config))?;
        self.apply_retention(&config.directory, config.retention)?;
        Ok(Some(summary))
    }
    fn read_auto_config(&self) -> AppResult<Option<AutoBackupConfig>> {
        self.vault.with_database(false, |conn, key, _, _| {
            let blob: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT value_blob FROM settings WHERE key='auto_backup'",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|_| AppError::Storage)?;
            blob.map(|b| {
                let plain = decrypt_record(
                    key,
                    &record_aad("settings", "auto_backup", "value_blob"),
                    &b,
                )?;
                serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)
            })
            .transpose()
        })
    }
    fn apply_retention(&self, directory: &Path, retention: usize) -> AppResult<()> {
        let mut owned = Vec::new();
        for item in fs::read_dir(directory).map_err(|_| AppError::Storage)? {
            let item = item.map_err(|_| AppError::Storage)?;
            let path = item.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| AppError::Storage)?;
            if !meta.is_file() || meta.file_type().is_symlink() {
                continue;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            if !is_owned_auto_backup_name(&name) {
                continue;
            }
            let mut header = [0u8; 8];
            let mut f = fs::File::open(&path).map_err(|_| AppError::Storage)?;
            if f.read_exact(&mut header).is_ok() && &header == format::MAGIC {
                owned.push((path, name));
            }
        }
        owned.sort_by(|a, b| b.1.cmp(&a.1));
        for (path, _) in owned.into_iter().skip(retention) {
            fs::remove_file(path).map_err(|_| AppError::Storage)?;
        }
        Ok(())
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
fn write_auto_config(
    conn: &rusqlite::Connection,
    key: &[u8; 32],
    config: &AutoBackupConfig,
) -> AppResult<()> {
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
    })
}
fn validate_payload(payload: &BackupPayload) -> AppResult<()> {
    if payload.accounts.len() > format::MAX_ACCOUNTS
        || payload.categories.len() > format::MAX_CATEGORIES
    {
        return Err(AppError::InvalidInput);
    }
    payload.settings.validate()?;
    let mut ids = std::collections::HashSet::new();
    for category in &payload.categories {
        if category.id.is_empty()
            || category.name.trim().is_empty()
            || category.name.len() > 100
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
            || entry.data.issuer.len() > 200
            || entry.data.account_name.trim().is_empty()
            || entry.data.account_name.len() > 320
            || entry.data.tags.len() > 32
            || entry
                .data
                .tags
                .iter()
                .any(|tag| tag.is_empty() || tag.len() > 100)
            || entry.data.notes.len() > 16384
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
