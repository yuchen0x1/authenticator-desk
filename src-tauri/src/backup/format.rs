use std::{
    convert::TryInto,
    fs,
    io::{Read, Write},
    path::Path,
};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use rand::{RngExt, rng};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    AppError, AppResult,
    accounts::{AccountData, AppSettings, CategoryView},
    vault::{KdfHeader, SecretString},
};

pub const MAGIC: &[u8; 8] = b"AUTHDESK";
pub const FORMAT_VERSION: u16 = 1;
pub const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_ACCOUNTS: usize = 10_000;
pub const MAX_CATEGORIES: usize = 1_000;
pub const MAX_TAGS: usize = 32;
pub const MAX_TAG_CHARS: usize = 100;
pub const MAX_CATEGORY_NAME_CHARS: usize = 100;
pub const MAX_ISSUER_CHARS: usize = 200;
pub const MAX_ACCOUNT_NAME_CHARS: usize = 320;
pub const MAX_NOTES_CHARS: usize = 16_384;
const NONCE_LEN: usize = 12;
const FIXED_HEADER_LEN: usize = 8 + 2 + 4 + NONCE_LEN + 8;

/// Rejects payloads that would exceed the decoder's structural bounds.
///
/// This check runs before key derivation, plaintext encoding, and any file
/// write, so an encoder can never produce a document its own decoder (or the
/// restore validator) would refuse.
pub(crate) fn check_counts(payload: &BackupPayload) -> AppResult<()> {
    if payload.accounts.len() > MAX_ACCOUNTS || payload.categories.len() > MAX_CATEGORIES {
        return Err(AppError::InvalidInput);
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupSummary {
    pub path: String,
    pub account_count: usize,
    pub category_count: usize,
    pub bytes: u64,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupAccount {
    pub data: AccountData,
    pub secret: Vec<u8>,
}
impl Drop for BackupAccount {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupPayload {
    pub accounts: Vec<BackupAccount>,
    pub categories: Vec<CategoryView>,
    pub settings: AppSettings,
    pub metadata: BackupMetadata,
    /// Stable vault identity. Only authentic documents (verified AEAD) may
    /// claim an identity; legacy documents decode with `None` and must be
    /// treated as unowned by destructive retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<String>,
}
impl Drop for BackupPayload {
    fn drop(&mut self) {
        for category in &mut self.categories {
            category.name.zeroize();
            category.color.zeroize();
        }
        self.settings.locale.zeroize();
        self.settings.theme.zeroize();
        self.settings.view_mode.zeroize();
        self.settings.default_algorithm.zeroize();
        self.metadata.application.zeroize();
        self.metadata.created_at.zeroize();
        if let Some(id) = self.vault_id.as_mut() {
            id.zeroize();
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupMetadata {
    pub application: String,
    pub created_at: String,
}

pub(crate) struct DecodedBackup {
    pub payload: BackupPayload,
    pub summary: BackupSummary,
}

pub(crate) fn encode(payload: &BackupPayload, password: &SecretString) -> AppResult<Vec<u8>> {
    if password.is_empty() {
        return Err(AppError::InvalidInput);
    }
    check_counts(payload)?;
    let plaintext =
        Zeroizing::new(serde_json::to_vec(payload).map_err(|_| AppError::InvalidInput)?);
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidInput);
    }
    let kdf = KdfHeader::new_default()?;
    let key = kdf.derive_key(password)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng().fill(&mut nonce_bytes);
    let encoded_kdf = kdf.encode()?;
    let ciphertext_len = plaintext
        .len()
        .checked_add(16)
        .ok_or(AppError::InvalidInput)?;
    let header_len = FIXED_HEADER_LEN
        .checked_add(encoded_kdf.len())
        .ok_or(AppError::InvalidInput)?;
    let mut header = Vec::with_capacity(header_len);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    header.extend_from_slice(&(encoded_kdf.len() as u32).to_be_bytes());
    header.extend_from_slice(&encoded_kdf);
    header.extend_from_slice(&nonce_bytes);
    header.extend_from_slice(&(ciphertext_len as u64).to_be_bytes());
    let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|_| AppError::Crypto)?;
    let ciphertext = cipher
        .encrypt(
            &Nonce::try_from(&nonce_bytes[..]).map_err(|_| AppError::Crypto)?,
            Payload {
                msg: &plaintext,
                aad: &header,
            },
        )
        .map_err(|_| AppError::Crypto)?;
    if ciphertext.len() != ciphertext_len {
        return Err(AppError::Crypto);
    }
    let mut output = header;
    output.extend_from_slice(&ciphertext);
    if output.len() as u64 > MAX_BACKUP_BYTES {
        return Err(AppError::InvalidInput);
    }
    Ok(output)
}

fn decode_payload(bytes: &[u8]) -> AppResult<BackupPayload> {
    let payload: BackupPayload =
        serde_json::from_slice(bytes).map_err(|_| AppError::InvalidInput)?;
    check_counts(&payload)?;
    Ok(payload)
}

pub(crate) fn decode_file(path: &Path, password: &SecretString) -> AppResult<DecodedBackup> {
    let file = fs::File::open(path).map_err(|_| AppError::NotFound)?;
    let size = file.metadata().map_err(|_| AppError::Storage)?.len();
    if size < FIXED_HEADER_LEN as u64 + 22 + 16 || size > MAX_BACKUP_BYTES {
        return Err(AppError::InvalidInput);
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_BACKUP_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::Storage)?;
    if bytes.len() as u64 != size || &bytes[..8] != MAGIC {
        return Err(AppError::InvalidInput);
    }
    let version = u16::from_be_bytes(
        bytes[8..10]
            .try_into()
            .map_err(|_| AppError::InvalidInput)?,
    );
    if version != FORMAT_VERSION {
        return Err(AppError::InvalidInput);
    }
    let kdf_len = u32::from_be_bytes(
        bytes[10..14]
            .try_into()
            .map_err(|_| AppError::InvalidInput)?,
    ) as usize;
    if !(22..=86).contains(&kdf_len) {
        return Err(AppError::InvalidInput);
    }
    let header_len = FIXED_HEADER_LEN
        .checked_add(kdf_len)
        .ok_or(AppError::InvalidInput)?;
    if header_len > bytes.len() || header_len + 16 > bytes.len() {
        return Err(AppError::InvalidInput);
    }
    let kdf_start = 14;
    let kdf_end = kdf_start + kdf_len;
    let kdf = KdfHeader::decode(&bytes[kdf_start..kdf_end]).map_err(|_| AppError::InvalidInput)?;
    let nonce_end = kdf_end + NONCE_LEN;
    let nonce = &bytes[kdf_end..nonce_end];
    let ciphertext_len = u64::from_be_bytes(
        bytes[nonce_end..nonce_end + 8]
            .try_into()
            .map_err(|_| AppError::InvalidInput)?,
    );
    if ciphertext_len < 16
        || ciphertext_len as usize > MAX_PAYLOAD_BYTES + 16
        || nonce_end + 8 + ciphertext_len as usize != bytes.len()
    {
        return Err(AppError::InvalidInput);
    }
    let key = kdf
        .derive_key(password)
        .map_err(|_| AppError::AuthenticationFailed)?;
    let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|_| AppError::Crypto)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &Nonce::try_from(nonce).map_err(|_| AppError::Crypto)?,
                Payload {
                    msg: &bytes[header_len..],
                    aad: &bytes[..header_len],
                },
            )
            .map_err(|_| AppError::AuthenticationFailed)?,
    );
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidInput);
    }
    let payload = decode_payload(&plaintext)?;
    let summary = BackupSummary {
        path: path.to_string_lossy().into_owned(),
        account_count: payload.accounts.len(),
        category_count: payload.categories.len(),
        bytes: size,
        created_at: payload.metadata.created_at.clone(),
    };
    Ok(DecodedBackup { payload, summary })
}

