use std::convert::TryFrom;

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadInOut, Payload},
};
use rand::{RngExt, rng};
use zeroize::Zeroizing;

use crate::{AppError, AppResult};

const NONCE_LEN: usize = 12;

pub fn encrypt_record(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> AppResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Crypto)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng().fill(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &Nonce::try_from(&nonce_bytes[..]).map_err(|_| AppError::Crypto)?,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AppError::Crypto)?;
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt_record(
    key: &[u8; 32],
    aad: &[u8],
    ciphertext: &[u8],
) -> AppResult<Zeroizing<Vec<u8>>> {
    if ciphertext.len() <= NONCE_LEN {
        return Err(AppError::Crypto);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Crypto)?;
    let mut plaintext = Zeroizing::new(ciphertext[NONCE_LEN..].to_vec());
    cipher
        .decrypt_in_place(
            &Nonce::try_from(&ciphertext[..NONCE_LEN]).map_err(|_| AppError::Crypto)?,
            aad,
            &mut *plaintext,
        )
        .map_err(|_| AppError::AuthenticationFailed)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::{decrypt_record, encrypt_record};
    use zeroize::Zeroizing;

    #[test]
    fn decrypt_returns_zeroizing_plaintext_buffer() {
        let key = [8u8; 32];
        let encrypted =
            encrypt_record(&key, b"buffer-test", b"secret-in-zeroizing-buffer").unwrap();
        let plaintext: Zeroizing<Vec<u8>> =
            decrypt_record(&key, b"buffer-test", &encrypted).unwrap();
        assert_eq!(plaintext.as_slice(), b"secret-in-zeroizing-buffer");
    }

    #[test]
    fn round_trip_uses_unique_nonces_and_authenticates() {
        let key = [7u8; 32];
        let first = encrypt_record(&key, b"record-1", b"secret payload").unwrap();
        let second = encrypt_record(&key, b"record-1", b"secret payload").unwrap();
        assert_ne!(first, second);
        assert_eq!(
            decrypt_record(&key, b"record-1", &first)
                .unwrap()
                .as_slice(),
            b"secret payload"
        );
        assert!(decrypt_record(&key, b"record-2", &first).is_err());
        let mut modified = first.clone();
        *modified.last_mut().unwrap() ^= 0x40;
        assert!(decrypt_record(&key, b"record-1", &modified).is_err());
    }
}
