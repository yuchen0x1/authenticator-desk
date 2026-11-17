# Acceptance checklist — AUTHENTICATOR DESK 0.1.0

Evidence run on Windows (10.0.26300 x64) on 2026-11-18. Statuses: PASS /
FAIL / PARTIAL / BLOCKED / NOT APPLICABLE. Only observed results are
recorded; claims without evidence are marked PARTIAL or BLOCKED.

## Build and packaging

| Check | Status | Evidence |
|---|---|---|
| `pnpm install --frozen-lockfile` | PASS | installs cleanly, lockfile unchanged |
| TypeScript + frontend tests | PASS | `pnpm exec tsc --noEmit` clean; 11 tests pass |
| Rust tests | PASS | `cargo test` exit 0; 74 tests across 14 binaries |
| `pnpm tauri build` NSIS bundle | PASS | `src-tauri/target/release/bundle/nsis/AUTHENTICATOR DESK_0.1.0_x64-setup.exe` (3,592,529 bytes) |
| `pnpm tauri build` MSI bundle | PASS | `src-tauri/target/release/bundle/msi/AUTHENTICATOR DESK_0.1.0_x64_en-US.msi` (5,095,424 bytes) |
| Bundle SHA-256 | PASS | NSIS `4330f4e80720c3635f78b5df47e3b0ab386412efd46c4783108e184075781108`; MSI `12787d0a5220bd213c287e069dc731eca5f58524ff3a19e2c58e14a1b2baf51d` |
| Code signing | NOT APPLICABLE | no signing identity configured; no signing claim is made |
| Upgrade path | BLOCKED | no prior released version exists to upgrade from |

## Install and lifecycle (NSIS, packaged app)

`scripts/verify-install.ps1` output, run against the NSIS setup exe:

| Check | Status | Evidence |
|---|---|---|
| NSIS silent install exits 0 | PASS | installer `/S` exit code 0 |
| App binary exists after install | PASS | `%LOCALAPPDATA%\AUTHENTICATOR DESK\authenticator-desk.exe` |
| App launches and runs | PASS | process `authenticator-desk` present after launch, then stopped cleanly |
| Vault creation in packaged app | PASS | `vault.sqlite3` created in `%APPDATA%\com.authenticatordesk.app` (49,152 bytes) |
| Account add + code generation | PASS | Demo/alice@example.com card rendered a live 6-digit TOTP with countdown ring |
| Lock/unlock cycle | PASS | lock overlay hides all page content; unlock with master password restores the code center |
| Uninstall | PASS | install directory removed by `uninstall.exe /S` |
| Data preservation across reinstall | PASS | vault SHA-256 `36b02ca2…5e45196` unchanged across a silent reinstall |

## Functional checks (automated)

| Check | Status | Evidence |
|---|---|---|
| OTP vectors (RFC 4226/6238) | PASS | Rust test suite, SHA-1/256/512, HOTP counters 0–9 |
| Vault wrong-password rejection | PASS | `tests/vault.rs` |
| Backup wrong password / tamper / truncate | PASS | `tests/backup.rs` |
| Migration adapters incl. encrypted rejection | PASS | `tests/import_formats.rs` (12 tests) |
| Clipboard ownership cleanup | PASS | `tests/clipboard.rs` |
| Locked shell hides codes | PASS | `src/App.lifecycle.test.tsx` + packaged-app lock overlay observed above |
| Locale switching (zh-CN/zh-TW/en/ja) | PASS | `src/i18n/index.test.ts` |

## Screenshots

Captured from the release build (`src-tauri/target/release/authenticator-desk.exe`) at 1280×857 under `screenshots/`:

- `01-locked-create-vault.png` — first run, locked shell with vault creation
- `02-dashboard.png` — overview with zero-state cards
- `03-codes.png` — code center with a live TOTP card and countdown ring
- `04-accounts.png` — account list
- `05-import.png` — import page with URI textarea and migration file picker
- `06-backup.png` — encrypted backup and automatic backup forms
- `07-security.png` — security audit report (integrity, duplicates, plaintext scan, clock, ACL status)
- `08-settings.png` — preferences with appearance, security, and honest Windows Hello/update disclosures
- `09-locked-overlay.png` — lock overlay hiding page content

## Evidence hygiene

The acceptance vault was a synthetic test vault (master password
`authdesk-acceptance-2026`, one Demo account with the public demo secret
`JBSWY3DPEHPK3PXP`) created for this run in an empty app data directory. The
evidence hashes above were recorded before the data directory was removed
after the run; no user data existed or was affected.
