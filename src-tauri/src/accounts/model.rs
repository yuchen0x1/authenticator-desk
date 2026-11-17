use crate::{
    AppError, AppResult,
    otp::{OtpAlgorithm, OtpConfig, OtpKind, SecretBytes, decode_base32},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Clone)]
pub struct AccountInput {
    pub issuer: String,
    pub account_name: String,
    pub secret: Option<String>,
    pub algorithm: String,
    pub digits: u8,
    pub kind: String,
    pub period_secs: u32,
    pub t0: String,
    pub hotp_counter: String,
    pub category_id: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
}
fn default_algorithm() -> String {
    "SHA1".into()
}
fn default_digits() -> u8 {
    6
}
fn default_kind() -> String {
    "totp".into()
}
fn default_period() -> u32 {
    30
}
fn default_zero() -> String {
    "0".into()
}
impl Default for AccountInput {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            account_name: String::new(),
            secret: None,
            algorithm: "SHA1".into(),
            digits: 6,
            kind: "totp".into(),
            period_secs: 30,
            t0: "0".into(),
            hotp_counter: "0".into(),
            category_id: None,
            tags: vec![],
            notes: String::new(),
            favorite: false,
        }
    }
}
impl Drop for AccountInput {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.notes.zeroize();
        self.issuer.zeroize();
        self.account_name.zeroize();
        self.tags.zeroize();
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountQuery {
    pub search: String,
    pub category_id: Option<String>,
    pub tag: Option<String>,
    pub favorite_only: bool,
    pub sort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountView {
    pub id: String,
    pub issuer: String,
    pub account_name: String,
    pub algorithm: String,
    pub digits: u8,
    pub kind: String,
    pub period_secs: u32,
    pub t0: String,
    pub hotp_counter: String,
    pub category_id: Option<String>,
    pub tags: Vec<String>,
    pub notes_present: bool,
    pub favorite: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Serialize)]
pub struct AccountDetails {
    pub account: AccountView,
    pub notes: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct CodeView {
    pub account_id: String,
    pub code: String,
    pub remaining_secs: u32,
    pub progress: f64,
    pub generated_at: i64,
    pub clock_offset_secs: i64,
    pub wall_clock_jump: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryView {
    pub id: String,
    pub name: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    pub id: i64,
    pub event_type: String,
    pub account_id: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Dashboard {
    pub account_count: usize,
    pub category_count: usize,
    pub favorite_count: usize,
    pub copies_today: usize,
    pub recent_accounts: Vec<AccountView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    pub locale: String,
    pub theme: String,
    pub idle_timeout_secs: u64,
    pub clipboard_ttl_secs: u32,
    pub hide_codes: bool,
    pub view_mode: String,
    pub animations: bool,
    pub compact: bool,
    pub font_size: u8,
    pub default_algorithm: String,
    pub default_digits: u8,
    pub default_period_secs: u32,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub startup: bool,
    pub backup_enabled: bool,
    pub backup_interval_hours: u32,
    pub backup_retention: u32,
    pub show_next_code: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "zh-CN".into(),
            theme: "dark".into(),
            idle_timeout_secs: 900,
            clipboard_ttl_secs: 30,
            hide_codes: false,
            view_mode: "cards".into(),
            animations: true,
            compact: false,
            font_size: 14,
            default_algorithm: "SHA1".into(),
            default_digits: 6,
            default_period_secs: 30,
            close_to_tray: false,
            minimize_to_tray: false,
            startup: false,
            backup_enabled: false,
            backup_interval_hours: 24,
            backup_retention: 7,
            show_next_code: false,
        }
    }
}
impl AppSettings {
    pub fn validate(&self) -> AppResult<()> {
        if !["zh-CN", "zh-TW", "en", "ja"].contains(&self.locale.as_str())
            || !["dark", "light", "system"].contains(&self.theme.as_str())
            || self.idle_timeout_secs > 86400
            || (self.idle_timeout_secs > 0 && self.idle_timeout_secs < 30)
            || self.clipboard_ttl_secs > 300
            || !["cards", "list"].contains(&self.view_mode.as_str())
            || !(11..=22).contains(&self.font_size)
            || !(6..=8).contains(&self.default_digits)
            || !(1..=86400).contains(&self.default_period_secs)
            || !(1..=8760).contains(&self.backup_interval_hours)
            || !(1..=100).contains(&self.backup_retention)
        {
            return Err(AppError::InvalidParameters);
        }
        algorithm(&self.default_algorithm)?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct AccountData {
    pub id: String,
    pub issuer: String,
    pub account_name: String,
    pub algorithm: String,
    pub digits: u8,
    pub kind: String,
    pub period_secs: u32,
    pub t0: String,
    pub hotp_counter: String,
    pub category_id: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod debug_tests {
    use super::AccountData;

    #[test]
    fn account_debug_output_redacts_plaintext_labels_and_notes() {
        let account = AccountData {
            id: "safe-id".into(),
            issuer: "private-issuer".into(),
            account_name: "private-account".into(),
            algorithm: "SHA1".into(),
            digits: 6,
            kind: "totp".into(),
            period_secs: 30,
            t0: "0".into(),
            hotp_counter: "0".into(),
            category_id: None,
            tags: vec!["private-tag".into()],
            notes: "private-note".into(),
            favorite: false,
            sort_order: 0,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let debug = format!("{account:?}");
        for sensitive in [
            "private-issuer",
            "private-account",
            "private-tag",
            "private-note",
        ] {
            assert!(!debug.contains(sensitive));
        }
        assert!(debug.contains("safe-id"));
    }
}

impl std::fmt::Debug for AccountData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountData")
            .field("id", &self.id)
            .field("algorithm", &self.algorithm)
            .field("digits", &self.digits)
            .field("kind", &self.kind)
            .field("period_secs", &self.period_secs)
            .field("favorite", &self.favorite)
            .field("sort_order", &self.sort_order)
            .finish_non_exhaustive()
    }
}
impl Drop for AccountData {
    fn drop(&mut self) {
        self.notes.zeroize();
        self.issuer.zeroize();
        self.account_name.zeroize();
        self.tags.zeroize();
    }
}
impl AccountData {
    pub fn view(&self) -> AccountView {
        AccountView {
            id: self.id.clone(),
            issuer: self.issuer.clone(),
            account_name: self.account_name.clone(),
            algorithm: self.algorithm.clone(),
            digits: self.digits,
            kind: self.kind.clone(),
            period_secs: self.period_secs,
            t0: self.t0.clone(),
            hotp_counter: self.hotp_counter.clone(),
            category_id: self.category_id.clone(),
            tags: self.tags.clone(),
            notes_present: !self.notes.is_empty(),
            favorite: self.favorite,
            sort_order: self.sort_order,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
    pub fn config(&self, secret: &[u8]) -> AppResult<OtpConfig> {
        Ok(OtpConfig {
            secret: SecretBytes::from_bytes(secret)?,
            algorithm: algorithm(&self.algorithm)?,
            digits: self.digits,
            kind: if self.kind == "hotp" {
                OtpKind::Hotp {
                    counter: decimal_u64(&self.hotp_counter)?,
                }
            } else {
                OtpKind::Totp {
                    period_secs: self.period_secs,
                    t0: decimal_i64(&self.t0)?,
                }
            },
        })
    }
}
pub(crate) fn algorithm(value: &str) -> AppResult<OtpAlgorithm> {
    match value {
        "SHA1" => Ok(OtpAlgorithm::Sha1),
        "SHA256" => Ok(OtpAlgorithm::Sha256),
        "SHA512" => Ok(OtpAlgorithm::Sha512),
        _ => Err(AppError::InvalidParameters),
    }
}
pub(crate) fn decimal_u64(value: &str) -> AppResult<u64> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidParameters);
    }
    value.parse().map_err(|_| AppError::InvalidParameters)
}
pub(crate) fn decimal_i64(value: &str) -> AppResult<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || digits.len() > 19 || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidParameters);
    }
    value.parse().map_err(|_| AppError::InvalidParameters)
}
impl AccountInput {
    pub fn validate(&self, require_secret: bool) -> AppResult<()> {
        if self.issuer.trim().len() > 200
            || self.account_name.trim().is_empty()
            || self.account_name.len() > 320
            || self.notes.len() > 16384
            || self.tags.len() > 32
            || self.tags.iter().any(|t| t.is_empty() || t.len() > 100)
            || !["totp", "hotp"].contains(&self.kind.as_str())
            || !(6..=8).contains(&self.digits)
            || !(1..=86400).contains(&self.period_secs)
        {
            return Err(AppError::InvalidInput);
        }
        algorithm(&self.algorithm)?;
        decimal_u64(&self.hotp_counter)?;
        decimal_i64(&self.t0)?;
        if let Some(secret) = self.secret.as_deref().filter(|s| !s.is_empty()) {
            decode_base32(secret)?;
        } else if require_secret {
            return Err(AppError::InvalidInput);
        }
        Ok(())
    }
}
impl<'de> Deserialize<'de> for AccountInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(default, deny_unknown_fields)]
        struct Wire {
            issuer: String,
            account_name: String,
            secret: Option<String>,
            algorithm: String,
            digits: u8,
            kind: String,
            period_secs: u32,
            t0: String,
            hotp_counter: String,
            category_id: Option<String>,
            tags: Vec<String>,
            notes: String,
            favorite: bool,
        }
        impl Default for Wire {
            fn default() -> Self {
                Self {
                    issuer: String::new(),
                    account_name: String::new(),
                    secret: None,
                    algorithm: default_algorithm(),
                    digits: default_digits(),
                    kind: default_kind(),
                    period_secs: default_period(),
                    t0: default_zero(),
                    hotp_counter: default_zero(),
                    category_id: None,
                    tags: vec![],
                    notes: String::new(),
                    favorite: false,
                }
            }
        }
        let w = Wire::deserialize(deserializer)?;
        Ok(Self {
            issuer: w.issuer,
            account_name: w.account_name,
            secret: w.secret,
            algorithm: w.algorithm,
            digits: w.digits,
            kind: w.kind,
            period_secs: w.period_secs,
            t0: w.t0,
            hotp_counter: w.hotp_counter,
            category_id: w.category_id,
            tags: w.tags,
            notes: w.notes,
            favorite: w.favorite,
        })
    }
}
