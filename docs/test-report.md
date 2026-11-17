# Test report

Recorded 2026-11-18 on Windows (Git Bash, MSVC toolchain). Every line below
reflects a command actually run in this working tree; statuses use PASS /
FAIL / PARTIAL / BLOCKED / NOT APPLICABLE.

## Commands and observed results

| Check | Command | Status | Observed |
|---|---|---|---|
| Rust unit + integration tests | `cargo test --manifest-path src-tauri/Cargo.toml` | PASS | exit 0; 14 test binaries, 74 tests passed, 0 failed |
| OTP vectors | included in Rust run | PASS | RFC 4226 HOTP counters 0–9; RFC 6238 SHA-1/256/512 vectors for t=59, 1111111109, 1111111111, 1234567890, 2000000000, 20000000000 |
| Vault lifecycle | `tests/vault.rs`, `tests/storage.rs` | PASS | create, unlock, wrong-password rejection, lock, re-unlock, integrity check, no plaintext secret in SQLite bytes |
| Accounts | `tests/accounts.rs` | PASS | CRUD, search, categories, tags, favorites, batch delete/move/reorder, duplicate conflict |
| otpauth import | `tests/otpauth.rs` | PASS | valid TOTP/HOTP URIs, label encoding, algorithms, digits, period, counter, invalid scheme/base32/params |
| Encrypted backup / restore | `tests/backup.rs` | PASS | round-trip, wrong password, tamper, truncated file, version mismatch, merge/replace previews |
| Security audit | `tests/security_audit.rs` | PASS | audit report runs over a real vault; SHA-1 alone is not reported weak |
| Clipboard | `tests/clipboard.rs` | PASS | token ownership, user content preserved, TTL cleanup |
| QR decode | `tests/qr.rs` | PASS | synthetic QR fixtures decode; malformed/empty images rejected |
| Desktop integrations | `tests/desktop.rs` | PASS | capability/config command surface (window-only APIs validated in-app, see limitations) |
| Migration adapters | `tests/import_formats.rs` | PASS | 12 tests: Google migration batches, Aegis/2FAS/andOTP field mapping + defaults, steam rejection, duplicate classification, transactional commit, encrypted-format rejection with and without password |
| Frontend tests | `pnpm test -- --run` | PASS | 5 files, 11 tests passed |
| TypeScript | `pnpm exec tsc --noEmit` | PASS | no errors |
| Desktop build | `cargo check --features desktop` / `pnpm tauri build` | PARTIAL | `cargo test` (default features) is green; full `pnpm tauri build` bundling evidence is tracked in Task 9 acceptance and not yet recorded in this report |

## Coverage notes

- OTP verification uses published RFC vectors only; no random or mocked OTP
  values are accepted as evidence.
- Migration fixtures under `src-tauri/tests/fixtures/import/` are synthetic,
  contain only public demo secrets, and were generated for this repository;
  provenance is recorded in `migration-format-matrix.md`.
- The desktop feature (tray, hotkeys, autostart, notifications) compiles and
  its commands are covered by `tests/desktop.rs`; interactive Windows
  behavior (tray click, global shortcut, Toast notification) still requires
  the Task 9 packaged-app evidence run.
- Not covered yet: installer verification, upgrade path, signing — tracked
  in `docs/acceptance-checklist.md` once Task 9 executes.
