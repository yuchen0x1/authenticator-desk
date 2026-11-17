use rusqlite::{Connection, OptionalExtension, params};

use crate::{AppError, AppResult};

pub fn require_unlocked<T>(
    key: Option<&[u8; 32]>,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    if key.is_none() {
        return Err(AppError::VaultLocked);
    }
    operation()
}

pub fn account_secret(
    conn: &Connection,
    key: Option<&[u8; 32]>,
    account_id: &str,
) -> AppResult<Vec<u8>> {
    require_unlocked(key, || {
        conn.query_row(
            "SELECT secret_blob FROM accounts WHERE id = ?1",
            params![account_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::Storage)?
        .ok_or(AppError::NotFound)
    })
}
