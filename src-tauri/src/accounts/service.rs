use super::model::*;
use crate::{
    AppError, AppResult,
    otp::{MonotonicClock, OtpKind, decode_base32, generate_code, time_state},
    vault::{VaultService, decrypt_record, encrypt_record, record_aad},
};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct AccountService {
    pub(crate) vault: Arc<VaultService>,
    clock: Mutex<MonotonicClock>,
}
impl AccountService {
    pub fn new(vault: Arc<VaultService>) -> Self {
        Self {
            vault,
            clock: Mutex::new(MonotonicClock::new()),
        }
    }
    pub fn list_accounts(&self, query: AccountQuery) -> AppResult<Vec<AccountView>> {
        self.vault.with_database(false, |conn, key, _, _| {
            let mut records = load_accounts(conn, key)?;
            let needle = query.search.to_lowercase();
            records.retain(|(a, _)| {
                (!query.favorite_only || a.favorite)
                    && query
                        .category_id
                        .as_ref()
                        .is_none_or(|c| a.category_id.as_ref() == Some(c))
                    && query.tag.as_ref().is_none_or(|t| a.tags.contains(t))
                    && (needle.is_empty()
                        || a.issuer.to_lowercase().contains(&needle)
                        || a.account_name.to_lowercase().contains(&needle)
                        || a.tags.iter().any(|t| t.to_lowercase().contains(&needle)))
            });
            match query.sort.as_str() {
                "issuer" => records.sort_by_key(|(a, _)| {
                    (
                        a.issuer.to_lowercase(),
                        a.account_name.to_lowercase(),
                        a.id.clone(),
                    )
                }),
                "favorites" => {
                    records.sort_by_key(|(a, _)| (!a.favorite, a.sort_order, a.id.clone()))
                }
                "" | "manual" => records.sort_by_key(|(a, _)| (a.sort_order, a.id.clone())),
                _ => return Err(AppError::InvalidInput),
            }
            Ok(records.into_iter().map(|(a, _)| a.view()).collect())
        })
    }
    pub fn account_details(&self, id: &str) -> AppResult<AccountDetails> {
        self.vault.with_database(false, |conn, key, _, _| {
            let (a, _) = load_account(conn, key, id)?;
            Ok(AccountDetails {
                account: a.view(),
                notes: a.notes.clone(),
            })
        })
    }
    pub fn create_account(&self, input: AccountInput) -> AppResult<AccountView> {
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            let view = insert_account(&tx, key, &input)?;
            audit(&tx, key, "account_created", Some(&view.id))?;
            tx.commit().map_err(|_| AppError::Storage)?;
            Ok(view)
        })
    }
    pub fn update_account(&self, id: &str, input: AccountInput) -> AppResult<AccountView> {
        input.validate(false)?;
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            let (old, old_secret) = load_account(&tx, key, id)?;
            if old.kind == "hotp"
                && (input.kind != "hotp"
                    || decimal_u64(&input.hotp_counter)? < decimal_u64(&old.hotp_counter)?)
            {
                return Err(AppError::InvalidParameters);
            }
            let secret = match input.secret.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => decode_base32(s)?,
                None => old_secret,
            };
            let data = data_from_input(
                &input,
                old.id.clone(),
                old.sort_order,
                old.created_at.clone(),
            );
            validate_category(&tx, key, &data.category_id)?;
            ensure_unique(&tx, key, &data, &secret, Some(id))?;
            save_account(&tx, key, &data, &secret)?;
            audit(&tx, key, "account_updated", Some(id))?;
            tx.commit().map_err(|_| AppError::Storage)?;
            Ok(data.view())
        })
    }
    pub fn delete_accounts(&self, ids: &[String]) -> AppResult<()> {
        validate_ids(ids)?;
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            for id in ids {
                let count = tx
                    .execute("DELETE FROM accounts WHERE id=?1", [id])
                    .map_err(|_| AppError::Storage)?;
                if count != 1 {
                    return Err(AppError::NotFound);
                }
                audit(&tx, key, "account_deleted", Some(id))?;
            }
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    pub fn set_favorite(&self, id: &str, favorite: bool) -> AppResult<()> {
        self.mutate_accounts(&[id.into()], "favorite_changed", |a| {
            a.favorite = favorite;
            Ok(())
        })
    }
    pub fn move_accounts(&self, ids: &[String], category: Option<String>) -> AppResult<()> {
        self.vault.with_database(true, |conn, key, _, _| {
            validate_ids(ids)?;
            validate_category(conn, key, &category)?;
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            for id in ids {
                let (mut a, s) = load_account(&tx, key, id)?;
                a.category_id = category.clone();
                a.updated_at = now();
                save_account(&tx, key, &a, &s)?;
            }
            audit(&tx, key, "accounts_moved", None)?;
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    pub fn reorder_accounts(&self, ids: &[String]) -> AppResult<()> {
        validate_ids(ids)?;
        self.vault.with_database(true, |conn, key, _, _| {
            let all = load_accounts(conn, key)?;
            if ids.len() != all.len() || all.iter().any(|(a, _)| !ids.contains(&a.id)) {
                return Err(AppError::InvalidInput);
            }
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            for (index, id) in ids.iter().enumerate() {
                let (mut a, s) = load_account(&tx, key, id)?;
                a.sort_order = index as i64;
                save_account(&tx, key, &a, &s)?;
            }
            audit(&tx, key, "accounts_reordered", None)?;
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    fn mutate_accounts(
        &self,
        ids: &[String],
        event: &str,
        mut change: impl FnMut(&mut AccountData) -> AppResult<()>,
    ) -> AppResult<()> {
        validate_ids(ids)?;
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            for id in ids {
                let (mut a, s) = load_account(&tx, key, id)?;
                change(&mut a)?;
                a.updated_at = now();
                save_account(&tx, key, &a, &s)?;
            }
            audit(&tx, key, event, None)?;
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    pub fn generate_codes(&self, ids: &[String]) -> AppResult<Vec<CodeView>> {
        let sample = self
            .clock
            .lock()
            .map_err(|_| AppError::Storage)?
            .sample_system()?;
        let mut codes = self.generate_codes_at(ids, sample.unix_seconds)?;
        for code in &mut codes {
            code.clock_offset_secs = sample.clock_offset_secs;
            code.wall_clock_jump = sample.wall_clock_jump;
        }
        Ok(codes)
    }
    pub fn generate_codes_at(&self, ids: &[String], unix: i64) -> AppResult<Vec<CodeView>> {
        validate_ids(ids)?;
        self.vault.with_database(false, |conn, key, _, _| {
            ids.iter()
                .map(|id| {
                    let (a, s) = load_account(conn, key, id)?;
                    code_view(&a, &s, unix)
                })
                .collect()
        })
    }
    pub fn generate_available_codes(&self, ids: &[String]) -> AppResult<Vec<CodeView>> {
        let sample = self
            .clock
            .lock()
            .map_err(|_| AppError::Storage)?
            .sample_system()?;
        let mut codes = self.generate_available_codes_at(ids, sample.unix_seconds)?;
        for code in &mut codes {
            code.clock_offset_secs = sample.clock_offset_secs;
            code.wall_clock_jump = sample.wall_clock_jump;
        }
        Ok(codes)
    }
    pub fn generate_available_codes_at(
        &self,
        ids: &[String],
        unix: i64,
    ) -> AppResult<Vec<CodeView>> {
        validate_ids(ids)?;
        self.vault.with_database(false, |conn, key, _, _| {
            let mut codes = Vec::with_capacity(ids.len());
            for id in ids {
                let (account, secret) = load_account(conn, key, id)?;
                if !["totp", "hotp"].contains(&account.kind.as_str()) {
                    return Err(AppError::InvalidParameters);
                }
                let config = account.config(&secret)?;
                config.validate()?;
                if matches!(config.kind, OtpKind::Totp { t0, .. } if unix < t0) {
                    continue;
                }
                codes.push(code_view(&account, &secret, unix)?);
            }
            Ok(codes)
        })
    }
    pub fn advance_hotp(&self, id: &str) -> AppResult<CodeView> {
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            let (mut a, s) = load_account(&tx, key, id)?;
            if a.kind != "hotp" {
                return Err(AppError::InvalidInput);
            }
            a.hotp_counter = decimal_u64(&a.hotp_counter)?
                .checked_add(1)
                .ok_or(AppError::InvalidParameters)?
                .to_string();
            a.updated_at = now();
            save_account(&tx, key, &a, &s)?;
            audit(&tx, key, "hotp_advanced", Some(id))?;
            let result = code_view(&a, &s, Utc::now().timestamp())?;
            tx.commit().map_err(|_| AppError::Storage)?;
            Ok(result)
        })
    }
    pub fn list_categories(&self) -> AppResult<Vec<CategoryView>> {
        self.vault
            .with_database(false, |conn, key, _, _| load_categories(conn, key))
    }
    pub fn save_category(
        &self,
        id: Option<String>,
        name: &str,
        color: &str,
    ) -> AppResult<CategoryView> {
        if name.trim().is_empty()
            || name.len() > 100
            || color.len() != 7
            || !color.starts_with('#')
            || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AppError::InvalidInput);
        }
        self.vault.with_database(true, |conn, key, _, _| {
            let categories = load_categories(conn, key)?;
            if categories.iter().any(|c| c.name.to_lowercase() == name.trim().to_lowercase() && Some(&c.id) != id.as_ref()) { return Err(AppError::Conflict); }
            if let Some(id) = &id { if !categories.iter().any(|c| &c.id == id) { return Err(AppError::NotFound); } }
            let category = CategoryView { id: id.unwrap_or_else(|| Uuid::new_v4().to_string()), name: name.trim().into(), color: color.into() };
            let blob = encrypt_json(key, "categories", &category.id, "details_blob", &category)?;
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            tx.execute("INSERT INTO categories(id,details_blob,created_at,updated_at) VALUES(?1,?2,?3,?3) ON CONFLICT(id) DO UPDATE SET details_blob=excluded.details_blob,updated_at=excluded.updated_at", params![category.id, blob, now()]).map_err(|_| AppError::Storage)?;
            audit(&tx, key, "category_saved", None)?;
            tx.commit().map_err(|_| AppError::Storage)?;
            Ok(category)
        })
    }
    pub fn delete_category(&self, id: &str) -> AppResult<()> {
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            if tx
                .execute("DELETE FROM categories WHERE id=?1", [id])
                .map_err(|_| AppError::Storage)?
                != 1
            {
                return Err(AppError::NotFound);
            }
            for (mut a, s) in load_accounts(&tx, key)? {
                if a.category_id.as_deref() == Some(id) {
                    a.category_id = None;
                    save_account(&tx, key, &a, &s)?;
                }
            }
            audit(&tx, key, "category_deleted", None)?;
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    pub fn settings(&self) -> AppResult<AppSettings> {
        self.vault
            .with_database(false, |conn, key, _, _| load_settings(conn, key))
    }
    pub fn save_settings(&self, settings: AppSettings) -> AppResult<()> {
        settings.validate()?;
        let timeout = settings.idle_timeout_secs;
        self.vault.with_database(true, |conn, key, _, _| {
            let blob = encrypt_json(key, "settings", "preferences", "value_blob", &settings)?;
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            tx.execute("INSERT INTO settings(key,value_blob,updated_at) VALUES('preferences',?1,?2) ON CONFLICT(key) DO UPDATE SET value_blob=excluded.value_blob,updated_at=excluded.updated_at", params![blob, now()]).map_err(|_| AppError::Storage)?;
            audit(&tx, key, "settings_saved", None)?;
            tx.commit().map_err(|_| AppError::Storage)
        })?;
        self.vault
            .set_idle_timeout(std::time::Duration::from_secs(timeout))
    }
    pub fn audit_events(&self, limit: usize) -> AppResult<Vec<AuditEvent>> {
        self.vault
            .with_database(false, |conn, key, _, _| load_events(conn, key, limit))
    }
    pub fn record_copy(&self, id: &str) -> AppResult<()> {
        self.vault.with_database(true, |conn, key, _, _| {
            let tx = conn.transaction().map_err(|_| AppError::Storage)?;
            load_account(&tx, key, id)?;
            audit(&tx, key, "code_copied", Some(id))?;
            tx.commit().map_err(|_| AppError::Storage)
        })
    }
    pub fn dashboard(&self) -> AppResult<Dashboard> {
        self.vault.with_database(false, |conn, key, _, _| {
            let accounts = load_accounts(conn, key)?;
            let categories = load_categories(conn, key)?;
            let events = load_events(conn, key, 10000)?;
            let day = Utc::now().date_naive().to_string();
            let mut recent = vec![];
            for event in &events {
                if event.event_type == "code_copied" {
                    if let Some(a) = event
                        .account_id
                        .as_ref()
                        .and_then(|id| accounts.iter().find(|(a, _)| &a.id == id))
                    {
                        if !recent.iter().any(|v: &AccountView| v.id == a.0.id) {
                            recent.push(a.0.view());
                            if recent.len() == 5 {
                                break;
                            }
                        }
                    }
                }
            }
            Ok(Dashboard {
                account_count: accounts.len(),
                category_count: categories.len(),
                favorite_count: accounts.iter().filter(|(a, _)| a.favorite).count(),
                copies_today: events
                    .iter()
                    .filter(|e| e.event_type == "code_copied" && e.created_at.starts_with(&day))
                    .count(),
                recent_accounts: recent,
            })
        })
    }
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339()
}
pub(crate) fn encrypt_json<T: serde::Serialize>(
    key: &[u8; 32],
    table: &str,
    id: &str,
    column: &str,
    value: &T,
) -> AppResult<Vec<u8>> {
    let bytes = Zeroizing::new(serde_json::to_vec(value).map_err(|_| AppError::InvalidInput)?);
    encrypt_record(key, &record_aad(table, id, column), &bytes)
}
pub(crate) fn load_accounts(
    conn: &Connection,
    key: &[u8; 32],
) -> AppResult<Vec<(AccountData, Zeroizing<Vec<u8>>)>> {
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
    rows.map(|row| {
        let (id, details, secret) = row.map_err(|_| AppError::Storage)?;
        let plain = decrypt_record(key, &record_aad("accounts", &id, "details_blob"), &details)?;
        let data: AccountData =
            serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)?;
        if data.id != id {
            return Err(AppError::InvalidInput);
        }
        let secret = decrypt_record(key, &record_aad("accounts", &id, "secret_blob"), &secret)?;
        data.config(&secret)?.validate()?;
        Ok((data, secret))
    })
    .collect()
}
pub(crate) fn load_account(
    conn: &Connection,
    key: &[u8; 32],
    id: &str,
) -> AppResult<(AccountData, Zeroizing<Vec<u8>>)> {
    let (details, secret): (Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT details_blob,secret_blob FROM accounts WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| AppError::Storage)?
        .ok_or(AppError::NotFound)?;
    let plain = decrypt_record(key, &record_aad("accounts", id, "details_blob"), &details)?;
    let data: AccountData = serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)?;
    if data.id != id {
        return Err(AppError::InvalidInput);
    }
    Ok((
        data,
        decrypt_record(key, &record_aad("accounts", id, "secret_blob"), &secret)?,
    ))
}
pub(crate) fn save_account(
    conn: &Connection,
    key: &[u8; 32],
    data: &AccountData,
    secret: &[u8],
) -> AppResult<()> {
    let secret = encrypt_record(
        key,
        &record_aad("accounts", &data.id, "secret_blob"),
        secret,
    )?;
    let details = encrypt_json(key, "accounts", &data.id, "details_blob", data)?;
    conn.execute("INSERT INTO accounts(id,secret_blob,details_blob,created_at,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET secret_blob=excluded.secret_blob,details_blob=excluded.details_blob,updated_at=excluded.updated_at",params![data.id,secret,details,data.created_at,data.updated_at]).map_err(|_|AppError::Storage)?;
    Ok(())
}
pub(crate) fn ensure_unique(
    conn: &Connection,
    key: &[u8; 32],
    data: &AccountData,
    secret: &[u8],
    exclude: Option<&str>,
) -> AppResult<()> {
    if load_accounts(conn, key)?.iter().any(|(a, s)| {
        Some(a.id.as_str()) != exclude
            && a.issuer.to_lowercase() == data.issuer.to_lowercase()
            && a.account_name.to_lowercase() == data.account_name.to_lowercase()
            && s.as_slice() == secret
    }) {
        return Err(AppError::Conflict);
    }
    Ok(())
}
pub(crate) fn insert_account(
    conn: &Connection,
    key: &[u8; 32],
    input: &AccountInput,
) -> AppResult<AccountView> {
    input.validate(true)?;
    let secret = decode_base32(input.secret.as_deref().ok_or(AppError::InvalidInput)?)?;
    let order = load_accounts(conn, key)?
        .iter()
        .map(|(a, _)| a.sort_order)
        .max()
        .unwrap_or(-1)
        .saturating_add(1);
    let data = data_from_input(input, Uuid::new_v4().to_string(), order, now());
    validate_category(conn, key, &data.category_id)?;
    ensure_unique(conn, key, &data, &secret, None)?;
    save_account(conn, key, &data, &secret)?;
    Ok(data.view())
}
fn data_from_input(
    i: &AccountInput,
    id: String,
    sort_order: i64,
    created_at: String,
) -> AccountData {
    AccountData {
        id,
        issuer: i.issuer.trim().into(),
        account_name: i.account_name.trim().into(),
        algorithm: i.algorithm.clone(),
        digits: i.digits,
        kind: i.kind.clone(),
        period_secs: i.period_secs,
        t0: i.t0.clone(),
        hotp_counter: i.hotp_counter.clone(),
        category_id: i.category_id.clone(),
        tags: i.tags.clone(),
        notes: i.notes.clone(),
        favorite: i.favorite,
        sort_order,
        created_at,
        updated_at: now(),
    }
}
fn code_view(a: &AccountData, s: &[u8], unix: i64) -> AppResult<CodeView> {
    let code = generate_code(&a.config(s)?, unix)?;
    let (remaining_secs, progress) = if a.kind == "totp" {
        let state = time_state(unix, a.period_secs, decimal_i64(&a.t0)?)?;
        (state.remaining_secs, state.progress)
    } else {
        (0, 0.)
    };
    Ok(CodeView {
        account_id: a.id.clone(),
        code,
        remaining_secs,
        progress,
        generated_at: unix,
        clock_offset_secs: 0,
        wall_clock_jump: false,
    })
}
pub(crate) fn load_categories(conn: &Connection, key: &[u8; 32]) -> AppResult<Vec<CategoryView>> {
    let mut stmt = conn
        .prepare("SELECT id,details_blob FROM categories ORDER BY created_at,id")
        .map_err(|_| AppError::Storage)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|_| AppError::Storage)?;
    rows.map(|r| {
        let (id, blob) = r.map_err(|_| AppError::Storage)?;
        let plain = decrypt_record(key, &record_aad("categories", &id, "details_blob"), &blob)?;
        let category: CategoryView =
            serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)?;
        if category.id != id {
            return Err(AppError::InvalidInput);
        }
        Ok(category)
    })
    .collect()
}
fn validate_category(
    conn: &Connection,
    key: &[u8; 32],
    category: &Option<String>,
) -> AppResult<()> {
    if let Some(id) = category {
        if !load_categories(conn, key)?.iter().any(|c| &c.id == id) {
            return Err(AppError::NotFound);
        }
    }
    Ok(())
}
pub(crate) fn load_settings(conn: &Connection, key: &[u8; 32]) -> AppResult<AppSettings> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value_blob FROM settings WHERE key='preferences'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| AppError::Storage)?;
    match blob {
        None => Ok(AppSettings::default()),
        Some(blob) => {
            let plain = decrypt_record(
                key,
                &record_aad("settings", "preferences", "value_blob"),
                &blob,
            )?;
            let settings: AppSettings =
                serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)?;
            settings.validate()?;
            Ok(settings)
        }
    }
}
pub(crate) fn audit(
    conn: &Connection,
    key: &[u8; 32],
    event: &str,
    account: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_events(event_type,details_blob,created_at) VALUES(?1,X'',?2)",
        params![event, now()],
    )
    .map_err(|_| AppError::Storage)?;
    let id = conn.last_insert_rowid();
    let blob = encrypt_json(
        key,
        "audit_events",
        &id.to_string(),
        "details_blob",
        &account,
    )?;
    conn.execute(
        "UPDATE audit_events SET details_blob=?1 WHERE id=?2",
        params![blob, id],
    )
    .map_err(|_| AppError::Storage)?;
    Ok(())
}
fn load_events(conn: &Connection, key: &[u8; 32], limit: usize) -> AppResult<Vec<AuditEvent>> {
    let mut stmt=conn.prepare("SELECT id,event_type,details_blob,created_at FROM audit_events ORDER BY id DESC LIMIT ?1").map_err(|_|AppError::Storage)?;
    let rows = stmt
        .query_map([limit.min(10000) as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| AppError::Storage)?;
    rows.map(|r| {
        let (id, event_type, blob, created_at) = r.map_err(|_| AppError::Storage)?;
        let plain = decrypt_record(
            key,
            &record_aad("audit_events", &id.to_string(), "details_blob"),
            &blob,
        )?;
        let account_id: Option<String> =
            serde_json::from_slice(&plain).map_err(|_| AppError::InvalidInput)?;
        Ok(AuditEvent {
            id,
            event_type,
            account_id,
            created_at,
        })
    })
    .collect()
}
fn validate_ids(ids: &[String]) -> AppResult<()> {
    if ids.len() > 10000
        || ids
            .iter()
            .enumerate()
            .any(|(i, id)| id.is_empty() || ids[..i].contains(id))
    {
        return Err(AppError::InvalidInput);
    }
    Ok(())
}
