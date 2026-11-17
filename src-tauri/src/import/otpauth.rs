use crate::{AppError, AppResult, accounts::AccountInput};
use std::collections::HashMap;
use url::Url;
use zeroize::Zeroizing;

use super::{MigrationItem, MigrationPreview};

/// Reader for plain newline-separated `otpauth://` lists, the shape of an
/// Ente Auth plaintext export and of generic URI dumps.
pub struct OtpAuthListAdapter;

impl OtpAuthListAdapter {
    pub const NAME: &'static str = "otpauth_list";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .next()
            .is_some_and(|line| line.trim_start().starts_with("otpauth://"))
    }

    pub fn preview(&self, bytes: &[u8]) -> AppResult<MigrationPreview> {
        let text = std::str::from_utf8(bytes).map_err(|_| AppError::InvalidInput)?;
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        if lines.is_empty() || lines.len() > 1000 {
            return Err(AppError::InvalidInput);
        }
        let items = lines
            .iter()
            .map(|line| match parse_otpauth(line.trim()) {
                Ok(input) => MigrationItem {
                    issuer: Some(input.issuer.clone()),
                    account_name: Some(input.account_name.clone()),
                    input: Some(input),
                    error_code: None,
                },
                Err(_) => MigrationItem {
                    input: None,
                    issuer: None,
                    account_name: None,
                    error_code: Some("invalid_otpauth".into()),
                },
            })
            .collect();
        Ok(MigrationPreview {
            format: Self::NAME.into(),
            items,
        })
    }
}

impl super::MigrationAdapter for OtpAuthListAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        OtpAuthListAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        OtpAuthListAdapter::preview(self, bytes)
    }
}

pub fn parse_otpauth(uri: &str) -> AppResult<AccountInput> {
    if uri.len() > 16384 || uri.bytes().any(|b| b.is_ascii_control()) {
        return Err(AppError::InvalidInput);
    }
    // Url accepts malformed percent escapes; reject them before decoding labels.
    let bytes = uri.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return Err(AppError::InvalidInput);
        }
    }
    let url = Url::parse(uri).map_err(|_| AppError::InvalidInput)?;
    let kind = url.host_str().ok_or(AppError::InvalidInput)?;
    if url.scheme() != "otpauth"
        || !["totp", "hotp"].contains(&kind)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::InvalidInput);
    }
    let path = url.path().strip_prefix('/').ok_or(AppError::InvalidInput)?;
    if path.is_empty() || path.contains('/') {
        return Err(AppError::InvalidInput);
    }
    let label = decode_label(path)?;
    let mut pairs = HashMap::new();
    for (key, value) in url.query_pairs() {
        if ![
            "secret",
            "issuer",
            "algorithm",
            "digits",
            "period",
            "counter",
            "t0",
        ]
        .contains(&key.as_ref())
            || pairs
                .insert(key.into_owned(), Zeroizing::new(value.into_owned()))
                .is_some()
        {
            return Err(AppError::InvalidInput);
        }
    }
    let (label_issuer, account) = label
        .split_once(':')
        .map(|(a, b)| (a.trim(), b.trim()))
        .unwrap_or(("", label.trim()));
    let issuer = pairs
        .get("issuer")
        .map(|v| v.trim())
        .unwrap_or(label_issuer);
    if !label_issuer.is_empty() && !issuer.is_empty() && label_issuer != issuer {
        return Err(AppError::InvalidInput);
    }
    if kind == "hotp" && pairs.contains_key("period")
        || kind == "totp" && pairs.contains_key("counter")
    {
        return Err(AppError::InvalidInput);
    }
    let mut input = AccountInput::default();
    input.issuer = issuer.into();
    input.account_name = account.into();
    input.kind = kind.into();
    input.secret = Some(
        pairs
            .get("secret")
            .ok_or(AppError::InvalidInput)?
            .as_str()
            .into(),
    );
    input.algorithm = pairs
        .get("algorithm")
        .map(|v| v.to_string())
        .unwrap_or_else(|| "SHA1".into());
    if let Some(value) = pairs.get("digits") {
        input.digits = value.parse().map_err(|_| AppError::InvalidInput)?;
    }
    if let Some(value) = pairs.get("period") {
        input.period_secs = value.parse().map_err(|_| AppError::InvalidInput)?;
    }
    if let Some(value) = pairs.get("t0") {
        input.t0 = value.to_string();
    }
    if kind == "hotp" {
        input.hotp_counter = pairs
            .get("counter")
            .ok_or(AppError::InvalidInput)?
            .to_string();
    }
    input.validate(true)?;
    Ok(input)
}
fn decode_label(value: &str) -> AppResult<String> {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex =
                std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| AppError::InvalidInput)?;
            decoded.push(u8::from_str_radix(hex, 16).map_err(|_| AppError::InvalidInput)?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let text = String::from_utf8(decoded).map_err(|_| AppError::InvalidInput)?;
    if text.chars().any(char::is_control) {
        return Err(AppError::InvalidInput);
    }
    Ok(text)
}
