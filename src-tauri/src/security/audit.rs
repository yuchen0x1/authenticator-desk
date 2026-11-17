use std::{fs, io::Read, path::Path};

use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::Serialize;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    AppError, AppResult,
    accounts::{
        AccountData,
        service::{load_accounts, load_categories},
    },
    vault::{IntegrityReport, VaultService, decrypt_record, record_aad},
};

const MAX_SCANNED_FILES: usize = 100;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct SecurityAudit {
    pub checked_at: String,
    pub locked: bool,
    pub integrity: IntegrityReport,
    pub corrupt_record_count: usize,
    pub duplicate_count: usize,
    pub invalid_config_count: usize,
    pub plaintext_file_findings: Vec<String>,
    pub permission_findings: String,
    pub backup_count: usize,
    pub last_backup_at: Option<String>,
    pub backup_configured: bool,
    pub time_status: String,
}

pub fn run_security_audit(
    vault: &VaultService,
    app_data_dir: &Path,
    backup_dir: Option<&Path>,
) -> AppResult<SecurityAudit> {
    let integrity = vault.integrity_check().unwrap_or(IntegrityReport {
        ok: false,
        user_version: 0,
    });
    let (corrupt, invalid, duplicate_count, mut fingerprints, configured) =
        vault.with_database(false, |conn, key, _, _| {
            let mut corrupt = 0usize;
            let mut invalid = 0usize;
            let mut stmt = conn
                .prepare("SELECT id,details_blob,secret_blob FROM accounts ORDER BY id")
                .map_err(|_| AppError::Storage)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .map_err(|_| AppError::Storage)?;
            let mut seen: Vec<(String, String, Zeroizing<Vec<u8>>)> = Vec::new();
            let mut duplicates = Vec::new();
            for row in rows {
                let (id, details, secret) = row.map_err(|_| AppError::Storage)?;
                let plain = match decrypt_record(
                    key,
                    &record_aad("accounts", &id, "details_blob"),
                    &details,
                ) {
                    Ok(v) => v,
                    Err(_) => {
                        corrupt += 1;
                        continue;
                    }
                };
                let account: AccountData = match serde_json::from_slice(&plain) {
                    Ok(v) => v,
                    Err(_) => {
                        corrupt += 1;
                        continue;
                    }
                };
                if account.id != id {
                    corrupt += 1;
                    continue;
                }
                let secret =
                    match decrypt_record(key, &record_aad("accounts", &id, "secret_blob"), &secret)
                    {
                        Ok(v) => v,
                        Err(_) => {
                            corrupt += 1;
                            continue;
                        }
                    };
                if account
                    .config(&secret)
                    .and_then(|config| config.validate())
                    .is_err()
                    || !["totp", "hotp"].contains(&account.kind.as_str())
                    || !(1..=86400).contains(&account.period_secs)
                    || account.tags.len() > 32
                    || account
                        .tags
                        .iter()
                        .any(|tag| tag.is_empty() || tag.len() > 100)
                {
                    invalid += 1;
                }
                if seen.iter().any(|(issuer, name, value)| {
                    issuer.eq_ignore_ascii_case(&account.issuer)
                        && name.eq_ignore_ascii_case(&account.account_name)
                        && value.as_slice() == secret.as_slice()
                }) {
                    duplicates.push(id.clone());
                } else {
                    seen.push((account.issuer.clone(), account.account_name.clone(), secret));
                }
            }
            drop(stmt);
            for category in raw_category_blobs(conn) {
                match category {
                    Ok((id, blob)) => {
                        let plaintext = decrypt_record(
                            key,
                            &record_aad("categories", &id, "details_blob"),
                            &blob,
                        );
                        if plaintext.is_err() {
                            corrupt += 1;
                        } else if let Ok(value) = plaintext {
                            if serde_json::from_slice::<crate::accounts::CategoryView>(&value)
                                .is_err()
                            {
                                corrupt += 1;
                            }
                        }
                    }
                    Err(_) => corrupt += 1,
                }
            }
            let setting: rusqlite::Result<Option<Vec<u8>>> = conn
                .query_row(
                    "SELECT value_blob FROM settings WHERE key='preferences'",
                    [],
                    |r| r.get(0),
                )
                .optional();
            match setting {
                Ok(Some(blob)) => match decrypt_record(
                    key,
                    &record_aad("settings", "preferences", "value_blob"),
                    &blob,
                ) {
                    Ok(v) => match serde_json::from_slice::<crate::accounts::AppSettings>(&v) {
                        Ok(settings) if settings.validate().is_err() => invalid += 1,
                        Err(_) => corrupt += 1,
                        _ => {}
                    },
                    Err(_) => corrupt += 1,
                },
                Err(_) => corrupt += 1,
                _ => {}
            }
            let auto: rusqlite::Result<Option<Vec<u8>>> = conn
                .query_row(
                    "SELECT value_blob FROM settings WHERE key='auto_backup'",
                    [],
                    |r| r.get(0),
                )
                .optional();
            let configured = match auto {
                Ok(Some(blob)) => match decrypt_record(
                    key,
                    &record_aad("settings", "auto_backup", "value_blob"),
                    &blob,
                ) {
                    Ok(_) => true,
                    Err(_) => {
                        corrupt += 1;
                        false
                    }
                },
                Ok(None) => false,
                Err(_) => {
                    corrupt += 1;
                    false
                }
            };
            if load_categories(conn, key).is_err() {
                corrupt += 1;
            }
            let _ = load_accounts(conn, key);
            Ok((corrupt, invalid, duplicates.len(), seen, configured))
        })?;
    let plaintext_file_findings = scan_for_plaintext(app_data_dir, &mut fingerprints)?;
    let (backup_count, last_backup_at) = backup_state(backup_dir)?;
    Ok(SecurityAudit {
        checked_at: Utc::now().to_rfc3339(),
        locked: false,
        integrity,
        corrupt_record_count: corrupt,
        duplicate_count,
        invalid_config_count: invalid,
        plaintext_file_findings,
        permission_findings: "not_checked_windows_acl_query_unavailable".into(),
        backup_count,
        last_backup_at,
        backup_configured: configured,
        time_status: "system_clock_only_not_calibrated".into(),
    })
}

