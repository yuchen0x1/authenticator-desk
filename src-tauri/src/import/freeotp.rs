use crate::{AppError, AppResult};

use super::{MigrationPreview, MigrationAdapter};

/// Detector for FreeOTP backups.
///
/// FreeOTP keeps token material in Android Keystore-bound encrypted
/// preferences; its Java object serialization backup and legacy XML
/// preference dump cannot be read portably without the on-device master
/// key. Every recognized shape is therefore rejected with
/// `UnsupportedEncryptedFormat` and no extraction is attempted.
pub struct FreeOtpAdapter;

const JAVA_SERIALIZATION_MAGIC: [u8; 4] = [0xac, 0xed, 0x00, 0x05];

impl FreeOtpAdapter {
    pub const NAME: &'static str = "freeotp";

    pub fn detect(&self, bytes: &[u8]) -> bool {
        if bytes.starts_with(&JAVA_SERIALIZATION_MAGIC) {
            return true;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        let trimmed = text.trim_start();
        let is_xml = trimmed.starts_with("<?xml") || trimmed.starts_with("<map");
        is_xml && text.contains("<map")
    }

    pub fn preview(&self, _bytes: &[u8]) -> AppResult<MigrationPreview> {
        Err(AppError::UnsupportedEncryptedFormat)
    }
}

impl MigrationAdapter for FreeOtpAdapter {
    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn detect(&self, bytes: &[u8]) -> bool {
        FreeOtpAdapter::detect(self, bytes)
    }
    fn preview(&self, bytes: &[u8], _password: Option<&crate::vault::SecretString>) -> AppResult<MigrationPreview> {
        FreeOtpAdapter::preview(self, bytes)
    }
}
