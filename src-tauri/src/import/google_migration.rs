use zeroize::Zeroizing;

use crate::{AppError, AppResult, accounts::AccountInput, otp::encode_base32};

use super::{MAX_IMPORT_ITEMS, MigrationItem, MigrationPreview, base64::decode_base64};

const URI_PREFIX: &str = "otpauth-migration://offline?";
const MAX_URIS: usize = 1000;
const MAX_PARAMETERS: usize = 1000;
const MAX_PAYLOAD_BYTES: usize = 65536;
const MAX_SECRET_BYTES: usize = 128;
const MAX_NAME_BYTES: usize = 512;
const MAX_ISSUER_BYTES: usize = 200;
const MAX_VARINT_BYTES: usize = 10;

/// Reader for the Google Authenticator migration protobuf payload.
///
/// The decoder is bounded by field, string, and payload limits and skips
/// unknown fields strictly by wire type so malformed or hostile payloads
/// fail closed instead of allocating unbounded buffers.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> AppResult<Self> {
        if data.len() > MAX_PAYLOAD_BYTES {
            return Err(AppError::InvalidInput);
        }
        Ok(Self { data, pos: 0 })
    }
    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }
    fn byte(&mut self) -> AppResult<u8> {
        let byte = *self.data.get(self.pos).ok_or(AppError::InvalidInput)?;
        self.pos += 1;
        Ok(byte)
    }
    fn varint(&mut self) -> AppResult<u64> {
        let mut value = 0u64;
        for shift in 0..MAX_VARINT_BYTES {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << (7 * shift);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(AppError::InvalidInput)
    }
    fn len_delimited(&mut self) -> AppResult<&'a [u8]> {
        let len = self.varint()?;
        let end = self
            .pos
            .checked_add(len as usize)
            .ok_or(AppError::InvalidInput)?;
        if end > self.data.len() {
            return Err(AppError::InvalidInput);
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
    fn skip(&mut self, wire_type: u8) -> AppResult<()> {
        match wire_type {
            0 => {
                self.varint()?;
            }
            1 => {
                self.pos = self.pos.checked_add(8).ok_or(AppError::InvalidInput)?;
                if self.pos > self.data.len() {
                    return Err(AppError::InvalidInput);
                }
            }
            2 => {
                self.len_delimited()?;
            }
            5 => {
                self.pos = self.pos.checked_add(4).ok_or(AppError::InvalidInput)?;
                if self.pos > self.data.len() {
                    return Err(AppError::InvalidInput);
                }
            }
            _ => return Err(AppError::InvalidInput),
        }
        Ok(())
    }
}

fn decode_uri_field<'a>(uri: &'a str, key: &str) -> AppResult<&'a str> {
    let query = uri.strip_prefix(URI_PREFIX).ok_or(AppError::InvalidInput)?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix(key)
            && value.starts_with('=')
        {
            return Ok(&value[1..]);
        }
    }
    Err(AppError::InvalidInput)
}

fn percent_decode(value: &str) -> AppResult<Zeroizing<Vec<u8>>> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .and_then(|pair| std::str::from_utf8(pair).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                .ok_or(AppError::InvalidInput)?;
            decoded.push(hex);
            index += 3;
        } else {
            if bytes[index] == b'&' || bytes[index].is_ascii_control() {
                return Err(AppError::InvalidInput);
            }
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    Ok(Zeroizing::new(decoded))
}

fn utf8_field(bytes: &[u8], max: usize) -> AppResult<String> {
    if bytes.len() > max {
        return Err(AppError::InvalidInput);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| AppError::InvalidInput)
}

struct OtpParameters {
    secret: Zeroizing<Vec<u8>>,
    name: Option<String>,
    issuer: Option<String>,
    algorithm: Option<&'static str>,
    digits: Option<u8>,
    kind: Option<&'static str>,
    counter: u64,
}

