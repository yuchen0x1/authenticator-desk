use std::convert::TryInto;

use argon2::{Algorithm, Argon2, Block, Params, Version};
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
        let mut memory = allocate_work_memory(&params)?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        argon
            .hash_password_into_with_memory(
                password.as_bytes(),
                &self.salt,
                &mut key[..],
                &mut memory[..],
            )
            .map_err(|_| AppError::Crypto)?;
        Ok(key)
    }
}

// Argon2 0.6's internally allocated work memory is deallocated without wiping.
// Keep a fixed-size, owned buffer zeroizing on success, error, and unwinding.
fn allocate_work_memory(params: &Params) -> AppResult<Zeroizing<Box<[Block]>>> {
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(params.block_count())
        .map_err(|_| AppError::Crypto)?;
    blocks.resize(params.block_count(), Block::new());
    Ok(Zeroizing::new(blocks.into_boxed_slice()))
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
    use super::{
        Algorithm, Argon2, DEFAULT_LANES, DEFAULT_MEMORY_KIB, DEFAULT_TIME_COST, KEY_LEN,
        KdfHeader, MIN_MEMORY_KIB, Params, SecretString, Version, allocate_work_memory,
    };
    use zeroize::{Zeroize, ZeroizeOnDrop};

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
    fn explicit_work_memory_preserves_existing_argon2id_derivation() {
        let password = SecretString::from("work-memory-regression");
        for (memory_kib, time_cost, lanes) in [
            (MIN_MEMORY_KIB, 1, 1),
            (MIN_MEMORY_KIB + 1, 2, 3),
            (DEFAULT_MEMORY_KIB, DEFAULT_TIME_COST, DEFAULT_LANES),
        ] {
            let mut header = KdfHeader::new_default().unwrap();
            header.salt = b"fixed-test-salt!0".to_vec();
            header.memory_kib = memory_kib;
            header.time_cost = time_cost;
            header.lanes = lanes;
            let params = Params::new(memory_kib, time_cost, lanes, Some(KEY_LEN)).unwrap();
            let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            let mut expected = [0u8; KEY_LEN];
            argon
                .hash_password_into(password.as_bytes(), &header.salt, &mut expected)
                .unwrap();
            assert_eq!(
                header.derive_key(&password).unwrap().as_slice(),
                expected,
                "memory={memory_kib}, time={time_cost}, lanes={lanes}",
            );
        }
    }

    #[test]
    fn work_memory_uses_argon2_block_count_and_zeroizes_in_place() {
        fn assert_zeroizes_on_drop<T: ZeroizeOnDrop>(_: &T) {}

        // The memory cost is rounded down to a multiple of lanes * four blocks.
        let params = Params::new(MIN_MEMORY_KIB + 1, 1, 3, Some(KEY_LEN)).unwrap();
        let mut memory = allocate_work_memory(&params).unwrap();
        assert_zeroizes_on_drop(&memory);
        assert_eq!(memory.len(), params.block_count());
        assert_ne!(memory.len(), params.m_cost() as usize);
        assert!(
            memory
                .iter()
                .all(|block| block.as_ref().iter().all(|&word| word == 0))
        );

        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = [0u8; KEY_LEN];
        argon
            .hash_password_into_with_memory(
                b"test-password",
                b"fixed-test-salt!0",
                &mut output,
                &mut memory[..],
            )
            .unwrap();
        assert!(
            memory
                .iter()
                .any(|block| block.as_ref().iter().any(|&word| word != 0))
        );
        let address = memory.as_ptr();
        let block_count = memory.len();
        memory.zeroize();
        assert_eq!(memory.as_ptr(), address);
        assert_eq!(memory.len(), block_count);
        assert!(
            memory
                .iter()
                .all(|block| block.as_ref().iter().all(|&word| word == 0))
        );
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