fn raw_category_blobs(conn: &rusqlite::Connection) -> Vec<rusqlite::Result<(String, Vec<u8>)>> {
    let mut output = Vec::new();
    let mut stmt = match conn.prepare("SELECT id,details_blob FROM categories") {
        Ok(v) => v,
        Err(e) => return vec![Err(e)],
    };
    let rows = match stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
        Ok(v) => v,
        Err(e) => return vec![Err(e)],
    };
    for row in rows {
        output.push(row);
    }
    output
}

fn scan_for_plaintext(
    root: &Path,
    secrets: &mut [(String, String, Zeroizing<Vec<u8>>)],
) -> AppResult<Vec<String>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut findings = Vec::new();
    let mut count = 0usize;
    for entry in fs::read_dir(root).map_err(|_| AppError::Storage)? {
        let entry = entry.map_err(|_| AppError::Storage)?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| AppError::Storage)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|v| v.to_str()) else {
            continue;
        };
        if !["txt", "json", "csv"].contains(&ext.to_ascii_lowercase().as_str()) {
            continue;
        }
        count += 1;
        if count > MAX_SCANNED_FILES {
            findings.push("scan_limit_reached".into());
            break;
        }
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let mut bytes = Zeroizing::new(Vec::with_capacity(meta.len() as usize));
        fs::File::open(&path)
            .map_err(|_| AppError::Storage)?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AppError::Storage)?;
        let mut found = false;
        for (_, _, secret) in secrets.iter() {
            if secret.len() >= 10 && bytes.windows(secret.len()).any(|w| w == secret.as_slice()) {
                found = true;
                break;
            }
        }
        if !found {
            for (_, _, secret) in secrets.iter() {
                let encoded = Zeroizing::new(base32::encode(
                    base32::Alphabet::Rfc4648 { padding: false },
                    secret,
                ));
                if encoded.len() >= 16
                    && bytes
                        .windows(encoded.len())
                        .any(|w| w.eq_ignore_ascii_case(encoded.as_bytes()))
                {
                    found = true;
                    break;
                }
            }
        }
        if found {
            let name = entry
                .file_name()
                .to_string_lossy()
                .chars()
                .take(120)
                .collect::<String>();
            findings.push(name);
        }
    }
    for (issuer, name, secret) in secrets.iter_mut() {
        issuer.zeroize();
        name.zeroize();
        secret.zeroize();
    }
    Ok(findings)
}