fn parse_otp_parameters(bytes: &[u8]) -> AppResult<OtpParameters> {
    let mut reader = Reader::new(bytes)?;
    let mut parameters = OtpParameters {
        secret: Zeroizing::new(Vec::new()),
        name: None,
        issuer: None,
        algorithm: None,
        digits: None,
        kind: None,
        counter: 0,
    };
    while !reader.done() {
        let tag = reader.varint()?;
        let (field, wire_type) = (u32::try_from(tag >> 3).unwrap_or(u32::MAX), (tag & 7) as u8);
        match (field, wire_type) {
            (1, 2) => {
                let secret = reader.len_delimited()?;
                if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
                    return Err(AppError::InvalidInput);
                }
                parameters.secret = Zeroizing::new(secret.to_vec());
            }
            (2, 2) => parameters.name = Some(utf8_field(reader.len_delimited()?, MAX_NAME_BYTES)?),
            (3, 2) => {
                parameters.issuer = Some(utf8_field(reader.len_delimited()?, MAX_ISSUER_BYTES)?)
            }
            (4, 0) => {
                parameters.algorithm = match reader.varint()? {
                    1 => Some("SHA1"),
                    2 => Some("SHA256"),
                    3 => Some("SHA512"),
                    _ => None,
                };
            }
            (5, 0) => {
                parameters.digits = match reader.varint()? {
                    1 => Some(6),
                    2 => Some(8),
                    _ => None,
                };
            }
            (6, 0) => {
                parameters.kind = match reader.varint()? {
                    1 => Some("hotp"),
                    2 => Some("totp"),
                    _ => None,
                };
            }
            (7, 0) => parameters.counter = reader.varint()?,
            (_, 3 | 4) => return Err(AppError::InvalidInput),
            _ => reader.skip(wire_type)?,
        }
    }
    if parameters.secret.is_empty() {
        return Err(AppError::InvalidInput);
    }
    Ok(parameters)
}

fn build_item(parameters: OtpParameters) -> MigrationItem {
    let invalid = |error_code: &str, name: Option<&str>| MigrationItem {
        input: None,
        issuer: name
            .map(|name| name.split(':').next().unwrap_or("").trim().to_owned())
            .filter(|issuer| !issuer.is_empty()),
        account_name: None,
        error_code: Some(error_code.into()),
    };
    let (algorithm, digits, kind) = match (parameters.algorithm, parameters.digits, parameters.kind)
    {
        (Some(algorithm), Some(digits), Some(kind)) => (algorithm, digits, kind),
        (None, _, _) => return invalid("unsupported_algorithm", parameters.name.as_deref()),
        (_, None, _) => return invalid("unsupported_digits", parameters.name.as_deref()),
        (_, _, None) => return invalid("unsupported_type", parameters.name.as_deref()),
    };
    let label = parameters.name.as_deref().unwrap_or("");
    let (label_issuer, account) = label
        .split_once(':')
        .map(|(issuer, rest)| (issuer.trim(), rest.trim()))
        .unwrap_or(("", label.trim()));
    if account.is_empty() {
        return invalid("missing_account", parameters.name.as_deref());
    }
    let issuer = match parameters.issuer.as_deref().map(str::trim) {
        Some(issuer) if !issuer.is_empty() => issuer,
        _ => label_issuer,
    };
    let mut input = AccountInput::default();
    input.issuer = issuer.to_owned();
    input.account_name = account.to_owned();
    input.kind = kind.into();
    input.secret = Some((*encode_base32(&parameters.secret)).clone());
    input.algorithm = algorithm.into();
    input.digits = digits;
    if kind == "hotp" {
        input.hotp_counter = parameters.counter.to_string();
    }
    if input.validate(true).is_err() {
        return invalid("invalid_otp_parameters", parameters.name.as_deref());
    }
    MigrationItem {
        issuer: Some(input.issuer.clone()),
        account_name: Some(input.account_name.clone()),
        input: Some(input),
        error_code: None,
    }
}

