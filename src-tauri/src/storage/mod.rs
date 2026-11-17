pub mod migrations;
pub mod repository;

use rusqlite::Connection;

use crate::{AppError, AppResult};

pub fn initialize_connection(conn: &mut Connection) -> AppResult<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA secure_delete = ON; PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL;")
        .map_err(|_| AppError::Storage)
}
