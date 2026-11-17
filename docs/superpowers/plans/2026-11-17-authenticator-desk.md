# AUTHENTICATOR DESK Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

Goal: Build a usable, offline-first Windows 2FA desktop application with a Rust security core, encrypted SQLite vault, React desktop UI, verified TOTP/HOTP behavior, URI import, encrypted backups, security auditing, and honest release evidence.

Architecture: Tauri 2 hosts a React/TypeScript desktop UI and a Rust application core. Rust owns secrets, cryptographic operations, persistence, time state, OTP generation, import validation, and backup/restore commands; React receives only typed, redacted view models through Tauri commands. The implementation is staged so each phase produces a runnable and testable increment.

Tech Stack: Tauri 2, Rust 1.99+, React, TypeScript, Vite, Tailwind CSS, SQLite via rusqlite, aes-gcm, argon2, hmac, sha1/sha2, base32, zeroize, uuid, chrono, Vitest, Testing Library, and Windows NSIS/MSI bundling.

Spec: docs/superpowers/specs/2026-11-17-authenticator-desk-design.md

## Global Constraints

- Offline first: no account data or secrets are uploaded; network time calibration only makes a time request.
- Frontend never reads SQLite and never persists the master password, derived key, account Secret, or plaintext backup.
- TOTP/HOTP, Argon2id, AES-256-GCM, SQLite access, imports, backups, and sensitive-memory cleanup stay in Rust.
- Use explicit typed Tauri commands and non-sensitive serializable errors.
- Use versioned SQLite migrations and a versioned .authdesk backup format.
- TOTP supports HMAC-SHA1/SHA-256/SHA-512, 6/7/8 digits, configurable period and T0; HOTP uses a persisted monotonic counter.
- Use RFC 4226 and RFC 6238 vectors; never use random data or mock calculations as OTP verification.
- Do not claim Windows Hello, camera capture, signing, installation upgrades, or online release verification without evidence.
- UI controls must call real commands; all user-facing strings belong in locale resources.
- Do not overwrite uncommitted work; every task ends with focused tests and a small commit.

---

### Task 1: Scaffold the Tauri workspace and secure module boundaries

Files:
- Create package.json, pnpm-lock.yaml, vite.config.ts, tsconfig.json, index.html.
- Create src/main.tsx, src/App.tsx, src/lib/tauri.ts, src/lib/types.ts.
- Create src-tauri/Cargo.toml, src-tauri/tauri.conf.json, src-tauri/src/main.rs, src-tauri/src/lib.rs.
- Create src-tauri/src/error.rs, src-tauri/src/commands/mod.rs, src-tauri/src/services/mod.rs.
- Create src-tauri/migrations/0001_initial.sql and initial README.md, LICENSE, .gitignore.
- Test src/App.test.tsx and a Rust compile smoke test.

Interfaces:
- AppResult<T> = Result<T, AppError>.
- AppState is the Tauri managed state container.
- tauriInvoke<T>(command: string, payload?: unknown): Promise<T>.
- app_snapshot returns AppSnapshot { locked, vault_exists, locale, theme }.

Steps:
- [ ] Generate the project with pnpm create tauri-app@latest . -- --template react-ts --manager pnpm.
- [ ] Add Tauri, serde, serde_json, thiserror, rusqlite bundled SQLite, argon2, aes-gcm, rand, zeroize, hmac, sha1, sha2, base32, uuid, chrono, and url dependencies.
- [ ] Define AppError variants VaultLocked, InvalidInput, AuthenticationFailed, Storage, Crypto, Conflict, and NotFound. Serialize only stable error codes and safe messages.
- [ ] Add a locked-state shell with navigation placeholders and no secret-bearing fields.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml, pnpm test -- --run src/App.test.tsx, and pnpm exec tsc --noEmit.
- [ ] Commit with git commit -m "build: scaffold authenticator desk tauri app".

