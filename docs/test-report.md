# Test report

Recorded on Windows 11 x64 on 2026-11-18. The automated checks cover the current implementation. The acceptance checklist identifies the builds used for interactive checks.

## Commands and observed results

| Check | Command | Status | Observed |
|---|---|---|---|
| Rust unit + integration tests | `cargo test --locked --manifest-path src-tauri/Cargo.toml` | PASS | 35 library tests and 39 integration tests across 12 integration binaries passed (74 total) |
| OTP vectors | included in Rust run | PASS | RFC 4226 HOTP counters 0–9; RFC 6238 SHA-1/256/512 vectors for t=59, 1111111109, 1111111111, 1234567890, 2000000000, 20000000000 |
| Vault lifecycle | `tests/vault.rs`, `tests/storage.rs` | PASS | create, unlock, wrong-password rejection, lock, re-unlock, integrity check, no plaintext secret in SQLite bytes |
| Accounts | `tests/accounts.rs` | PASS | CRUD, search, categories, tags, favorites, batch delete/move/reorder, duplicate conflict |
| otpauth import | `tests/otpauth.rs` | PASS | valid TOTP/HOTP URIs, label encoding, algorithms, digits, period, counter, invalid scheme/base32/params |
| Encrypted backup / restore | `tests/backup.rs` | PASS | round-trip, wrong password, tamper, truncated file, version mismatch, merge/replace previews |
| Security audit | `tests/security_audit.rs` | PASS | audit report runs over a real vault; SHA-1 alone is not reported weak |
| Clipboard | `tests/clipboard.rs` | PASS | token ownership, user content preserved, TTL cleanup |
| QR decode | `tests/qr.rs` | PASS | synthetic QR fixtures decode; malformed/empty images rejected |
| Desktop integrations | `tests/desktop.rs` | PASS | tray availability gates close-to-tray and minimize-to-tray preferences |
| Migration adapters | `tests/import_formats.rs` | PASS | 12 tests: Google migration batches, Aegis/2FAS/andOTP field mapping + defaults, steam rejection, duplicate classification, transactional commit, encrypted-format rejection with and without password |
| Frontend tests | `pnpm exec vitest run` | PASS | 18 tests in 8 files passed, including desktop settings, window routing, saved panel language, and late code responses after lock |
| TypeScript | `pnpm exec tsc --noEmit` | PASS | no errors |
| Frontend production build | `pnpm build` | PASS | TypeScript and Vite completed successfully |
| Desktop feature | `cargo check --locked --manifest-path src-tauri/Cargo.toml --features desktop --bin authenticator-desk` | PASS | MSVC check succeeded with one unused Rust import warning |
| Windows packaging | `pnpm tauri build --ci` | PASS | Produced the x64 NSIS and MSI installers. Tauri also warned about the bundle identifier ending in `.app`. |

## Desktop and installer evidence

In the isolated release executable, the global shortcut opened the favorites panel with a live TOTP and countdown. Locking hid the panel. Unlocking and reopening it restored the account. The application acknowledged the test notification after the saved notification setting was enabled. A component test covers the final correction to the panel's saved language.

The earlier NSIS install, launch, reinstall and uninstall checks apply to the earlier 0.1.0 bundle. The installer hashes and remaining manual checks are listed in the [acceptance checklist](acceptance-checklist.md).

## v0.2.0 Windows release qualification

| Check | Status | Observed |
|---|---|---|
| Signed updater build | PASS | Built x64 NSIS and MSI bundles with separate Tauri updater signatures using the configured release key. |
| Release staging | PASS | `scripts/release.mjs` staged versioned installers, signatures, `SHA256SUMS.txt`, `release-assets.json` and signed `update.json`; local verification matched the bytes and metadata. |
| Signed manifest and installer signatures | PASS | The staged-release test verified the minisign envelope and both installer signatures, checked SHA-256 and version-bound trusted comments, then rejected tampered bytes and payloads. |
| Tauri manifest compatibility | PASS | The signed-updater unit test deserialized the static platform manifest through Tauri's `RemoteRelease` type. |
| Updater URL and integrity guards | PASS | Four non-ignored signed-updater tests cover accepted/rejected URL hosts, malformed signatures, SHA-256, and release metadata shape. |
| NSIS install, launch, reinstall and uninstall | PASS | The v0.2.0 installer started successfully. Reinstall and uninstall preserved a synthetic data marker outside the application directory. |
| MSI install and uninstall | NOT RUN | The MSI was built and signed. This local run exercised the NSIS lifecycle only. |
| Live update discovery and installation | NOT YET AVAILABLE | v0.2.0 is built locally but is not published from this verification record. The installed v0.1.0 build has no updater. |
| GitHub Actions run for this change | NOT RUN | `actionlint` passed locally. Remote CI and signed-tag release checks require the branch and tag to be pushed. |
| npm production dependency audit | PASS | `pnpm audit --prod --registry=https://registry.npmjs.org --audit-level high` found no high or critical production advisory. |
| npm development dependency audit | PARTIAL | `GHSA-fc5q-39xc-vjpf` remains on the development-only `picomatch@4.0.7` path. The public npm registry has not published a fixed package. CI permits only this advisory and fails on any other finding. |
| Rust dependency audit | PASS WITH WARNINGS | `cargo audit --file src-tauri/Cargo.lock --no-yanked --target-os windows --target-arch x86_64` exited successfully with six allowed advisories. Five are unmaintained crate warnings and one concerns glib 0.18.5, which is not used by the Windows runtime. |
| Formatting and workflow syntax | PASS | `rustfmt --edition 2024 --check src-tauri/src/update.rs`, `actionlint` on all four workflows, `node --check` on release scripts, and `git diff --check` passed. |
