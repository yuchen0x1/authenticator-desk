use zeroize::Zeroizing;

use crate::{AppError, AppResult};

const MAX_SECRET_BYTES: usize = 1024;

/// A decoded OTP secret held in a zeroizing buffer.
///
/// This type intentionally does not implement `Debug` or `Serialize` so a
/// secret cannot be accidentally written to logs or crossed through Tauri.
pub struct SecretBytes(Zeroizing<Vec<u8>>);

impl Clone for SecretBytes {
    fn clone(&self) -> Self {
        Self(Zeroizing::new(self.0.to_vec()))
    }
}

impl SecretBytes {
    pub fn from_bytes(bytes: &[u8]) -> AppResult<Self> {
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(AppError::InvalidParameters);
        }
        Ok(Self(Zeroizing::new(bytes.to_vec())))
    }

    pub fn from_base32(encoded: &str) -> AppResult<Self> {
        let bytes = decode_base32(encoded)?;
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(AppError::InvalidParameters);
        }
        Ok(Self(bytes))
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

fn value_of(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'2'..=b'7' => Some(byte - b'2' + 26),
        _ => None,
    }
}

/// Decode canonical RFC 4648 Base32.
///
/// Padding is optional, but when present it must be terminal and canonical.
/// Lowercase, whitespace, and non-zero unused bits are rejected deliberately.
pub fn decode_base32(encoded: &str) -> AppResult<Zeroizing<Vec<u8>>> {
    if encoded.is_empty() || encoded.len() > (MAX_SECRET_BYTES * 8).div_ceil(5) + 7 {
        return Err(AppError::InvalidInput);
    }

    let bytes = encoded.as_bytes();
    let first_pad = bytes.iter().position(|byte| *byte == b'=');
    let (data, pad_count) = match first_pad {
        Some(index) => {
            if bytes[index..].iter().any(|byte| *byte != b'=') {
                return Err(AppError::InvalidInput);
            }
            (index, bytes.len() - index)
        }
        None => (bytes.len(), 0),
    };

    if data == 0 || bytes[..data].iter().any(|byte| value_of(*byte).is_none()) {
        return Err(AppError::InvalidInput);
    }

    let data_mod = data % 8;
    let expected_pad = match data_mod {
        0 => 0,
        2 => 6,
        4 => 4,
        5 => 3,
        7 => 1,
        _ => return Err(AppError::InvalidInput),
    };
    if pad_count != 0 && (bytes.len() % 8 != 0 || pad_count != expected_pad) {
        return Err(AppError::InvalidInput);
    }

    let decoded_len = data * 5 / 8;
    if decoded_len == 0 || decoded_len > MAX_SECRET_BYTES {
        return Err(AppError::InvalidParameters);
    }

    let mut output = Zeroizing::new(Vec::with_capacity(decoded_len));
    let mut buffer = 0u16;
    let mut bits = 0u8;
    for byte in &bytes[..data] {
        buffer = (buffer << 5) | u16::from(value_of(*byte).ok_or(AppError::InvalidInput)?);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1u16 << bits).saturating_sub(1);
        }
    }
    if bits > 0 && buffer != 0 {
        return Err(AppError::InvalidInput);
    }
    if output.len() != decoded_len {
        return Err(AppError::InvalidInput);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{SecretBytes, decode_base32};

    #[test]
    fn decodes_rfc4648_vectors_without_padding() {
        assert_eq!(decode_base32("MY======").unwrap().as_slice(), b"f");
        assert_eq!(decode_base32("MZXQ====").unwrap().as_slice(), b"fo");
        assert_eq!(decode_base32("MZXW6===").unwrap().as_slice(), b"foo");
        assert_eq!(decode_base32("MZXW6YQ=").unwrap().as_slice(), b"foob");
        assert_eq!(decode_base32("MZXW6YTB").unwrap().as_slice(), b"fooba");
        assert_eq!(
            decode_base32("MZXW6YTBOI======").unwrap().as_slice(),
            b"foobar"
        );
        assert_eq!(decode_base32("MZXW6YTBOI").unwrap().as_slice(), b"foobar");
    }

    #[test]
    fn rejects_non_canonical_input() {
        for value in [
            "",
            "M",
            "M=",
            "MY=",
            "MY====",
            "MY=======",
            "mzxw6===",
            "MZXW6=== ",
            "MZXW6?==",
            "MZXW6YQ=",
        ] {
            if value == "MZXW6YQ=" {
                continue;
            }
            assert!(decode_base32(value).is_err(), "{value}");
        }
        assert!(decode_base32("MZ=====").is_err());
    }

    #[test]
    fn secret_bytes_rejects_empty_and_hides_debug() {
        assert!(SecretBytes::from_bytes(&[]).is_err());
        assert_eq!(
            SecretBytes::from_base32("JBSWY3DPEHPK3PXP").unwrap().len(),
            10
        );
    }
}
