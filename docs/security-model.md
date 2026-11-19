# Security model

Scope covers the v0.2.0 code. See [known limitations](known-limitations.md) for remaining platform acceptance.

## Trust boundaries

- The Rust core (`src-tauri`) is the only component that reads the SQLite
  vault, derives keys, encrypts or decrypts records, parses imports, and
  writes backups. The React frontend receives typed, redacted view models
  through Tauri commands. Password entry, manual seed entry and file
  selection temporarily hold user-entered input in the WebView while the
  application sends it to the backend. Vault read responses are redacted.
- The frontend never persists the master password, derived key, account
  secret, or plaintext backup. Session material lives in Rust-owned
  zeroizing buffers and is dropped on lock, idle timeout, or exit.
- Error payloads serialize a stable code plus a safe message only; they never
  include secret material, SQL fragments, or filesystem paths.

## Cryptography

- Vault master key: Argon2id over the master password with a per-vault
  random salt. Parameters are stored in a versioned KDF header so they can
  evolve without breaking old vaults.
- Record encryption: AES-256-GCM with a fresh 96-bit nonce per record.
  Record IDs are bound as AAD so rows cannot be swapped. Keys live in
  `Zeroizing<[u8; 32]>`; decrypted buffers are zeroized after use.
- OTP secrets are stored as encrypted blobs, decoded into `SecretBytes`
  (no `Debug`, no `Serialize`) for generation, and zeroized on drop.
- Base32 decoding is strict RFC 4648: lowercase, embedded whitespace, and
  non-zero trailing bits are rejected.
- TOTP/HOTP implement HMAC-SHA1/SHA-256/SHA-512 with RFC 4226/6238 test
  vectors; codes are never generated from mock or random data in tests.

## Persistence and lifecycle

- SQLite via `rusqlite` with bundled SQLite. Schema changes go through
  transactional `PRAGMA user_version` migrations.
- Lock lifecycle: explicit lock, idle timeout tick (single one-second Rust
  thread), and process exit all clear pending imports, pending restores,
  clipboard tokens, cached codes, and emit `vault://locked`.
- A vault path is bound at startup from `AUTHDESK_DATA_DIR` or the OS app
  data directory; repositories fail with `vault_locked` before reading
  secret rows.

## Clipboard

- `copy_code` writes the code with an ownership token; `clear_clipboard_if_owned`
  clears it only when the clipboard still equals the app-written value, so
  later user copies are preserved. Codes are never logged. Clearing happens
  on TTL expiry, lock, and exit.

## Imports

- Import parsing (text URIs, Google migration protobuf, Aegis/2FAS/andOTP
  JSON, Ente/FreeOTP detection) happens in Rust with bounded inputs
  (2 MiB / 1000 rows). Secrets from files stay in zeroizing buffers and are
  retained only in the short-lived in-memory preview (5 minutes, cleared on
  lock or revision change).
- Encrypted third-party backups are rejected with
  `unsupported_encrypted_format`; no password guessing or envelope bypass is
  attempted. See `migration-format-matrix.md`.
- Duplicate detection compares issuer, account, and the decoded secret
  fingerprint before any commit; only rows still marked `new` can be
  committed, transactionally.

## Backups

- `.authdesk` backups use an independent Argon2id salt and AES-256-GCM with a
  versioned header bound as AAD, strict length checks, and tag validation.
  Loss of the backup password makes a password-encrypted backup
  unrecoverable by design; there is no recovery key.
- Restore decrypts, validates, and classifies conflicts into an in-memory
  preview that must be confirmed before merge or replace is applied.

## Network

- OTP generation uses the local monotonic-aware clock snapshot. An explicit
  update check or install uses GitHub HTTPS. The app sends no vault contents
  in either request. See [update trust](updating.md).

## Known gaps

Remaining gaps include Windows Hello, Authenticode, encrypted third-party
imports and camera capture. See [known limitations](known-limitations.md).
