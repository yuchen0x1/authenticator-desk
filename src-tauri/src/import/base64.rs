use zeroize::Zeroizing;

use crate::{AppError, AppResult};

pub(crate) const MAX_DECODED_BYTES: usize = 2 * 1024 * 1024;
/// Reject oversized transport text before allocating or scanning it.
const MAX_ENCODED_LEN: usize = MAX_DECODED_BYTES / 3 * 4 + 4;

fn value_of(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Decode standard or URL-safe Base64 with optional canonical padding.
///
/// Google Authenticator migration payloads arrive URL-safe-encoded, while
/// other producers emit the standard alphabet; mixing alphabets, embedded
/// whitespace, or non-terminal padding is rejected so malformed transport
/// never reaches the protobuf reader.
pub fn decode_base64(encoded: &str) -> AppResult<Zeroizing<Vec<u8>>> {
    if encoded.is_empty() || encoded.len() > MAX_ENCODED_LEN {
        return Err(AppError::InvalidInput);
    }
    let bytes = encoded.as_bytes();
    let standard = bytes.iter().any(|b| *b == b'+' || *b == b'/');
    let url_safe = bytes.iter().any(|b| *b == b'-' || *b == b'_');
    if standard && url_safe {
        return Err(AppError::InvalidInput);
    }
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
    let remainder = data % 4;
    let expected_pad = match remainder {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => return Err(AppError::InvalidInput),
    };
    if pad_count != 0 && (bytes.len() % 4 != 0 || pad_count != expected_pad) {
        return Err(AppError::InvalidInput);
    }
    let decoded_len = data * 3 / 4;
    if decoded_len > MAX_DECODED_BYTES {
        return Err(AppError::InvalidInput);
    }
    let mut output = Zeroizing::new(Vec::with_capacity(decoded_len));
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in &bytes[..data] {
        buffer = (buffer << 6) | u32::from(value_of(*byte).ok_or(AppError::InvalidInput)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1u32 << bits).saturating_sub(1);
        }
    }
    if bits > 0 && buffer != 0 {
        return Err(AppError::InvalidInput);
    }
    if output.len() != decoded_len || output.len() > MAX_DECODED_BYTES {
        return Err(AppError::InvalidInput);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{MAX_DECODED_BYTES, MAX_ENCODED_LEN, decode_base64};

    #[test]
    fn transport_accepts_two_mib_and_rejects_larger_decoded_or_encoded_inputs() {
        let mut encoded = "A".repeat((MAX_DECODED_BYTES * 4).div_ceil(3));
        assert_eq!(decode_base64(&encoded).unwrap().len(), MAX_DECODED_BYTES);
        encoded.push('=');
        assert_eq!(decode_base64(&encoded).unwrap().len(), MAX_DECODED_BYTES);
        encoded.pop();
        encoded.push('A');
        assert!(encoded.len() <= MAX_ENCODED_LEN);
        assert!(decode_base64(&encoded).is_err());
        encoded.push('A');
        assert!(encoded.len() > MAX_ENCODED_LEN);
        assert!(decode_base64(&encoded).is_err());
    }

    #[test]
    fn decodes_standard_and_url_safe_alphabets() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap().as_slice(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap().as_slice(), b"hello");
        assert_eq!(
            decode_base64("-_8").unwrap().as_slice(),
            decode_base64("+/8").unwrap().as_slice()
        );
        assert!(decode_base64("+_8").is_err());
        assert!(decode_base64("").is_err());
        assert!(decode_base64("aGVs bG8").is_err());
        assert!(decode_base64("aGVsbG8=").is_ok());
        assert!(decode_base64("aGVs=").is_err());
        assert!(decode_base64("aGVsbG").is_err());
        assert!(decode_base64("aGVsbG8!").is_err());
    }
}
