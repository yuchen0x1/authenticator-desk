use serde_json::Value;

use crate::{AppError, AppResult, accounts::AccountInput, otp::decode_base32};

use super::{
    MigrationItem, MigrationPreview, MigrationAdapter, invalid_item, json_optional_u64, json_str,
    json_u64, parse_json,
};

/// Reader for Aegis plain JSON vaults (and Aegis-shaped deskOTP plain backups).
///
/// Only the documented plaintext schema is read. Encrypted vaults — Aegis
/// `header.slots`/`header.params`, or deskOTP's `data` field — are rejected
/// without decryption because no verified vector implementation exists in
/// this release; Aegis UUIDs, groups, and icons are treated as provenance
/// only and never become local identity.
pub struct AegisAdapter;

fn map_entry(entry: &Value) -> MigrationItem {
    let entry_type = json_str(entry, "type").unwrap_or_default();
    let name = json_str(entry, "name").map(str::to_owned);
    match entry_type {
        "totp" | "hotp" => {}
        "steam" => {
            return MigrationItem {
                input: None,
                issuer: None,
                account_name: name,
                error_code: Some("unsupported_steam".into()),
            };
        }
        _ => return invalid_item("unsupported_type"),
    }
    let Some(info) = entry.get("info") else {
        return MigrationItem {
            input: None,
            issuer: None,
            account_name: name,
            error_code: Some("missing_info".into()),
        };
    };
    let Some(secret) = json_str(info, "secret") else {
        return MigrationItem {
            input: None,
            issuer: None,
            account_name: name,
            error_code: Some("missing_secret".into()),
        };
    };
    let algorithm = json_str(info, "algo").unwrap_or("SHA1").to_owned();
    if !["SHA1", "SHA256", "SHA512"].contains(&algorithm.as_str()) {
        return MigrationItem {
            input: None,
            issuer: None,
            account_name: name,
            error_code: Some("unsupported_algorithm".into()),
        };
    }
    let (Ok(digits), Ok(period_secs)) = (
        json_optional_u64(info.get("digits")),
        json_optional_u64(info.get("period")),
    ) else {
        return invalid_item("invalid_parameters");
    };
    let digits = digits.unwrap_or(6);
    let period_secs = period_secs.unwrap_or(30);
    let kind = entry_type.to_string();
    let mut input = AccountInput::default();
    input.kind = kind.clone();
    input.algorithm = algorithm;
    input.digits = digits.min(u8::MAX as u64) as u8;
    input.period_secs = period_secs.min(u32::MAX as u64) as u32;
    if kind == "hotp" {
        let Ok(counter) = json_optional_u64(info.get("counter")) else {
            return invalid_item("invalid_parameters");
        };
        input.hotp_counter = counter.unwrap_or(0).to_string();
    }
    // Aegis stores issuer and name separately; ':' is account-name data.
    input.issuer = json_str(entry, "issuer").unwrap_or_default().to_owned();
    input.account_name = name.as_deref().unwrap_or_default().trim().to_owned();
    input.secret = Some(secret.to_owned());
    if decode_base32(secret).is_err() || input.validate(true).is_err() {
        return MigrationItem {
            input: None,
            issuer: Some(input.issuer.clone()),
            account_name: Some(input.account_name.clone()),
            error_code: Some("invalid_parameters".into()),
        };
    }
    MigrationItem {
        issuer: Some(input.issuer.clone()),
        account_name: Some(input.account_name.clone()),
        input: Some(input),
        error_code: None,
    }
}

impl AegisAdapter {
    pub const NAME: &'static str = "aegis";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        parse_json(bytes)
            .and_then(|value| value.as_object().map(|object| object.contains_key("db") || (object.contains_key("header") && object.contains_key("data"))))
            .unwrap_or(false)
    }

    pub fn preview(&self, bytes: &[u8]) -> AppResult<MigrationPreview> {
        let value = parse_json(bytes).ok_or(AppError::InvalidInput)?;
        let Some(object) = value.as_object() else {
            return Err(AppError::InvalidInput);
        };
        let encrypted = object
            .get("header")
            .is_some_and(|header| {
                header.get("slots").is_some_and(|slots| !slots.is_null())
                    || header.get("params").is_some_and(|params| !params.is_null())
            })
            || object.contains_key("data");
        if encrypted {
            return Err(AppError::UnsupportedEncryptedFormat);
        }
        let Some(db) = object.get("db") else {
            return Err(AppError::InvalidInput);
        };
        let version = json_u64(db, "version").unwrap_or(1);
        if !(1..=3).contains(&version) {
            return Err(AppError::InvalidInput);
        }
        let entries = db
            .get("entries")
            .and_then(Value::as_array)
            .ok_or(AppError::InvalidInput)?;
        if entries.is_empty() || entries.len() > 1000 {
            return Err(AppError::InvalidInput);
        }
        Ok(MigrationPreview {
            format: Self::NAME.into(),
            items: entries.iter().map(map_entry).collect(),
        })
    }
}

impl MigrationAdapter for AegisAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        AegisAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        AegisAdapter::preview(self, bytes)
    }
}
