mod otpauth;
use crate::{
    AppError, AppResult,
    accounts::service::{audit, insert_account, load_accounts},
    accounts::{AccountInput, AccountService},
    otp::decode_base32,
};
pub use otpauth::parse_otpauth;
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

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
        if text.len() > 2 * 1024 * 1024 {
            return Err(AppError::InvalidInput);
        }
        let owned_lines: Vec<Zeroizing<String>> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Zeroizing::new(l.to_owned()))
            .collect();
        if owned_lines.is_empty() || owned_lines.len() > 1000 {
            return Err(AppError::InvalidInput);
        }
        let lines: Vec<&str> = owned_lines.iter().map(|s| s.as_str()).collect();
        let (mut inputs, mut items) = (vec![], vec![]);
        let (generation, revision) =
            self.accounts
                .vault
                .with_database(false, |conn, key, generation, revision| {
                    let existing = load_accounts(conn, key)?;
                    let mut accepted: Vec<(String, String, Zeroizing<Vec<u8>>)> = vec![];
                    for (index, line) in lines.iter().enumerate() {
                        match parse_otpauth(line.trim()) {
                            Ok(input) => {
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
                            Err(_) => {
                                items.push(ImportItem {
                                    index,
                                    issuer: None,
                                    account_name: None,
                                    status: "invalid".into(),
                                    error_code: Some("invalid_otpauth".into()),
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