fn parse_payload(bytes: &[u8]) -> AppResult<Vec<MigrationItem>> {
    let mut reader = Reader::new(bytes)?;
    let mut items = Vec::new();
    let mut version: Option<u64> = None;
    while !reader.done() {
        let tag = reader.varint()?;
        let (field, wire_type) = (u32::try_from(tag >> 3).unwrap_or(u32::MAX), (tag & 7) as u8);
        match (field, wire_type) {
            (1, 2) => {
                let parameters = parse_otp_parameters(reader.len_delimited()?)?;
                items.push(build_item(parameters));
                if items.len() > MAX_PARAMETERS {
                    return Err(AppError::InvalidInput);
                }
            }
            (2, 0) => {
                if version.replace(reader.varint()?).is_some() {
                    return Err(AppError::InvalidInput);
                }
            }
            (_, 3 | 4) => return Err(AppError::InvalidInput),
            _ => reader.skip(wire_type)?,
        }
    }
    if version.is_some_and(|version| version != 1) {
        return Err(AppError::InvalidInput);
    }
    Ok(items)
}

fn decode_uri(uri: &str) -> AppResult<Vec<MigrationItem>> {
    let uri = uri.trim();
    if !uri.starts_with(URI_PREFIX)
        || uri.len() > MAX_PAYLOAD_BYTES + URI_PREFIX.len() + 64
        || uri.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(AppError::InvalidInput);
    }
    let data = decode_uri_field(uri, "data")?;
    let payload = percent_decode(data)?;
    let payload = String::from_utf8(payload.to_vec()).map_err(|_| AppError::InvalidInput)?;
    let bytes = decode_base64(payload.trim())?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidInput);
    }
    parse_payload(&bytes)
}

pub struct GoogleMigrationAdapter;

impl GoogleMigrationAdapter {
    pub const NAME: &'static str = "google_migration";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        text.split_whitespace()
            .any(|line| line.starts_with("otpauth-migration://"))
    }

    pub fn preview(&self, bytes: &[u8]) -> AppResult<MigrationPreview> {
        let text = std::str::from_utf8(bytes).map_err(|_| AppError::InvalidInput)?;
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        if lines.is_empty() || lines.len() > MAX_URIS {
            return Err(AppError::InvalidInput);
        }
        let mut items = Vec::new();
        for line in &lines {
            items.append(&mut decode_uri(line)?);
            // A single URI is bounded by MAX_PARAMETERS, but a multi-URI
            // migration must also respect the import-wide aggregate bound
            // before any vault classification work begins.
            if items.len() > MAX_IMPORT_ITEMS {
                return Err(AppError::InvalidInput);
            }
        }
        if items.is_empty() {
            return Err(AppError::InvalidInput);
        }
        Ok(MigrationPreview {
            format: Self::NAME.into(),
            items,
        })
    }
}

