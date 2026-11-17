PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

INSERT OR IGNORE INTO app_settings (key, value, updated_at)
VALUES
    ('schema_version', '1', '1970-01-01T00:00:00Z'),
    ('locale', 'zh-CN', '1970-01-01T00:00:00Z'),
    ('theme', 'dark', '1970-01-01T00:00:00Z');
