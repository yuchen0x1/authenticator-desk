mod aegis;
mod andotp;
pub(crate) mod base64;
mod ente;
mod freeotp;
mod google_migration;
mod otpauth;
mod twofas;
use crate::{
    AppError, AppResult,
    accounts::service::{audit, insert_account, load_accounts},
    accounts::{AccountInput, AccountService},
    otp::decode_base32,
    vault::SecretString,
};
pub use aegis::AegisAdapter;
pub use andotp::AndOtpAdapter;
pub use ente::EnteAdapter;
pub use freeotp::FreeOtpAdapter;
pub use google_migration::GoogleMigrationAdapter;
pub use otpauth::{OtpAuthListAdapter, parse_otpauth};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
pub use twofas::TwoFasAdapter;
use uuid::Uuid;
use zeroize::Zeroizing;

pub(crate) const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_IMPORT_ITEMS: usize = 1000;
const PREVIEW_TTL_SECS: u64 = 300;

/// One parsed migration row before vault duplicate classification.
///
/// `input` is `None` for rows the source file could not map; `error_code`
/// carries the stable reason shown to the user.
pub struct MigrationItem {
    pub input: Option<AccountInput>,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub error_code: Option<String>,
}

/// Adapter output: a detected format name plus per-item rows.
pub struct MigrationPreview {
    pub format: String,
    pub items: Vec<MigrationItem>,
}

/// A content-based reader for one third-party backup format.
///
/// Detection is bounded and content-based; adapters normalize rows into
/// [`AccountInput`] and never touch the vault themselves. The password is
/// part of the interface for future adapters with verified decryption
/// vectors; every encrypted variant this release recognizes is rejected
/// with `UnsupportedEncryptedFormat` before password handling, and no
/// bypass is attempted.
pub trait MigrationAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self, bytes: &[u8]) -> bool;
    fn preview(&self, bytes: &[u8], password: Option<&SecretString>)
    -> AppResult<MigrationPreview>;
}

