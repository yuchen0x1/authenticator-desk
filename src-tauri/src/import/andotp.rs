use serde_json::Value;

use crate::{AppError, AppResult, accounts::AccountInput, otp::decode_base32};

use super::{
    MigrationItem, MigrationPreview, MigrationAdapter, json_str, json_u64, parse_json,
};

/// Reader for andOTP plain JSON backups and detector for its `.aes` envelope.
///
/// The legacy plain schema is a JSON array of entries. The encrypted `.aes`
/// envelope (big-endian PBKDF2 iteration count, salt, AES/GCM body) and the
/// OpenPGP `.gpg` export are rejected without decryption; STEAM entries are
/// reported unsupported instead of being silently converted.
pub struct AndOtpAdapter;

const AES_HEADER_BYTES: usize = 48;
const ITERATION_MIN: u32 = 140_000;
const ITERATION_MAX: u32 = 160_000;

fn is_aes_envelope(bytes: &[u8]) -> bool {
    if bytes.len() < AES_HEADER_BYTES {
        return false;
    }
    let iterations = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    (ITERATION_MIN..=ITERATION_MAX).contains(&iterations)
}

fn map_entry(entry: &Value) -> MigrationItem {
    let label = json_str(entry, "label").unwrap_or_default().to_owned();
    let invalid = |error_code: &str| MigrationItem {
        input: None,
        issuer: json_str(entry, "issuer").map(str::to_owned),
        account_name: Some(label.clone()).filter(|account| !account.is_empty()),
        error_code: Some(error_code.into()),
    };
    let entry_type = json_str(entry, "type").unwrap_or_default().to_owned();
    match entry_type.as_str() {
        "TOTP" | "HOTP" => {}
        "STEAM" => return invalid("unsupported_steam"),
        _ => return invalid("unsupported_type"),
    }
    let Some(secret) = json_str(entry, "secret") else {
        return invalid("missing_secret");
    };
    let algorithm = json_str(entry, "algorithm")
        .unwrap_or("SHA1")
        .to_ascii_uppercase();
    if !["SHA1", "SHA256", "SHA512"].contains(&algorithm.as_str()) {
        return invalid("unsupported_algorithm");
    }
    let mut input = AccountInput::default();
    input.kind = entry_type.to_lowercase();
    input.algorithm = algorithm;
    input.digits = json_u64(entry, "digits").unwrap_or(6).min(u8::MAX as u64) as u8;
    input.period_secs = json_u64(entry, "period").unwrap_or(30).min(u32::MAX as u64) as u32;
    if entry_type == "HOTP" {
        input.hotp_counter = json_u64(entry, "counter").unwrap_or(0).to_string();
    }
    if let Some(tags) = entry.get("tags").and_then(Value::as_array) {
        input.tags = tags
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .take(32)
            .collect();
    }
    input.issuer = json_str(entry, "issuer").unwrap_or_default().to_owned();
    input.account_name = label;
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

impl AndOtpAdapter {
    pub const NAME: &'static str = "andotp";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        if is_aes_envelope(bytes) {
            return true;
        }
        parse_json(bytes)
            .and_then(|value| value.as_array().map(|entries| {
                entries
                    .first()
                    .is_some_and(|entry| entry.as_object().is_some_and(|object| object.contains_key("secret")))
            }))
            .unwrap_or(false)
    }

    pub fn preview(&self, bytes: &[u8]) -> AppResult<MigrationPreview> {
        if is_aes_envelope(bytes) {
            return Err(AppError::UnsupportedEncryptedFormat);
        }
        let Some(entries) = parse_json(bytes).and_then(|value| value.as_array().cloned()) else {
            return Err(AppError::InvalidInput);
        };
        if entries.is_empty() || entries.len() > 1000 {
            return Err(AppError::InvalidInput);
        }
        Ok(MigrationPreview {
            format: Self::NAME.into(),
            items: entries.iter().map(map_entry).collect(),
        })
    }
}

impl MigrationAdapter for AndOtpAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        AndOtpAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        AndOtpAdapter::preview(self, bytes)
    }
}