pub(crate) fn write_new_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if path.extension().and_then(|v| v.to_str()) != Some("authdesk") {
        return Err(AppError::InvalidInput);
    }
    let parent = path.parent().ok_or(AppError::InvalidInput)?;
    fs::create_dir_all(parent).map_err(|_| AppError::Storage)?;
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or(AppError::InvalidInput)?;
    let temp = parent.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| AppError::Storage)?;
        f.write_all(bytes).map_err(|_| AppError::Storage)?;
        f.sync_all().map_err(|_| AppError::Storage)?;
        fs::hard_link(&temp, path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::Conflict
            } else {
                AppError::Storage
            }
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

#[cfg(test)]
mod tests {
    use super::{
        BackupAccount, BackupMetadata, BackupPayload, MAX_ACCOUNTS, MAX_CATEGORIES, check_counts,
        decode_payload, encode,
    };
    use crate::{
        accounts::{AccountData, AppSettings, CategoryView},
        vault::SecretString,
    };

    fn payload(account_count: usize, category_count: usize) -> BackupPayload {
        let accounts = (0..account_count)
            .map(|index| BackupAccount {
                data: AccountData {
                    id: format!("account-{index}"),
                    issuer: "Issuer".into(),
                    account_name: format!("account-{index}"),
                    algorithm: "SHA1".into(),
                    digits: 6,
                    kind: "totp".into(),
                    period_secs: 30,
                    t0: "0".into(),
                    hotp_counter: "0".into(),
                    category_id: None,
                    tags: Vec::new(),
                    notes: String::new(),
                    favorite: false,
                    sort_order: index as i64,
                    created_at: "2026-01-01T00:00:00Z".into(),
                    updated_at: "2026-01-01T00:00:00Z".into(),
                },
                secret: vec![1, 2, 3],
            })
            .collect();
        let categories = (0..category_count)
            .map(|index| CategoryView {
                id: format!("category-{index}"),
                name: format!("Category {index}"),
                color: "#123456".into(),
            })
            .collect();
        BackupPayload {
            accounts,
            categories,
            settings: AppSettings::default(),
            metadata: BackupMetadata {
                application: "AUTHENTICATOR DESK".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
            vault_id: None,
        }
    }

    #[test]
    fn encoder_rejects_counts_above_decoder_limits_before_kdf_or_write() {
        let oversized_accounts = payload(MAX_ACCOUNTS + 1, 0);
        assert!(check_counts(&oversized_accounts).is_err());
        assert!(encode(&oversized_accounts, &SecretString::from("password")).is_err());

        let oversized_categories = payload(0, MAX_CATEGORIES + 1);
        assert!(check_counts(&oversized_categories).is_err());
        assert!(encode(&oversized_categories, &SecretString::from("password")).is_err());
    }

    #[test]
    fn decoder_payload_path_enforces_the_same_counts_and_reads_legacy_documents() {
        let oversized_accounts = payload(MAX_ACCOUNTS + 1, 0);
        let encoded = serde_json::to_vec(&oversized_accounts).unwrap();
        assert!(decode_payload(&encoded).is_err());

        let oversized_categories = payload(0, MAX_CATEGORIES + 1);
        let encoded = serde_json::to_vec(&oversized_categories).unwrap();
        assert!(decode_payload(&encoded).is_err());

        // The identity field is optional for format-v1 documents created by
        // earlier releases, preserving their decode compatibility.
        let legacy = br#"{"accounts":[],"categories":[],"settings":{},"metadata":{"application":"AUTHENTICATOR DESK","created_at":"2026-01-01T00:00:00Z"}}"#;
        let decoded = decode_payload(legacy).unwrap();
        assert!(decoded.vault_id.is_none());
    }
}
