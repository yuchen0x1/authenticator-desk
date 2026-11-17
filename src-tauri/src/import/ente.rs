use crate::{AppError, AppResult};

use super::{MigrationPreview, MigrationAdapter, parse_json};

/// Detector for Ente Auth encrypted export JSON.
///
/// Ente's encrypted local backup wraps newline-separated `otpauth://` lines
/// in an Argon2id + XChaCha20-Poly1305 secret stream envelope. This release
/// has no verified vector implementation for that envelope, so recognized
/// encrypted exports are rejected with `UnsupportedEncryptedFormat`; the
/// plaintext URI-list export is handled by the generic otpauth list adapter.
pub struct EnteAdapter;

impl EnteAdapter {
    pub const NAME: &'static str = "ente_encrypted";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        parse_json(bytes)
            .and_then(|value| value.as_object().map(|object| {
                object.contains_key("encryptedData")
                    && object.contains_key("kdfParams")
                    && object.contains_key("encryptionNonce")
            }))
            .unwrap_or(false)
    }

    pub fn preview(&self, _bytes: &[u8]) -> AppResult<MigrationPreview> {
        Err(AppError::UnsupportedEncryptedFormat)
    }
}

impl MigrationAdapter for EnteAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        EnteAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        EnteAdapter::preview(self, bytes)
    }
}