fn backup_state(directory: Option<&Path>) -> AppResult<(usize, Option<String>)> {
    let Some(directory) = directory else {
        return Ok((0, None));
    };
    if !directory.exists() {
        return Ok((0, None));
    }
    let mut count = 0usize;
    let mut latest = None;
    for entry in fs::read_dir(directory).map_err(|_| AppError::Storage)? {
        let entry = entry.map_err(|_| AppError::Storage)?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| AppError::Storage)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || path.extension().and_then(|x| x.to_str()) != Some("authdesk")
        {
            continue;
        }
        let mut magic = [0u8; 8];
        if fs::File::open(&path)
            .map_err(|_| AppError::Storage)?
            .read_exact(&mut magic)
            .is_err()
            || &magic != b"AUTHDESK"
        {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs().to_string());
        if let Some(value) = modified {
            if latest.as_ref().is_none_or(|old: &String| value > *old) {
                latest = Some(value);
            }
        }
        count += 1;
    }
    Ok((count, latest))
}

#[cfg(test)]
mod tests {
    use super::run_security_audit;
    use crate::{
        accounts::{AccountInput, AccountService},
        vault::{SecretString, VaultService, create_vault},
    };
    use std::{path::PathBuf, sync::Arc};
    fn fixture() -> (PathBuf, Arc<VaultService>, AccountService) {
        let dir = std::env::temp_dir().join(format!("authdesk-audit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vault.sqlite3");
        create_vault(SecretString::from("audit-test"), &path).unwrap();
        let vault = Arc::new(VaultService::new(Some(path)));
        vault
            .unlock_vault(SecretString::from("audit-test"))
            .unwrap();
        let accounts = AccountService::new(vault.clone());
        (dir, vault, accounts)
    }
    fn input(name: &str) -> AccountInput {
        let mut value = AccountInput::default();
        value.issuer = "GitHub".into();
        value.account_name = name.into();
        value.secret = Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".into());
        value
    }
    #[test]
    fn accepts_standard_sha1_totp_and_detects_plaintext_fixture() {
        let (dir, vault, accounts) = fixture();
        accounts.create_account(input("alice")).unwrap();
        std::fs::write(dir.join("leak.txt"), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        let report = run_security_audit(&vault, &dir, None).unwrap();
        assert_eq!(report.invalid_config_count, 0);
        assert_eq!(report.corrupt_record_count, 0);
        assert_eq!(report.plaintext_file_findings, vec!["leak.txt"]);
        assert_eq!(report.time_status, "system_clock_only_not_calibrated");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn sha1_is_not_labelled_weak() {
        let (dir, vault, accounts) = fixture();
        accounts.create_account(input("alice")).unwrap();
        let report = run_security_audit(&vault, &dir, None).unwrap();
        assert_eq!(report.invalid_config_count, 0);
        assert!(report.integrity.ok);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reports_duplicate_invalid_and_authenticated_corrupt_records() {
        let (dir, vault, accounts) = fixture();
        accounts.create_account(input("alice")).unwrap();
        vault
            .with_database(true, |conn, key, _, _| {
                let mut entries = crate::accounts::service::load_accounts(conn, key)?;
                let (data, secret) = entries.pop().unwrap();
                let mut duplicate = data.clone();
                duplicate.id = uuid::Uuid::new_v4().to_string();
                crate::accounts::service::save_account(conn, key, &duplicate, &secret)?;
                let mut invalid = data.clone();
                invalid.id = uuid::Uuid::new_v4().to_string();
                invalid.account_name = "invalid".into();
                invalid.digits = 9;
                crate::accounts::service::save_account(conn, key, &invalid, &secret)?;
                let mut corrupt = data.clone();
                corrupt.id = uuid::Uuid::new_v4().to_string();
                corrupt.account_name = "corrupt".into();
                crate::accounts::service::save_account(conn, key, &corrupt, &secret)?;
                conn.execute(
                    "UPDATE accounts SET secret_blob=X'00' WHERE id=?1",
                    [&corrupt.id],
                )
                .map_err(|_| crate::AppError::Storage)?;
                Ok(())
            })
            .unwrap();
        let report = run_security_audit(&vault, &dir, None).unwrap();
        assert_eq!(report.duplicate_count, 1);
        assert_eq!(report.invalid_config_count, 1);
        assert_eq!(report.corrupt_record_count, 1);
        assert_eq!(report.backup_count, 0);
        assert_eq!(report.last_backup_at, None);
        vault.lock_vault().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
