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
pub use twofas::TwoFasAdapter;
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_IMPORT_ITEMS: usize = 1000;

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
    fn preview(&self, bytes: &[u8], password: Option<&SecretString>) -> AppResult<MigrationPreview>;
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
    generation: u64,
    revision: u64,
    inputs: Vec<Option<AccountInput>>,
    statuses: Vec<String>,
}
pub struct ImportService {
    accounts: Arc<AccountService>,
    pending: Mutex<Option<Pending>>,
}
impl ImportService {
    pub fn new(accounts: Arc<AccountService>) -> Self {
        Self {
            accounts,
            pending: Mutex::new(None),
        }
    }
    pub fn clear(&self) -> AppResult<()> {
        *self.pending.lock().map_err(|_| AppError::Storage)? = None;
        Ok(())
    }
    pub fn preview_import(&self, text: String) -> AppResult<ImportPreview> {
        let text = Zeroizing::new(text);
        if text.len() > MAX_IMPORT_BYTES {
            return Err(AppError::InvalidInput);
        }
        let owned_lines: Vec<Zeroizing<String>> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Zeroizing::new(l.to_owned()))
            .collect();
        if owned_lines.is_empty() || owned_lines.len() > MAX_IMPORT_ITEMS {
            return Err(AppError::InvalidInput);
        }
        let items = owned_lines
            .iter()
            .map(|line| match parse_otpauth(line.trim()) {
                Ok(input) => MigrationItem {
                    issuer: Some(input.issuer.clone()),
                    account_name: Some(input.account_name.clone()),
                    input: Some(input),
                    error_code: None,
                },
                Err(_) => invalid_item("invalid_otpauth"),
            })
            .collect();
        self.store_preview(MigrationPreview {
            format: "otpauth_text".into(),
            items,
        })
    }
    /// Parse a third-party backup file by content detection.
    ///
    /// The bytes are bounded, handed to the first adapter that recognizes
    /// the content, and classified against the vault exactly like text
    /// imports so commit keeps its single transactional path.
    pub fn preview_file(&self, bytes: Vec<u8>) -> AppResult<ImportPreview> {
        if bytes.len() > MAX_IMPORT_BYTES {
            return Err(AppError::InvalidInput);
        }
        let bytes = Zeroizing::new(bytes);
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
        self.store_preview(preview)
    }
    fn store_preview(&self, preview: MigrationPreview) -> AppResult<ImportPreview> {
        if preview.items.is_empty() {
            return Err(AppError::InvalidInput);
        }
        let (mut inputs, mut items) = (vec![], vec![]);
        let (generation, revision) =
            self.accounts
                .vault
                .with_database(false, |conn, key, generation, revision| {
                    let existing = load_accounts(conn, key)?;
                    let mut accepted: Vec<(String, String, Zeroizing<Vec<u8>>)> = vec![];
                    for (index, row) in preview.items.into_iter().enumerate() {
                        match row.input {
                            Some(input) => {
                                let secret = decode_base32(
                                    input.secret.as_deref().ok_or(AppError::InvalidInput)?,
                                )?;
                                let issuer = input.issuer.to_lowercase();
                                let account = input.account_name.to_lowercase();
                                let duplicate = existing.iter().any(|(a, s)| {
                                    a.issuer.to_lowercase() == issuer
                                        && a.account_name.to_lowercase() == account
                                        && s.as_slice() == secret.as_slice()
                                }) || accepted.iter().any(|(i, a, s)| {
                                    i == &issuer && a == &account && s.as_slice() == secret.as_slice()
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
                    Ok((generation, revision))
                })?;
        let id = Uuid::new_v4().to_string();
        let statuses = items.iter().map(|i| i.status.clone()).collect();
        *self.pending.lock().map_err(|_| AppError::Storage)? = Some(Pending {
            id: id.clone(),
            expires: Instant::now() + Duration::from_secs(300),
            generation,
            revision,
            inputs,
            statuses,
        });
        Ok(ImportPreview {
            preview_id: id,
            format: preview.format,
            items,
        })
    }
    pub fn commit_import(&self, id: &str, selection: &[usize]) -> AppResult<usize> {
        let mut pending = self.pending.lock().map_err(|_| AppError::Storage)?;
        let p = pending
            .as_ref()
            .filter(|p| p.id == id && p.expires > Instant::now())
            .ok_or(AppError::Conflict)?;
        if selection.is_empty()
            || selection.iter().enumerate().any(|(i, n)| {
                *n >= p.inputs.len() || selection[..i].contains(n) || p.statuses[*n] != "new"
            })
        {
            return Err(AppError::InvalidInput);
        }
        let result = self
            .accounts
            .vault
            .with_database(true, |conn, key, generation, revision| {
                if p.generation != generation || p.revision != revision {
                    return Err(AppError::Conflict);
                }
                let tx = conn.transaction().map_err(|_| AppError::Storage)?;
                for index in selection {
                    insert_account(
                        &tx,
                        key,
                        p.inputs[*index].as_ref().ok_or(AppError::InvalidInput)?,
                    )?;
                }
                audit(&tx, key, "accounts_imported", None)?;
                tx.commit().map_err(|_| AppError::Storage)?;
                Ok(selection.len())
            });
        // Drop retained secrets on both successful commits and stale/failed attempts.
        *pending = None;
        result
    }
}