/// Parse bounded JSON input, or `None` when the bytes are not JSON.
fn parse_json(bytes: &[u8]) -> Option<serde_json::Value> {
    let text = std::str::from_utf8(bytes).ok()?;
    serde_json::from_str(text).ok()
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn json_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

fn invalid_item(error_code: &str) -> MigrationItem {
    MigrationItem {
        input: None,
        issuer: None,
        account_name: None,
        error_code: Some(error_code.into()),
    }
}

/// Apply the same label normalization used by stored accounts and the
/// account uniqueness check: trim surrounding whitespace before case folding.
fn normalize_label(value: &str) -> String {
    value.trim().to_lowercase()
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportItem {
    pub index: usize,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ImportPreview {
    pub preview_id: String,
    pub format: String,
    pub items: Vec<ImportItem>,
}
struct Pending {
    id: String,
    expires: Instant,
    epoch: u64,
    generation: u64,
    revision: u64,
    inputs: Vec<Option<AccountInput>>,
    statuses: Vec<String>,
}
pub struct ImportService {
    accounts: Arc<AccountService>,
    pending: Arc<Mutex<Option<Pending>>>,
    epoch: Arc<AtomicU64>,
    /// Serializes preview installation, cancellation, and commit selection so
    /// a cancel/clear cannot race a commit that has already chosen a pending
    /// preview. Vault invalidation callbacks never take this lock.
    operation: Mutex<()>,
}
impl ImportService {
    pub fn new(accounts: Arc<AccountService>) -> Self {
        let pending = Arc::new(Mutex::new(None));
        let epoch = Arc::new(AtomicU64::new(0));
        let vault_epoch = epoch.clone();
        let vault_pending = Arc::downgrade(&pending);
        // Lock or revision changes drop retained plaintext immediately. The
        // callback only touches import-owned state, never the vault.
        let _ = accounts.vault.register_invalidator(Arc::new(move || {
            vault_epoch.fetch_add(1, Ordering::AcqRel);
            if let Some(pending) = vault_pending.upgrade()
                && let Ok(mut pending) = pending.lock()
            {
                *pending = None;
            }
        }));
        Self {
            accounts,
            pending,
            epoch,
            operation: Mutex::new(()),
        }
    }

    /// Drop the retained preview and prevent an in-flight installation from
    /// repopulating it after a lock, cancel, or explicit clear.
    pub fn clear(&self) -> AppResult<()> {
        let _operation = self.operation.lock().map_err(|_| AppError::Storage)?;
        self.epoch.fetch_add(1, Ordering::AcqRel);
        *self.pending.lock().map_err(|_| AppError::Storage)? = None;
        Ok(())
    }

    /// Drop only the matching pending preview. A newer preview is untouched.
    pub fn cancel_preview(&self, id: &str) -> AppResult<()> {
        let _operation = self.operation.lock().map_err(|_| AppError::Storage)?;
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        if pending.as_ref().is_some_and(|pending| pending.id == id) {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            *pending = None;
        }
        Ok(())
    }

    /// Drop a pending preview whose five-minute prompt deadline has elapsed.
    /// Safe to call from the maintenance loop while unlocked.
    pub fn cleanup_expired(&self) -> AppResult<()> {
        let _operation = self.operation.lock().map_err(|_| AppError::Storage)?;
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        if pending
            .as_ref()
            .is_some_and(|pending| pending.expires <= Instant::now())
        {
            *pending = None;
        }
        Ok(())
    }

    /// Parse pasted text, including Google Authenticator migration URIs from
    /// QR images. Normal `otpauth://` lines and migration batches may be mixed;
    /// the aggregate item count stays bounded before any vault access.
    pub fn preview_import(&self, text: String) -> AppResult<ImportPreview> {
        let epoch = self.epoch.load(Ordering::Acquire);
        let text = Zeroizing::new(text);
        if text.len() > MAX_IMPORT_BYTES {
            return Err(AppError::InvalidInput);
        }
        let owned_lines: Vec<Zeroizing<String>> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| Zeroizing::new(line.to_owned()))
            .collect();
        if owned_lines.is_empty() || owned_lines.len() > MAX_IMPORT_ITEMS {
            return Err(AppError::InvalidInput);
        }
        let all_migration = owned_lines
            .iter()
            .all(|line| line.trim().starts_with("otpauth-migration://"));
        let any_migration = owned_lines
            .iter()
            .any(|line| line.trim().starts_with("otpauth-migration://"));
        let mut items = Vec::new();
        for line in &owned_lines {
            if line.trim().starts_with("otpauth-migration://") {
                // Do not turn a decoder/size-limit error into a one-row
                // preview: oversized migration batches must fail before
                // vault/database work on the text/QR path as well.
                let preview = GoogleMigrationAdapter.preview(line.trim().as_bytes())?;
                items.extend(preview.items);
            } else {
                match parse_otpauth(line.trim()) {
                    Ok(input) => items.push(MigrationItem {
                        issuer: Some(input.issuer.clone()),
                        account_name: Some(input.account_name.clone()),
                        input: Some(input),
                        error_code: None,
                    }),
                    Err(_) => items.push(invalid_item("invalid_otpauth")),
                }
            }
            if items.len() > MAX_IMPORT_ITEMS {
                return Err(AppError::InvalidInput);
            }
        }
        let format = if any_migration && all_migration {
            "google_migration"
        } else {
            "otpauth_text"
        };
        self.store_preview(
            MigrationPreview {
                format: format.into(),
                items,
            },
            epoch,
        )
    }

    /// Parse a third-party backup file by content detection.
    ///
    /// The bytes are bounded, handed to the first adapter that recognizes
    /// the content, and classified against the vault exactly like text
    /// imports so commit keeps its single transactional path. The aggregate
    /// row count is checked before the vault is opened.
    pub fn preview_file(&self, bytes: Vec<u8>) -> AppResult<ImportPreview> {
        let epoch = self.epoch.load(Ordering::Acquire);
        let bytes = Zeroizing::new(bytes);
        if bytes.len() > MAX_IMPORT_BYTES {
            return Err(AppError::InvalidInput);
        }
        let adapters: [&dyn MigrationAdapter; 7] = [
            &google_migration::GoogleMigrationAdapter,
            &freeotp::FreeOtpAdapter,
            &andotp::AndOtpAdapter,
            &ente::EnteAdapter,
            &aegis::AegisAdapter,
            &twofas::TwoFasAdapter,
            &otpauth::OtpAuthListAdapter,
        ];
        let adapter = adapters
            .iter()
            .find(|adapter| adapter.detect(&bytes))
            .ok_or(AppError::InvalidInput)?;
        let preview = adapter.preview(&bytes, None)?;
        if preview.items.len() > MAX_IMPORT_ITEMS {
            return Err(AppError::InvalidInput);
        }
        self.store_preview(preview, epoch)
    }

    fn store_preview(&self, preview: MigrationPreview, epoch: u64) -> AppResult<ImportPreview> {
        // Bound the aggregate row count before taking any lock or opening the
        // database. Adapters cap their own batches, but a multi-batch Google
        // migration is bounded only here and in the adapter's line loop.
        if preview.items.is_empty() || preview.items.len() > MAX_IMPORT_ITEMS {
            return Err(AppError::InvalidInput);
        }
        let _operation = self.operation.lock().map_err(|_| AppError::Storage)?;
        // Capture the request epoch before parsing, not here: a clear/lock
        // during parsing must also prevent this request from repopulating.
        if self.epoch.load(Ordering::Acquire) != epoch {
            return Err(AppError::Conflict);
        }
        let (generation, revision, inputs, items) =
            self.accounts
                .vault
                .with_database(false, |conn, key, generation, revision| {
                    let existing = load_accounts(conn, key)?;
                    let mut accepted: Vec<(String, String, Zeroizing<Vec<u8>>)> = vec![];
                    let mut inputs = Vec::with_capacity(preview.items.len());
                    let mut items = Vec::with_capacity(preview.items.len());
                    for (index, row) in preview.items.into_iter().enumerate() {
                        match row.input {
                            Some(input) => {
                                let secret = decode_base32(
                                    input.secret.as_deref().ok_or(AppError::InvalidInput)?,
                                )?;
                                let issuer = normalize_label(&input.issuer);
                                let account = normalize_label(&input.account_name);
                                let duplicate = existing.iter().any(|(a, s)| {
                                    normalize_label(&a.issuer) == issuer
                                        && normalize_label(&a.account_name) == account
                                        && s.as_slice() == secret.as_slice()
                                }) || accepted.iter().any(|(i, a, s)| {
                                    i == &issuer
                                        && a == &account
                                        && s.as_slice() == secret.as_slice()
                                });
                                let status = if duplicate { "duplicate" } else { "new" };
                                items.push(ImportItem {
                                    index,
                                    issuer: Some(input.issuer.clone()),
                                    account_name: Some(input.account_name.clone()),
                                    status: status.into(),
                                    error_code: None,
                                });
                                accepted.push((issuer, account, secret));
                                inputs.push(Some(input));
                            }
                            None => {
                                items.push(ImportItem {
                                    index,
                                    issuer: row.issuer,
                                    account_name: row.account_name,
                                    status: "invalid".into(),
                                    error_code: Some(
                                        row.error_code.unwrap_or_else(|| "invalid_otpauth".into()),
                                    ),
                                });
                                inputs.push(None);
                            }
                        }
                    }
                    Ok((generation, revision, inputs, items))
                })?;
        if self.epoch.load(Ordering::Acquire) != epoch {
            return Err(AppError::Conflict);
        }
        let id = Uuid::new_v4().to_string();
        let statuses = items.iter().map(|item| item.status.clone()).collect();
        let expires = Instant::now() + Duration::from_secs(PREVIEW_TTL_SECS);
        {
            let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
            if self.epoch.load(Ordering::Acquire) != epoch {
                return Err(AppError::Conflict);
            }
            *pending = Some(Pending {
                id: id.clone(),
                expires,
                epoch,
                generation,
                revision,
                inputs,
                statuses,
            });
            if self.epoch.load(Ordering::Acquire) != epoch {
                *pending = None;
                return Err(AppError::Conflict);
            }
        }
        Ok(ImportPreview {
            preview_id: id,
            format: preview.format,
            items,
        })
    }

    pub fn commit_import(&self, id: &str, selection: &[usize]) -> AppResult<usize> {
        // Selecting and taking the pending row is serialized against cancel,
        // clear, and preview replacement. The lock is released before the
        // vault operation so a vault invalidation callback cannot deadlock.
        let _operation = self.operation.lock().map_err(|_| AppError::Storage)?;
        let pending = {
            let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
            if pending
                .as_ref()
                .is_some_and(|pending| pending.expires <= Instant::now())
            {
                *pending = None;
                return Err(AppError::Conflict);
            }
            let existing = pending
                .as_ref()
                .filter(|pending| pending.id == id)
                .ok_or(AppError::Conflict)?;
            if selection.is_empty()
                || selection.iter().enumerate().any(|(i, n)| {
                    *n >= existing.inputs.len()
                        || selection[..i].contains(n)
                        || existing.statuses[*n] != "new"
                })
            {
                *pending = None;
                return Err(AppError::InvalidInput);
            }
            pending.take().ok_or(AppError::Conflict)?
        };
        if pending.epoch != self.epoch.load(Ordering::Acquire) {
            return Err(AppError::Conflict);
        }
        self.accounts
            .vault
            .with_database(true, |conn, key, generation, revision| {
                if pending.generation != generation
                    || pending.revision != revision
                    || pending.epoch != self.epoch.load(Ordering::Acquire)
                {
                    return Err(AppError::Conflict);
                }
                let tx = conn.transaction().map_err(|_| AppError::Storage)?;
                for index in selection {
                    insert_account(
                        &tx,
                        key,
                        pending.inputs[*index]
                            .as_ref()
                            .ok_or(AppError::InvalidInput)?,
                    )?;
                }
                audit(&tx, key, "accounts_imported", None)?;
                tx.commit().map_err(|_| AppError::Storage)?;
                Ok(selection.len())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::VaultService;

    const URI: &str = "otpauth://totp/Demo:alice@example.com?secret=JBSWY3DPEHPK3PXP";

    struct Harness {
        path: std::path::PathBuf,
        vault: Arc<VaultService>,
        accounts: Arc<AccountService>,
        imports: ImportService,
    }

    impl Harness {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("authdesk-import-unit-{}.sqlite3", Uuid::new_v4()));
            let vault = Arc::new(VaultService::new(Some(path.clone())));
            vault
                .create_vault(SecretString::from("import-test"), &path)
                .unwrap();
            vault
                .unlock_vault(SecretString::from("import-test"))
                .unwrap();
            let accounts = Arc::new(AccountService::new(vault.clone()));
            let imports = ImportService::new(accounts.clone());
            Self {
                path,
                vault,
                accounts,
                imports,
            }
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            self.vault.lock_vault().unwrap();
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn pending_plaintext_is_dropped_on_expiry_cancel_lock_and_revision() {
        let h = Harness::new();
        let preview = h.imports.preview_import(URI.into()).unwrap();
        {
            let mut pending = h.imports.pending.lock().unwrap();
            pending.as_mut().unwrap().expires = Instant::now() - Duration::from_secs(1);
        }
        h.imports.cleanup_expired().unwrap();
        assert!(h.imports.pending.lock().unwrap().is_none());
        assert!(matches!(
            h.imports.commit_import(&preview.preview_id, &[0]),
            Err(AppError::Conflict)
        ));

        let preview = h.imports.preview_import(URI.into()).unwrap();
        {
            let mut pending = h.imports.pending.lock().unwrap();
            pending.as_mut().unwrap().expires = Instant::now() - Duration::from_secs(1);
        }
        assert!(matches!(
            h.imports.commit_import("stale-id", &[0]),
            Err(AppError::Conflict)
        ));
        assert!(h.imports.pending.lock().unwrap().is_none());
        // Retrying with the right id cannot restore an expired payload.
        assert!(h.imports.commit_import(&preview.preview_id, &[0]).is_err());

        let preview = h.imports.preview_import(URI.into()).unwrap();
        h.imports.cancel_preview(&preview.preview_id).unwrap();
        assert!(h.imports.pending.lock().unwrap().is_none());

        h.imports.preview_import(URI.into()).unwrap();
        h.accounts
            .create_account(parse_otpauth(&URI.replace("alice", "bob")).unwrap())
            .unwrap();
        assert!(h.imports.pending.lock().unwrap().is_none());

        h.imports.preview_import(URI.into()).unwrap();
        h.vault.lock_vault().unwrap();
        assert!(h.imports.pending.lock().unwrap().is_none());
    }

    #[test]
    fn stale_request_epoch_cannot_install_after_clear_or_generation_change() {
        let h = Harness::new();
        let parsed = || MigrationPreview {
            format: "otpauth_text".into(),
            items: vec![MigrationItem {
                input: Some(parse_otpauth(URI).unwrap()),
                issuer: None,
                account_name: None,
                error_code: None,
            }],
        };
        // Deterministic pause between parsing and installation: clear must
        // invalidate even a request that has not reached the vault yet.
        let epoch = h.imports.epoch.load(Ordering::Acquire);
        h.imports.clear().unwrap();
        assert!(matches!(
            h.imports.store_preview(parsed(), epoch),
            Err(AppError::Conflict)
        ));
        assert!(h.imports.pending.lock().unwrap().is_none());

        let epoch = h.imports.epoch.load(Ordering::Acquire);
        h.vault.lock_vault().unwrap();
        h.vault
            .unlock_vault(SecretString::from("import-test"))
            .unwrap();
        assert!(matches!(
            h.imports.store_preview(parsed(), epoch),
            Err(AppError::Conflict)
        ));
        assert!(h.imports.pending.lock().unwrap().is_none());
        // A new request in the live generation is still usable.
        assert!(h.imports.preview_import(URI.into()).is_ok());
    }

    #[test]
    fn final_row_bound_is_checked_before_vault_access() {
        let vault = Arc::new(VaultService::default());
        let imports = ImportService::new(Arc::new(AccountService::new(vault)));
        let preview = MigrationPreview {
            format: "test".into(),
            items: (0..=MAX_IMPORT_ITEMS)
                .map(|_| invalid_item("invalid"))
                .collect(),
        };
        assert!(matches!(
            imports.store_preview(preview, 0),
            Err(AppError::InvalidInput)
        ));
    }
}
