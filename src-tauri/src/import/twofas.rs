use serde_json::Value;

use crate::{AppError, AppResult, accounts::AccountInput, otp::decode_base32};

use super::{
    MigrationItem, MigrationPreview, MigrationAdapter, json_str, json_u64, parse_json,
};

/// Reader for 2FAS plaintext backup JSON (schema versions 1 through 4).
///
/// The nested `otp` object is the only source of OTP parameters; the outer
/// service `type` field is never read as an OTP type. Exports with
/// `servicesEncrypted` or `reference` payloads are rejected without
/// decryption, and group/order/icon metadata is ignored.
pub struct TwoFasAdapter;

fn json_nullable_u64(value: Option<&Value>) -> Result<Option<u64>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().ok_or(()).map(Some),
    }
}

fn map_service(service: &Value) -> MigrationItem {
    let name = json_str(service, "name").unwrap_or_default().to_owned();
    let otp = service.get("otp");
    let token_type = otp
        .and_then(|otp| json_str(otp, "tokenType"))
        .unwrap_or("TOTP")
        .to_owned();
    let invalid = |error_code: &str| MigrationItem {
        input: None,
        issuer: json_str(service, "name").map(str::to_owned),
        account_name: otp.and_then(|otp| json_str(otp, "account")).map(str::to_owned),
        error_code: Some(error_code.into()),
    };
    match token_type.as_str() {
        "TOTP" | "HOTP" => {}
        _ => return invalid("unsupported_token_type"),
    }
    let Some(secret) = json_str(service, "secret") else {
        return invalid("missing_secret");
    };
    let algorithm = otp
        .and_then(|otp| json_str(otp, "algorithm"))
        .unwrap_or("SHA1")
        .to_owned();
    if !["SHA1", "SHA256", "SHA512"].contains(&algorithm.as_str()) {
        return invalid("unsupported_algorithm");
    }
    let (Ok(digits), Ok(period_secs)) = (
        json_nullable_u64(otp.and_then(|otp| otp.get("digits"))),
        json_nullable_u64(otp.and_then(|otp| otp.get("period"))),
    ) else {
        return invalid("invalid_parameters");
    };
    let digits = digits.unwrap_or(6);
    let period_secs = period_secs.unwrap_or(30);
    let account = otp
        .and_then(|otp| json_str(otp, "account"))
        .filter(|account| !account.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            otp.and_then(|otp| json_str(otp, "label"))
                .and_then(|label| label.split_once(':').map(|(_, rest)| rest.trim().to_owned()))
        })
        .unwrap_or_else(|| name.clone());
    let issuer = otp
        .and_then(|otp| json_str(otp, "issuer"))
        .filter(|issuer| !issuer.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| name.clone());
    let mut input = AccountInput::default();
    input.kind = token_type.to_lowercase();
    input.algorithm = algorithm;
    input.digits = digits.min(u8::MAX as u64) as u8;
    input.period_secs = period_secs.min(u32::MAX as u64) as u32;
    if token_type == "HOTP" {
        let counter = match json_nullable_u64(otp.and_then(|otp| otp.get("counter"))) {
            Ok(Some(counter)) => counter,
            Ok(None) => return invalid("missing_counter"),
            Err(()) => return invalid("invalid_parameters"),
        };
        input.hotp_counter = counter.to_string();
    }
    input.issuer = issuer;
    input.account_name = account;
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

impl TwoFasAdapter {
    pub const NAME: &'static str = "twofas";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        parse_json(bytes)
            .and_then(|value| value.as_object().map(|object| object.contains_key("services")))
            .unwrap_or(false)
    }

    pub fn preview(&self, bytes: &[u8]) -> AppResult<MigrationPreview> {
        let value = parse_json(bytes).ok_or(AppError::InvalidInput)?;
        let Some(object) = value.as_object() else {
            return Err(AppError::InvalidInput);
        };
        let encrypted = object.contains_key("servicesEncrypted")
            || object
                .get("reference")
                .is_some_and(|reference| !reference.is_null());
        if encrypted {
            return Err(AppError::UnsupportedEncryptedFormat);
        }
        let schema_version = json_u64(&value, "schemaVersion").unwrap_or(1);
        if !(1..=4).contains(&schema_version) {
            return Err(AppError::InvalidInput);
        }
        let services = object
            .get("services")
            .and_then(Value::as_array)
            .ok_or(AppError::InvalidInput)?;
        if services.is_empty() || services.len() > 1000 {
            return Err(AppError::InvalidInput);
        }
        Ok(MigrationPreview {
            format: Self::NAME.into(),
            items: services.iter().map(map_service).collect(),
        })
    }
}

impl MigrationAdapter for TwoFasAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        TwoFasAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        TwoFasAdapter::preview(self, bytes)
    }
}