impl super::MigrationAdapter for GoogleMigrationAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        GoogleMigrationAdapter::detect(self, bytes)
    }
    fn preview(
        &self,
        bytes: &[u8],
        _password: Option<&crate::vault::SecretString>,
    ) -> AppResult<MigrationPreview> {
        GoogleMigrationAdapter::preview(self, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{GoogleMigrationAdapter, decode_uri, parse_otp_parameters};
    use crate::AppError;
    use zeroize::Zeroizing;

    fn varint(value: u64) -> Vec<u8> {
        let mut out = vec![];
        let mut value = value;
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
        out
    }

    fn field_header(field: u32, wire_type: u8) -> Vec<u8> {
        varint(((u64::from(field)) << 3) | u64::from(wire_type))
    }

    #[test]
    fn parses_single_parameter_with_issuer_and_algorithm() {
        let mut parameters = vec![];
        parameters.extend(field_header(1, 2));
        parameters.extend(varint(3));
        parameters.extend_from_slice(b"\x2a\x48\x00");
        parameters.extend(field_header(2, 2));
        parameters.extend(varint(22));
        parameters.extend_from_slice(b"Demo:alice@example.com");
        parameters.extend(field_header(3, 2));
        parameters.extend(varint(4));
        parameters.extend_from_slice(b"Demo");
        parameters.extend(field_header(4, 0));
        parameters.extend(varint(2));
        parameters.extend(field_header(5, 0));
        parameters.extend(varint(1));
        parameters.extend(field_header(6, 0));
        parameters.extend(varint(2));

        let parsed = parse_otp_parameters(&parameters).unwrap();
        assert_eq!(parsed.algorithm, Some("SHA256"));
        assert_eq!(parsed.digits, Some(6));
        assert_eq!(parsed.kind, Some("totp"));
    }

    #[test]
    fn rejects_unknown_wire_types_and_groups() {
        assert!(matches!(
            parse_otp_parameters(&[0x0b, 0x01]),
            Err(AppError::InvalidInput)
        ));
        assert!(matches!(
            parse_otp_parameters(&[0x33, 0x01, 0x34]),
            Err(AppError::InvalidInput)
        ));
    }

    #[test]
    fn decodes_real_uri_shape_with_base64_transport() {
        // data encodes: version=1, one TOTP parameter with secret 0x2a4800.
        let mut inner = vec![];
        inner.extend(field_header(1, 2));
        inner.extend(varint(3));
        inner.extend_from_slice(b"*H ");
        inner.extend(field_header(2, 2));
        inner.extend(varint(6));
        inner.extend_from_slice(b"Demo:a");
        inner.extend(field_header(4, 0));
        inner.extend(varint(2));
        inner.extend(field_header(5, 0));
        inner.extend(varint(1));
        inner.extend(field_header(6, 0));
        inner.extend(varint(2));
        let mut payload = vec![];
        payload.extend(field_header(1, 2));
        payload.extend(varint(inner.len() as u64));
        payload.extend_from_slice(&inner);
        payload.extend(field_header(2, 0));
        payload.extend(varint(1));
        payload.extend(field_header(3, 0));
        payload.extend(varint(1));
        payload.extend(field_header(4, 0));
        payload.extend(varint(0));
        let encoded = {
            const ALPHABET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut out = String::new();
            let mut buffer = 0u32;
            let mut bits = 0;
            for byte in &payload {
                buffer = (buffer << 8) | u32::from(*byte);
                bits += 8;
                while bits >= 6 {
                    bits -= 6;
                    out.push(ALPHABET[((buffer >> bits) & 0x3f) as usize] as char);
                    buffer &= (1u32 << bits).saturating_sub(1);
                }
            }
            if bits > 0 {
                out.push(ALPHABET[((buffer << (6 - bits)) & 0x3f) as usize] as char);
            }
            out
        };
        let uri = format!("otpauth-migration://offline?data={encoded}");
        let items = decode_uri(&uri).unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.issuer.as_deref(), Some("Demo"));
        assert_eq!(item.account_name.as_deref(), Some("a"));
        let input = item.input.as_ref().unwrap();
        assert_eq!(input.kind, "totp");
        assert_eq!(input.algorithm, "SHA256");
        assert_eq!(input.digits, 6);
    }

    #[test]
    fn detect_matches_migration_uris_only() {
        let adapter = GoogleMigrationAdapter;
        assert!(adapter.detect(b"otpauth-migration://offline?data=abc"));
        assert!(!adapter.detect(b"otpauth://totp/Demo"));
        assert!(!adapter.detect(&[0xff, 0xfe]));
    }

    #[test]
    fn utf8_fields_reject_oversized_names() {
        let mut parameters = vec![];
        parameters.extend(field_header(1, 2));
        parameters.extend(varint(1));
        parameters.extend_from_slice(b"s");
        parameters.extend(field_header(2, 2));
        parameters.extend(varint(600));
        parameters.extend(vec![b'a'; 600]);
        assert!(matches!(
            parse_otp_parameters(&parameters),
            Err(AppError::InvalidInput)
        ));
        let _ = Zeroizing::new(Vec::<u8>::new());
    }
}
