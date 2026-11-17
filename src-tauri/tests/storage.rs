use std::time::{SystemTime, UNIX_EPOCH};

use authenticator_desk_lib::storage::{initialize_connection, migrations, repository};
use rusqlite::Connection;

fn temp_path(label: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("authdesk-storage-{label}-{suffix}.sqlite3"))
}

#[test]
fn migrations_are_versioned_and_locked_reads_fail_before_secret_access() {
    let path = temp_path("migration");
    let mut conn = Connection::open(&path).unwrap();
    initialize_connection(&mut conn).unwrap();
    migrations::migrate(&mut conn).unwrap();
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, migrations::CURRENT_SCHEMA_VERSION);
    assert_eq!(
        repository::account_secret(&conn, None, "missing")
            .unwrap_err()
            .to_string(),
        "vault locked"
    );
    let _ = std::fs::remove_file(path);
}
