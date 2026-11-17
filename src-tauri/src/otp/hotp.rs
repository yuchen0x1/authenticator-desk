use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::{AppError, AppResult};

use super::{OtpAlgorithm, OtpConfig};

pub(crate) fn generate_hotp(config: &OtpConfig, counter: u64) -> AppResult<String> {
    let mut message = [0u8; 8];
    message.copy_from_slice(&counter.to_be_bytes());
    let digest = match config.algorithm {
        OtpAlgorithm::Sha1 => hmac_sha1(config, &message)?,
        OtpAlgorithm::Sha256 => hmac_sha256(config, &message)?,
        OtpAlgorithm::Sha512 => hmac_sha512(config, &message)?,
    };
    truncate_and_format(&digest, config.digits)
}

fn hmac_sha1(config: &OtpConfig, message: &[u8]) -> AppResult<Vec<u8>> {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(config.secret.as_slice()).map_err(|_| AppError::Crypto)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn hmac_sha256(config: &OtpConfig, message: &[u8]) -> AppResult<Vec<u8>> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(config.secret.as_slice()).map_err(|_| AppError::Crypto)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn hmac_sha512(config: &OtpConfig, message: &[u8]) -> AppResult<Vec<u8>> {
    let mut mac =
        Hmac::<Sha512>::new_from_slice(config.secret.as_slice()).map_err(|_| AppError::Crypto)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

pub(crate) fn truncate_and_format(digest: &[u8], digits: u8) -> AppResult<String> {
    if !(6..=8).contains(&digits) || digest.len() < 20 {
        return Err(AppError::InvalidParameters);
    }
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let end = offset.checked_add(4).ok_or(AppError::Crypto)?;
    if end > digest.len() {
        return Err(AppError::Crypto);
    }
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let modulus = 10u32.pow(u32::from(digits));
    Ok(format!(
        "{:0width$}",
        binary % modulus,
        width = usize::from(digits)
    ))
}
