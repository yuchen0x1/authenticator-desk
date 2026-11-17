use std::convert::TryInto;

use argon2::{Algorithm, Argon2, Params, Version};
use rand::{RngExt, rng};
use zeroize::Zeroizing;

use crate::{AppError, AppResult};

pub const KDF_MAGIC: &[u8; 4] = b"ADKF";
pub const KDF_VERSION: u8 = 1;
pub const KDF_ALGORITHM_ARGON2ID: u8 = 1;
pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const DEFAULT_MEMORY_KIB: u32 = 64 * 1024;
pub const DEFAULT_TIME_COST: u32 = 3;
pub const DEFAULT_LANES: u32 = 1;
pub const MIN_MEMORY_KIB: u32 = 8 * 1024;
pub const MAX_MEMORY_KIB: u32 = 256 * 1024;
pub const MAX_TIME_COST: u32 = 10;
pub const MAX_LANES: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KdfHeader {
    pub version: u8,
    pub algorithm: u8,
    pub memory_kib: u32,
    pub time_cost: u32,
    pub lanes: u32,
    pub output_len: u16,
    pub salt: Vec<u8>,
}

impl KdfHeader {
    pub fn new_default() -> AppResult<Self> {
        let mut salt = vec![0u8; SALT_LEN];
        rng().fill(&mut salt[..]);
        Ok(Self {
            version: KDF_VERSION,
            algorithm: KDF_ALGORITHM_ARGON2ID,
            memory_kib: DEFAULT_MEMORY_KIB,
            time_cost: DEFAULT_TIME_COST,
            lanes: DEFAULT_LANES,
            output_len: KEY_LEN as u16,
            salt,
        })
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.version != KDF_VERSION
            || self.algorithm != KDF_ALGORITHM_ARGON2ID
            || self.output_len as usize != KEY_LEN
            || self.salt.len() < SALT_LEN
            || self.salt.len() > 64
            || self.memory_kib < MIN_MEMORY_KIB
            || self.memory_kib > MAX_MEMORY_KIB
            || self.time_cost == 0
            || self.time_cost > MAX_TIME_COST
            || self.lanes == 0
            || self.lanes > MAX_LANES
        {
            return Err(AppError::InvalidParameters);
        }
        Ok(())
    }

    pub fn encode(&self) -> AppResult<Vec<u8>> {
        self.validate()?;
        let salt_len: u16 = self
            .salt
            .len()
            .try_into()
            .map_err(|_| AppError::InvalidParameters)?;
        let mut output = Vec::with_capacity(4 + 1 + 1 + 4 + 4 + 4 + 2 + 2 + self.salt.len());
        output.extend_from_slice(KDF_MAGIC);
        output.extend_from_slice(&[self.version, self.algorithm]);
        output.extend_from_slice(&self.memory_kib.to_le_bytes());
        output.extend_from_slice(&self.time_cost.to_le_bytes());
        output.extend_from_slice(&self.lanes.to_le_bytes());
        output.extend_from_slice(&self.output_len.to_le_bytes());
        output.extend_from_slice(&salt_len.to_le_bytes());
        output.extend_from_slice(&self.salt);
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> AppResult<Self> {
        if bytes.len() < 22 || &bytes[..4] != KDF_MAGIC {
            return Err(AppError::InvalidInput);
        }
        let version = bytes[4];
        let algorithm = bytes[5];
        let memory_kib = u32::from_le_bytes(
            bytes[6..10]
                .try_into()
                .map_err(|_| AppError::InvalidInput)?,
        );
        let time_cost = u32::from_le_bytes(
            bytes[10..14]
                .try_into()
                .map_err(|_| AppError::InvalidInput)?,
        );
        let lanes = u32::from_le_bytes(
            bytes[14..18]
                .try_into()
                .map_err(|_| AppError::InvalidInput)?,
        );
        let output_len = u16::from_le_bytes(
            bytes[18..20]
                .try_into()
                .map_err(|_| AppError::InvalidInput)?,
        );
        let salt_len = usize::from(u16::from_le_bytes(
            bytes[20..22]
                .try_into()
                .map_err(|_| AppError::InvalidInput)?,
        ));
        if salt_len > 64 || bytes.len() != 22 + salt_len {
            return Err(AppError::InvalidInput);
        }
        let header = Self {
            version,
            algorithm,
            memory_kib,
            time_cost,
            lanes,
            output_len,
            salt: bytes[22..].to_vec(),
        };
        header.validate()?;
        Ok(header)
    }

    pub fn derive_key(&self, password: &SecretString) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        self.validate()?;
        let params = Params::new(self.memory_kib, self.time_cost, self.lanes, Some(KEY_LEN))
            .map_err(|_| AppError::InvalidParameters)?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        argon
            .hash_password_into(password.as_bytes(), &self.salt, &mut key[..])
            .map_err(|_| AppError::Crypto)?;
        Ok(key)
    }
}

pub struct SecretString(Zeroizing<String>);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self::new(self.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{KdfHeader, SecretString};

    #[test]
    fn header_round_trip_and_bounds() {
        let header = KdfHeader::new_default().unwrap();
        let encoded = header.encode().unwrap();
        assert_eq!(KdfHeader::decode(&encoded).unwrap(), header);
        let mut tampered = encoded;
        tampered[6..10].copy_from_slice(&1u32.to_le_bytes());
        assert!(KdfHeader::decode(&tampered).is_err());
    }

    #[test]
    fn rejects_resource_exhaustion_kdf_headers() {
        let mut header = KdfHeader::new_default().unwrap();
        header.memory_kib = 1024 * 1024;
        assert!(header.validate().is_err());
        header.memory_kib = 64 * 1024;
        header.time_cost = 16;
        assert!(header.validate().is_err());
    }

    #[test]
    fn same_password_and_header_derives_same_key() {
        let header = KdfHeader::new_default().unwrap();
        let password = SecretString::from("test-password");
        assert_eq!(
            header.derive_key(&password).unwrap().as_slice(),
            header.derive_key(&password).unwrap().as_slice()
        );
    }
}
