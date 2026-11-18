# Acceptance checklist — AUTHENTICATOR DESK 0.1.0

Evidence run on Windows (10.0.26300 x64) on 2026-11-18. Statuses: PASS /
FAIL / PARTIAL / BLOCKED / NOT APPLICABLE. Only observed results are
recorded; claims without evidence are marked PARTIAL or BLOCKED.

## Build and packaging

| Check | Status | Evidence |
|---|---|---|
| `pnpm install --frozen-lockfile` | PASS | installs cleanly, lockfile unchanged |
| TypeScript + frontend tests | PASS | `pnpm exec tsc --noEmit` clean; 18 tests across 8 files passed |
| Rust tests | PASS | `cargo test` exit 0; 35 library tests and 39 integration tests passed |
| `pnpm tauri build` NSIS bundle | PASS | `src-tauri/target/release/bundle/nsis/AUTHENTICATOR DESK_0.1.0_x64-setup.exe` (3,595,009 bytes) |
| `pnpm tauri build` MSI bundle | PASS | `src-tauri/target/release/bundle/msi/AUTHENTICATOR DESK_0.1.0_x64_en-US.msi` (5,099,520 bytes) |
| Bundle SHA-256 | PASS | NSIS `0993558f2ea6d97e6b09cff5b4a4d11fb4e64bd27dffac09ce01a90a25c067a5`; MSI `992c0d61d56246d78dada5ef85d4e01f562e827dceb54738318b1217be7fd541` |
| Code signing | NOT APPLICABLE | Get-AuthenticodeSignature reported NotSigned for both installers |
| Upgrade path | BLOCKED | no prior released version exists to upgrade from |

## Earlier NSIS install and lifecycle checks

`scripts/verify-install.ps1` and manual observations from the earlier 0.1.0 NSIS build are recorded below. The final rebuild received build and hash verification.

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

## Isolated desktop session

The release executable used a temporary `AUTHDESK_DATA_DIR` and one synthetic favorite account. These observations precede the final panel language correction.

| Check | Status | Observation |
|---|---|---|
| Open favorites | PASS | Ctrl+Shift+Q opened a separate window showing the favorite, its current code and countdown. |
| Lock and reopen | PASS | Ctrl+Shift+L hid the panel and showed the locked main window. After unlock, Ctrl+Shift+Q showed the favorite and a current code again. |
| Save desktop settings | PASS | The notification switch retained its saved value after navigating away and returning. It was restored to off. |
| Test notification | PARTIAL | The app confirmed submission. Visual delivery of a Windows toast was unverified. |
| Protected copy | PARTIAL | Clipboard ownership and expiry passed in the Rust tests. The copy button was skipped to preserve existing system clipboard content. |
| Tray and startup | PARTIAL | Tray preference logic passed its Rust test. Tray clicks and launch at Windows sign-in remain unverified. |

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

Captured from the release build (`src-tauri/target/release/authenticator-desk.exe`) under `screenshots/`:

- `01-locked-create-vault.png` — first run, locked shell with vault creation
- `02-dashboard.png` — overview with zero-state cards
- `03-codes.png` — code center with a live TOTP card and countdown ring
- `04-accounts.png` — account list
- `05-import.png` — import page with URI textarea, migration file input and QR image picker
- `06-backup.png` — encrypted backup and automatic backup forms
- `07-security.png` — security audit report (integrity, duplicates, plaintext scan, clock, ACL status)
- `08-settings.png` — preferences showing appearance and security controls
- `09-locked-overlay.png` — lock overlay hiding page content
- `10-quick-panel.png` — separate favorites window showing the synthetic account and countdown

Images 05, 08 and 09 are 1493×997. Image 10 is 543×747. The other six images are 1280×857.

## Evidence hygiene

The isolated desktop session used a synthetic vault. Its temporary data directory was removed after capture.
