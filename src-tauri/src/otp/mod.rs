mod base32;
mod hotp;
mod time;
mod totp;

pub use base32::{SecretBytes, decode_base32, encode_base32};
pub use time::{ClockSample, MonotonicClock, TimeState, time_state, time_state_with_offset};

use crate::{AppError, AppResult};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OtpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OtpKind {
    Totp { period_secs: u32, t0: i64 },
    Hotp { counter: u64 },
}

pub struct OtpConfig {
    pub secret: SecretBytes,
    pub algorithm: OtpAlgorithm,
    pub digits: u8,
    pub kind: OtpKind,
}

impl OtpConfig {
    pub fn validate(&self) -> AppResult<()> {
        if !(6..=8).contains(&self.digits) || self.secret.len() == 0 {
            return Err(AppError::InvalidParameters);
        }
        if let OtpKind::Totp { period_secs, .. } = self.kind {
            if period_secs == 0 {
                return Err(AppError::InvalidParameters);
            }
        }
        Ok(())
    }
}

pub fn generate_code(config: &OtpConfig, unix_seconds: i64) -> AppResult<String> {
    config.validate()?;
    match config.kind {
        OtpKind::Hotp { counter } => hotp::generate_hotp(config, counter),
        OtpKind::Totp { period_secs, t0 } => {
            totp::generate_totp(config, unix_seconds, period_secs, t0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OtpAlgorithm, OtpConfig, OtpKind, SecretBytes, generate_code};

    fn secret(value: &[u8]) -> SecretBytes {
        SecretBytes::from_bytes(value).unwrap()
    }

    #[test]
    fn rfc4226_hotp_vectors() {
        let key = secret(b"12345678901234567890");
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, expected) in expected.into_iter().enumerate() {
            let config = OtpConfig {
                secret: key.clone(),
                algorithm: OtpAlgorithm::Sha1,
                digits: 6,
                kind: OtpKind::Hotp {
                    counter: counter as u64,
                },
            };
            assert_eq!(generate_code(&config, 0).unwrap(), expected);
        }
    }

    #[test]
    fn rfc6238_totp_vectors_all_algorithms() {
        let timestamps = [
            59_i64,
            1_111_111_109,
            1_111_111_111,
            1_234_567_890,
            2_000_000_000,
            20_000_000_000,
        ];
        let sha1 = [
            "94287082", "07081804", "14050471", "89005924", "69279037", "65353130",
        ];
        let sha256 = [
            "46119246", "68084774", "67062674", "91819424", "90698825", "77737706",
        ];
        let sha512 = [
            "90693936", "25091201", "99943326", "93441116", "38618901", "47863826",
        ];
        for (index, timestamp) in timestamps.into_iter().enumerate() {
            for (algorithm, expected) in [
                (OtpAlgorithm::Sha1, sha1[index]),
                (OtpAlgorithm::Sha256, sha256[index]),
                (OtpAlgorithm::Sha512, sha512[index]),
            ] {
                let key = match algorithm {
                    OtpAlgorithm::Sha1 => secret(b"12345678901234567890"),
                    OtpAlgorithm::Sha256 => secret(b"12345678901234567890123456789012"),
                    OtpAlgorithm::Sha512 => {
                        secret(b"1234567890123456789012345678901234567890123456789012345678901234")
                    }
                };
                let config = OtpConfig {
                    secret: key,
                    algorithm,
                    digits: 8,
                    kind: OtpKind::Totp {
                        period_secs: 30,
                        t0: 0,
                    },
                };
                assert_eq!(generate_code(&config, timestamp).unwrap(), expected);
            }
        }
    }

    #[test]
    fn supports_six_and_seven_digits() {
        let key = secret(b"12345678901234567890");
        let six = OtpConfig {
            secret: key.clone(),
            algorithm: OtpAlgorithm::Sha1,
            digits: 6,
            kind: OtpKind::Totp {
                period_secs: 30,
                t0: 0,
            },
        };
        let seven = OtpConfig {
            secret: key,
            algorithm: OtpAlgorithm::Sha1,
            digits: 7,
            kind: OtpKind::Totp {
                period_secs: 30,
                t0: 0,
            },
        };
        assert_eq!(generate_code(&six, 59).unwrap(), "287082");
        assert_eq!(generate_code(&seven, 59).unwrap(), "4287082");
    }

    #[test]
    fn rejects_invalid_parameters_and_time_before_t0() {
        let key = secret(b"12345678901234567890");
        for digits in [0, 5, 9] {
            let config = OtpConfig {
                secret: key.clone(),
                algorithm: OtpAlgorithm::Sha1,
                digits,
                kind: OtpKind::Hotp { counter: 0 },
            };
            assert!(generate_code(&config, 0).is_err());
        }
        let bad_period = OtpConfig {
            secret: key.clone(),
            algorithm: OtpAlgorithm::Sha1,
            digits: 6,
            kind: OtpKind::Totp {
                period_secs: 0,
                t0: 0,
            },
        };
        assert!(generate_code(&bad_period, 0).is_err());
        let before_t0 = OtpConfig {
            secret: key,
            algorithm: OtpAlgorithm::Sha1,
            digits: 6,
            kind: OtpKind::Totp {
                period_secs: 30,
                t0: 100,
            },
        };
        assert!(generate_code(&before_t0, 99).is_err());
    }

    #[test]
    fn handles_far_future_and_i64_time_edges_with_checked_arithmetic() {
        let key = secret(b"12345678901234567890");
        let config = OtpConfig {
            secret: key.clone(),
            algorithm: OtpAlgorithm::Sha1,
            digits: 8,
            kind: OtpKind::Totp {
                period_secs: 1,
                t0: i64::MIN,
            },
        };
        assert!(generate_code(&config, i64::MAX).is_ok());

        let before = OtpConfig {
            secret: key,
            algorithm: OtpAlgorithm::Sha1,
            digits: 6,
            kind: OtpKind::Totp {
                period_secs: 30,
                t0: i64::MAX,
            },
        };
        assert!(generate_code(&before, i64::MIN).is_err());
    }
}