### Task 2: Implement and verify the Rust OTP engine

Files:
- Create src-tauri/src/otp/mod.rs, base32.rs, totp.rs, hotp.rs, and time.rs.
- Modify src-tauri/src/lib.rs and src-tauri/src/error.rs.
- Test src-tauri/src/otp/*.rs and src-tauri/tests/otp_commands.rs.

Interfaces:
- OtpAlgorithm::{Sha1, Sha256, Sha512}.
- OtpKind::{Totp { period_secs: u32, t0: i64 }, Hotp { counter: u64 }}.
- OtpConfig { secret: SecretBytes, algorithm, digits: u8, kind }.
- generate_code(config: &OtpConfig, unix_seconds: i64) -> AppResult<String>.
- time_state(unix_seconds: i64, period_secs: u32, t0: i64) -> TimeState.
- TimeState { code_epoch, remaining_secs, progress, clock_offset_secs }.

Steps:
- [ ] Add RFC 4226 HOTP vectors for counters 0 through 9 using 12345678901234567890 and assert 755224, 287082, 359152, 969429, 338314, 254676, 287922, 162583, 399871, 520489.
- [ ] Add RFC 6238 SHA-1, SHA-256, and SHA-512 vectors for timestamps 59, 1111111109, 1111111111, 1234567890, 2000000000, and 20000000000.
- [ ] Add 6- and 7-digit truncation tests, period/T0 boundary tests, invalid Base32 tests, and invalid parameter tests.
- [ ] Implement strict RFC 4648 Base32 decoding, big-endian counter/time-step encoding, dynamic truncation, modulo 10^digits, and left padding.
- [ ] Implement a monotonic-aware clock snapshot that detects wall-clock jumps above 5 seconds and exposes one shared time state.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml otp:: -- --nocapture and cargo test --manifest-path src-tauri/Cargo.toml --test otp_commands.
- [ ] Commit with git commit -m "feat: implement verified totp and hotp engine".

### Task 3: Add the encrypted SQLite vault and lock lifecycle

Files:
- Create src-tauri/src/vault/mod.rs, crypto.rs, kdf.rs, session.rs.
- Create src-tauri/src/storage/mod.rs, migrations.rs, repository.rs.
- Modify src-tauri/migrations/0001_initial.sql, src-tauri/src/lib.rs, and src-tauri/src/commands/mod.rs.
- Test src-tauri/tests/vault.rs and storage.rs.

Interfaces:
- create_vault(password: SecretString, path: &Path) -> AppResult<()>.
- unlock_vault(password: SecretString) -> AppResult<SessionHandle>.
- lock_vault() -> AppResult<()>.
- is_locked() -> bool.
- encrypt_record(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> AppResult<Vec<u8>>.
- decrypt_record(key: &[u8; 32], aad: &[u8], ciphertext: &[u8]) -> AppResult<Vec<u8>>.
- integrity_check() -> AppResult<IntegrityReport>.
- Tables: vault_meta, accounts, categories, settings, audit_events; Secret and sensitive JSON are encrypted blobs.

Steps:
- [ ] Write temporary-directory tests for first-run creation, correct unlock, wrong-password rejection, lock, re-unlock, and absence of plaintext Secret in SQLite bytes.
- [ ] Write crypto tests for round-trip, unique nonces, AAD mismatch, modified ciphertext, and zeroized session material after lock.
- [ ] Implement Argon2id with a versioned KDF header and AES-256-GCM with a fresh 96-bit nonce per record. Use stable record IDs as AAD and keep keys in Zeroizing<[u8; 32]>.
- [ ] Implement transactional PRAGMA user_version migrations and repositories that fail with VaultLocked before reading secret rows.
- [ ] Expose create_vault, unlock_vault, lock_vault, vault_status, and integrity_check commands; emit vault://locked and clear cached codes.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml --test vault --test storage and cargo test --manifest-path src-tauri/Cargo.toml.
- [ ] Commit with git commit -m "feat: add encrypted vault and lock lifecycle".

### Task 4: Build account management and typed Tauri commands

Files:
- Create src-tauri/src/accounts/mod.rs, model.rs, service.rs, templates.rs.
- Create src-tauri/src/import/mod.rs and otpauth.rs.
- Modify src-tauri/src/commands/mod.rs, src-tauri/src/lib.rs, src/lib/types.ts, and src/lib/tauri.ts.
- Test src-tauri/tests/accounts.rs and otpauth.rs.

Interfaces:
- AccountInput { issuer, account_name, secret, algorithm, digits, period_secs, t0, hotp_counter, category_id, tags, notes, favorite }.
- AccountQuery { search, category_id, favorite_only, sort }.
- AccountView { id, issuer, account_name, algorithm, digits, period_secs, kind, category, tags, notes_present, favorite, created_at, updated_at }.
- CodeView { account_id, code, remaining_secs, progress, generated_at }.
- Commands: list_accounts, create_account, update_account, delete_account, delete_accounts, set_favorite, move_accounts, reorder_accounts, generate_codes, parse_otpauth, preview_import, commit_import.

Steps:
- [ ] Test add/edit/delete, category and tag filters, favorites, stable ordering, duplicate detection by issuer/account/secret fingerprint, and HOTP counter monotonic updates.
- [ ] Test valid TOTP/HOTP URIs, label encoding, SHA-1/SHA-256/SHA-512, digits, period, counter, missing secret, invalid scheme, invalid Base32, unsupported parameters, and conflict preview.
- [ ] Implement encrypted Secret payloads, redacted AccountView, Rust validation, and matching frontend zod validation.
- [ ] Add GitHub, Google, Microsoft, Discord, Cloudflare, Amazon AWS, Steam, Facebook, Instagram, Binance, and Custom templates.
- [ ] Parse and validate every import item before returning per-item new/duplicate/invalid status. Commit selected valid items transactionally.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml --test accounts --test otpauth.
- [ ] Commit with git commit -m "feat: add account management and otpauth import".

### Task 5: Add encrypted backups, restore previews, and security audit

Files:
- Create src-tauri/src/backup/mod.rs, format.rs, restore.rs.
- Create src-tauri/src/security/mod.rs and audit.rs.
- Modify src-tauri/src/commands/mod.rs and src-tauri/src/lib.rs.
- Test src-tauri/tests/backup.rs and security_audit.rs.

Interfaces:
- BackupHeader { magic: AUTHDESK, version: u16, kdf: KdfParams, nonce: [u8; 12], ciphertext_len: u64 }.
- BackupPayload { accounts, categories, settings, metadata }.
- create_backup(password: SecretString, destination: &Path) -> AppResult<BackupSummary>.
- preview_restore(password: SecretString, source: &Path, mode: RestoreMode) -> AppResult<RestorePreview>.
- apply_restore(preview_id: Uuid, selection: RestoreSelection) -> AppResult<RestoreResult>.
- RestoreMode::{Merge, Replace}.
- SecurityAudit { integrity, duplicate_count, invalid_config_count, plaintext_file_findings, permission_findings, backup_status, time_status }.

Steps:
- [ ] Test encrypted round-trip, wrong password, byte tamper, truncated file, version mismatch, duplicate merge, replace restore, and no unencrypted default backup.
- [ ] Test that SHA-1 TOTP is not reported weak solely for algorithm; detect duplicates, invalid digits/periods, missing backups, and plaintext fixture files.
- [ ] Implement versioned .authdesk with independent Argon2id salt, AES-256-GCM, header AAD, strict length checks, and tag validation.
- [ ] Decrypt, validate, classify conflicts, and keep a short-lived in-memory preview ID. Apply merge or replace only after explicit confirmation.
- [ ] Add create_backup, preview_restore, apply_restore, list_backups, run_security_audit, and list_audit_events commands.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml --test backup --test security_audit.
- [ ] Commit with git commit -m "feat: add encrypted backups and security audit".

### Task 6: Build the real desktop UI and localization

Files:
- Create src/components/layout/AppShell.tsx, Sidebar.tsx, TopBar.tsx.
- Create src/components/accounts/AccountCard.tsx, AccountList.tsx, AccountDialog.tsx, CodeRing.tsx.
- Create src/components/common/EmptyState.tsx and ErrorState.tsx.
- Create src/pages/DashboardPage.tsx, CodesPage.tsx, AccountsPage.tsx, ImportPage.tsx, BackupPage.tsx, SecurityPage.tsx, SettingsPage.tsx.
- Create src/i18n/index.ts, locale files zh-CN.ts, zh-TW.ts, en.ts, ja.ts, and src/styles/theme.css.
- Modify src/App.tsx, src/lib/tauri.ts, src/lib/types.ts.
- Test AccountCard.test.tsx, CodesPage.test.tsx, and i18n/index.test.ts.

Interfaces:
- useVaultStatus(): { locked, vaultExists, refresh }.
- useAccounts(query: AccountQuery): { data, loading, error, refresh }.
- useCodes(accountIds: string[]): { data: CodeView[], time: TimeState, refresh }.
- Pages consume typed command wrappers; no page owns Secret state.

Steps:
- [ ] Test locked state hiding codes, empty-state Add Account and Import actions, copy through copy_code, favorite through set_favorite, and runtime locale switching.
- [ ] Add deep navy/black surfaces, blue-violet accent, restrained glass panels, focus rings, 8px spacing, reduced-motion support, and one deliberate refresh animation.
- [ ] Implement shell, navigation, top status, content area, toasts, locked overlay, responsive narrow-window behavior, and real route mapping.
- [ ] Implement code center card/list toggle, search, category/favorite filters, code, countdown, progress ring, copy, favorite, and batch selection from live commands. Use one shared one-second refresh loop.
- [ ] Implement account, import, backup, security, and settings pages with real commands and truthful unavailable states.
- [ ] Run pnpm test -- --run, pnpm exec tsc --noEmit, and pnpm lint.
- [ ] Commit with git commit -m "feat: add functional desktop ui and localization".

### Task 7: Add clipboard protection, QR/image import, and desktop integrations

Files:
- Create src-tauri/src/clipboard.rs, qr.rs.
- Create src-tauri/src/desktop/mod.rs, tray.rs, hotkeys.rs, startup.rs.
- Modify src-tauri/src/commands/mod.rs, src-tauri/src/lib.rs, src/pages/ImportPage.tsx, src/pages/SettingsPage.tsx, src-tauri/tauri.conf.json.
- Test src-tauri/tests/clipboard.rs, qr.rs, and desktop.rs.

Interfaces:
- copy_code(code: &str, ttl_secs: u32) -> AppResult<ClipboardToken>.
- clear_clipboard_if_owned(token: ClipboardToken) -> AppResult<bool>.
- decode_qr_image(path: &Path) -> AppResult<Vec<String>>.
- register_desktop_integrations(config: DesktopConfig) -> AppResult<DesktopCapabilities>.

Steps:
- [ ] Test clipboard cleanup only when the clipboard still equals the app-written value; preserve later user content and never log codes.
- [ ] Implement clipboard ownership tokens, scheduled cleanup, and cancellation on lock/exit.
- [ ] Test QR fixture decoding and malformed/empty images, then pass decoded URIs to the existing preview parser.
- [ ] Implement image import with a maintained Windows-compatible QR decoder. Report camera unavailable when no camera exists.
- [ ] Implement tray show/lock/quit, minimize-to-tray, startup option, notifications, and a favorites quick panel that refuses codes while locked.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml --test clipboard --test qr --test desktop. Mark unavailable Windows-only APIs BLOCKED.
- [ ] Commit with git commit -m "feat: add clipboard qr import and desktop integrations".

### Task 8: Add migration adapters and operational documentation

Files:
- Create src-tauri/src/import/google_migration.rs, aegis.rs, twofas.rs, andotp.rs, freeotp.rs, ente.rs.
- Create docs/migration-format-matrix.md, docs/security-model.md, docs/test-report.md, docs/known-limitations.md.
- Modify src-tauri/src/import/mod.rs, src/pages/ImportPage.tsx, README.md.
- Test src-tauri/tests/import_formats.rs and fixtures/import/*.

Interfaces:
- trait MigrationAdapter { fn detect(&self, bytes: &[u8]) -> bool; fn preview(&self, bytes: &[u8], password: Option<&SecretString>) -> AppResult<MigrationPreview>; }.
- Encrypted formats without a supplied password return UnsupportedEncryptedFormat; no bypass attempts.

Steps:
- [ ] Add non-secret public or generated fixtures and record provenance/license in migration-format-matrix.md.
- [ ] Test detection, field mapping, defaults, malformed input, encrypted-without-password rejection, and duplicate preview classification.
- [ ] Implement isolated adapters normalized into AccountInput and routed through preview/commit.
- [ ] Document trust boundaries, KDF, backup password loss, unsupported encrypted migrations, no-network behavior, and unverified Windows features.
- [ ] Run cargo test --manifest-path src-tauri/Cargo.toml --test import_formats.
- [ ] Commit with git commit -m "feat: add migration adapters and security documentation".

### Task 9: Release builds, screenshots, and final acceptance evidence

Files:
- Create .github/workflows/ci.yml and release.yml.
- Create src-tauri/icons/*, docs/release-notes.md, docs/acceptance-checklist.md, screenshots/*.
- Create scripts/verify-release.ps1 and scripts/verify-install.ps1.
- Modify src-tauri/tauri.conf.json, README.md, docs/test-report.md.

Interfaces:
- Release artifacts are Windows NSIS/MSI bundles when supported by the local Tauri toolchain.
- Evidence reports use PASS, FAIL, PARTIAL, BLOCKED, or NOT APPLICABLE.

Steps:
- [ ] Add Windows CI for pnpm install --frozen-lockfile, TypeScript, frontend tests, Rust tests, and pnpm tauri build.
- [ ] Set version, add real icons, configure NSIS/MSI without claiming signing.
- [ ] Run pnpm install --frozen-lockfile and pnpm tauri build. Record exact missing SDK/tool output if blocked.
- [ ] Launch the built app and capture real dashboard, code center, accounts, import, backup, security, and settings screenshots under screenshots/.
- [ ] Verify install, launch, vault creation, account add, lock/unlock, uninstall, reinstall, and data preservation. Mark upgrade/signing PARTIAL or BLOCKED without evidence.
- [ ] Update test-report.md, acceptance-checklist.md, known-limitations.md, release-notes.md, and README.md only with observed results.
- [ ] Commit with git commit -m "chore: add release packaging and acceptance evidence".

## Self-review checklist

- Spec coverage: Tasks 1-3 cover architecture, crypto, storage, migrations, lock lifecycle, and OTP; Tasks 4-5 cover accounts, URI import, backups, restore, and security audit; Task 6 covers UI, localization, accessibility, and real command wiring; Task 7 covers clipboard, QR, tray, hotkeys, startup, and notifications; Task 8 covers migration adapters and encrypted-format limits; Task 9 covers release, screenshots, installation evidence, README, and final acceptance.
- Placeholder scan: no TODO, TBD, or implement-later steps are used.
- Type consistency: AccountInput, AccountView, CodeView, AccountQuery, TimeState, RestorePreview, and command names are defined before later tasks consume them.
- Safety: no task writes remote services, publishes releases, rotates secrets, or claims signing without separate evidence.
