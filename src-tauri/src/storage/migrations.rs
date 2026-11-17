use rusqlite::Connection;

use crate::{AppError, AppResult};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn migrate(conn: &mut Connection) -> AppResult<()> {
    let tx = conn.transaction().map_err(|_| AppError::Storage)?;
    let version: u32 = tx
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| AppError::Storage)?;
    if version == 0 {
        tx.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .map_err(|_| AppError::Storage)?;
        tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .map_err(|_| AppError::Storage)?;
    } else if version != CURRENT_SCHEMA_VERSION {
        return Err(AppError::Storage);
    }
    tx.commit().map_err(|_| AppError::Storage)
}
