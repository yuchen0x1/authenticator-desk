# Test report

## 2026-11-21 — integrated safety evolution, local qualification

Observed on Windows x64, local date 2026-11-21 (Asia/Shanghai), Node 26.13.1, pnpm 11.20.0, rustc 1.99.1. Local Rust commands used `RUSTUP_TOOLCHAIN=stable` because the separate version-named installation is partially broken; the compiler versions were checked to match. This entry qualifies local source/artifacts only. Remote CI, push and publication state must be established separately.

The starting dirty tree already passed 194 frontend tests, 100 Rust core tests and 36 Node trust tests. Those inherited changes were preserved. The integrated results below include the additional regressions from this round, not 219 newly written tests.

| Check | Actual result / scope |
|---|---|
| Frontend full suite | `pnpm exec vitest run`: **219 passed in 25 files**, zero failures. Includes restricted recovery, stale/locked hydration, named invalid import rows, hidden-code, and Add/Edit settings-failure recovery without inert Retry/Lock controls, plus existing workspace regressions. |
| TypeScript / frontend packaging | `pnpm build`: no TypeScript errors; Vite built 147 modules. CSS 34.55 kB; JS 447.17 kB (gzip 136.69 kB). These are artifact measurements, not a performance speedup claim. |
| Rust core full suite | `cargo test --locked --manifest-path src-tauri/Cargo.toml`: **55 library + 53 integration tests passed** (108 total), zero failures. One subprocess helper is ignored in ordinary discovery and explicitly executed by the passing parent regression. |
| New native regressions | Forward/backward UTC correction and sleep/resume; authenticated confirmed replacement over damaged live account ciphertext; same-process and real-subprocess exclusion through password rotation, then takeover after owner drop. |
| Signed-updater boundaries | Four passed; `verify_staged_release` intentionally not run locally because it requires real signed release assets and the trusted verification key. Synthetic signature fixtures are not official-release evidence. |
| Signed-updater binary | `cargo check --locked --manifest-path src-tauri/Cargo.toml --features signed-updater --bin authenticator-desk`: passed. |
| Node release trust suite | Final `node --test scripts/*.test.mjs`: **38 passed**, zero skipped/failures, after isolated-runner and explicit local installer-family regressions. |
| Static syntax / formatting | Four workflow YAML files parsed; all three PowerShell scripts parsed without execution; changed production JavaScript passed syntax checks. Targeted OTP/updater Rust formatting and `git diff --check` passed. This round did not run actionlint locally. |
| Dependencies | Public-registry production and full pnpm audits reported zero advisories. Windows-target `cargo audit` exited 0 with six warnings listed below. No dependency changes occurred after these checks. |
| Final Windows packaging | `pnpm tauri build --ci --target x86_64-pc-windows-msvc -- --locked`: **NSIS and MSI built** from integrated source. Explicit target output avoided overwriting the original running executable. |
| Real native UI, synthetic data only | Interim build: create vault, preview one valid/one invalid URI, select/import exactly one account, display live code/countdown, hide and disable Copy, lock/re-unlock. Final rebuilt executable: reopen the same synthetic vault, preserve hidden preference, enable live code/countdown, lock and close successfully. Actual screenshots were emitted during acceptance, not fabricated or added to repository screenshots. No clipboard copy was invoked. |
| Recovery UI / installer lifecycle / live update | Corrupt-row recovery is covered by Rust integration and frontend mocked-IPC regressions, not end-to-end native corruption injection. Final installers were **not installed locally** because a real instance was running. Hosted NSIS/MSI lanes, real-key staging and genuine prior-version live upgrade require separate evidence. |

### Final local artifacts (unsigned, not published assets)

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Standalone `authenticator-desk.exe` | 13,853,696 | `2c8aa57a8049d9f787cb34c5d0e16294a96f12066725d82988a96e0a567be861` |
| NSIS `AUTHENTICATOR DESK_0.2.1_x64-setup.exe` | 3,664,292 | `0b4be1f51c8a468c226658376fa924831ecc26d7cdd46e5af4896e0b602f0aa5` |
| MSI `AUTHENTICATOR DESK_0.2.1_x64_en-US.msi` | 5,206,016 | `ecad9f420e0e2791a77455ca51ee10fc585c8f094503a9aca84f0de8c9a60aab` |

All three returned Authenticode `NotSigned`. These hashes identify local artifacts only; a future signed build will have different bytes. Build warnings remain for the `.app` bundle identifier convention and updater-only enum variants unused in an unsigned build.

### Dependency warning evidence

The audit reported unmaintained `chrono` 0.4.45 (RUSTSEC-2026-0543), `proc-macro-error` 1.0.4 (RUSTSEC-2024-0370), `zbus_names` 4.3.5 (RUSTSEC-2026-0371), `zvariant` 5.15.1 (RUSTSEC-2026-0370), `zvariant_derive` 5.15.1 (RUSTSEC-2026-0372), and unsound `glib` 0.18.5 (RUSTSEC-2024-0429). The inspected signed-updater Windows target graph contained no `glib`. This distinction is not a waiver for a future Linux desktop release and does not prove absence of dependency risk.

### Exposure audit — 1. Overview

The heuristic scanner checked final `dist/` (three files, one JS artifact) and the standalone PE executable. Web score: 100/A, no findings. Native score: 76/C, with one publisher-signature finding, one localhost-string finding and an informational PE record. Scores are relative triage signals, not security certification or proof of resistance to reverse engineering. The project is open source; concealing its algorithm is not a security boundary.

### Exposure audit — 2. Findings

| ID | Scanner severity | Observed finding | Review disposition |
|---|---|---|---|
| PE-SIGN | High | PE security certificate directory absent | Confirmed; `Get-AuthenticodeSignature` reports `NotSigned`. Publisher identity remains unqualified. |
| LEAK-INTRANET | High | Embedded `http://localhost:1420` development URL | Development configuration string, not a discovered private service or proof a listener runs in production. No secret endpoint claim. |
| PE-INFO | Info | PE32+, machine `0x8664`, DllCharacteristics `0xc160` | Parsed native hardening metadata; no PDB/missing-CFG finding in this scan. |

### Exposure audit — 3. Evidence and remediation

Scans used `python -I .../exposure_scan.py dist --profile web --json` and the final standalone executable with `--profile native --json`. The native scan reports an empty `DataDirectory[SECURITY]`; independent Windows signature inspection agrees. If publisher signing is required, securely provision the certificate/signer and set the pinned thumbprint, timestamp service and mandatory policy before the signed pipeline. Do not invent a certificate or treat a public thumbprint as a credential. The localhost URL originates in Tauri development configuration; removing it is not evidence of fixing an intranet compromise. No obfuscation or anti-debugging layer was added.

### Exposure audit — 4. Client-side limits

An offline authenticator necessarily computes OTPs and temporarily handles unlocked secrets locally; a compromised OS/debugger with equivalent user access is outside vault-at-rest protection. No scan can prove otherwise. There is no server subscription/authorization decision to move, and moving OTP secrets to a service would contradict the local-only product model. Long-lived updater private keys must remain in the protected release environment, never in distributed clients; only the public verifier belongs in the binary.

### Exposure audit — 5. Remediation order

P0: keep signing credentials and real vault/test data out of commits and artifacts. P1: complete exact-commit CI, signature and installer acceptance before promotion. P2: qualify optional Authenticode identity and documented platform/resilience follow-ups. A heuristic grade does not override any of those gates.

### Verification incidents and boundaries

A targeted Rust compile initially failed because `rusqlite` cannot read SQLite integers directly as `usize`; it was corrected to an `i64` read plus checked conversion, then targeted and full suites passed. A hash-reporting PowerShell snippet had a parser error; scans had already completed, and only corrected read-only hash collection was rerun. Native accessibility controls were resolved again from fresh full state after stale/delta lookups, not blindly clicked. No failing run was counted as a pass. Feature-specific generated-schema churn was compared to the protected starting state and removed without changing user source.

### First remote push and scanner feedback

The first safety commit `d6d502d6b94356e8cd226a3703d354586ddf9829` was fast-forwarded and pushed to `main`. Security run `44824258560` failed at gitleaks: `generic-api-key` matched the **public RFC 4226 test vector** embedded in `tests/privacy.rs`, not a user credential. Later security steps were skipped in that run and are not counted as passes. The fixture now explicitly encodes RFC Appendix D's public byte vector and additionally asserts HOTP counter 0 equals `755224`; no test assertions or scanner rules were removed, ignored or weakened. The follow-up commit needs its own complete remote run. Production artifacts are unchanged by this test/documentation-only correction.

GitHub also reported the already documented open moderate `glib` advisory (GHSA-wrw7-89jp-8q8g); it was not the cause of the gitleaks failure. The inspected Windows graph excludes `glib`, not all potential platforms.

The older observations below are historical and do not qualify the artifacts above.

## 2026-11-20 — prepared 0.2.1 repair verification

Observed locally on Windows 11 x64 using Node 26.13.1, pnpm 11.20.0 and Rust 1.99.1 (the installed `stable` toolchain contains that exact compiler; `RUSTUP_TOOLCHAIN=stable` was used locally because a separate version-named toolchain had a partial rustup installation). No GitHub push, tag, release, production-vault operation or installer execution was performed.

| Check | Status | Observed |
|---|---|---|
| Frontend regression suite | PASS | 84 tests in 15 files; includes cross-window hide/lock/stale responses, expiry/copy, hydration recovery, bulk filtering, explicit security-disable controls, import cancellation, restore review and dashboard refresh. |
| TypeScript and Vite | PASS | `pnpm exec tsc --noEmit` and `pnpm build`; repeated directly in the original workspace after offline frozen reinstall repaired relocated node_modules links. |
| Rust core | PASS | `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml`: 50 library + 47 integration tests, zero failures. Repeated after Control Flow Guard was enabled. |
| Signed updater + desktop feature | PASS / 1 IGNORED | `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --features signed-updater --lib --test desktop`: 54 library + 5 desktop tests passed. The real-key staged-release test remains deliberately ignored in this local run. |
| Desktop compilation | PASS | signed-updater binary check passed. |
| Node release/security regressions | PASS | 36 tests, including actual invocation enforcement helper, policy wiring, stable promotion, fail-closed audit reports and authentic/tampered minisign fixtures with in-memory ephemeral test keys. |
| Workflows and PowerShell | PASS | All four workflows passed actionlint 1.7.11; all three PowerShell verification scripts passed parser checks. MSI INSTALLDIR was verified against generated WiX. This is not installer execution evidence. |
| Final unsigned packaging | PASS | NSIS EXE 3,655,419 bytes; MSI 5,197,824 bytes. Both remain Authenticode NotSigned. Release linker uses no PDB reference and Control Flow Guard. |
| Native WebView2 interaction | PASS, scoped | An isolated temporary vault was created/unlocked, one synthetic favorite account saved, live TOTP observed, codes hidden in main and favorites windows, Copy disabled while hidden, lock removed content, and main-window close terminated the process after a panel had been created. Clipboard content, startup registration and notifications were not exercised. The last rebuild adds linker hardening and text/clock-display corrections; it was packaging-verified, not a second full UI acceptance run. |
| npm audit | PASS | Current JSON report had zero advisories. Strict parser accepted it; the historical broad exception was removed. |
| Rust dependency audit | PASS WITH WARNINGS | Current Windows-target audit exited 0 with six warnings: five unmaintained crates and target-irrelevant glib unsoundness. No claim of zero dependency risk. |
| Exposure checks | PASS WITH LIMITATIONS | Frontend produced no scanner findings. Final native scan no longer reports PDB or missing CFG; unsigned publisher identity remains. `localhost:1420` is the development-config string, not a discovered intranet service. Scanner score is not a security certification. |
| Actual signed 0.2.1 release / live prior-version upgrade / MSI lifecycle | NOT RUN | Requires configured signing identity, an authorized remote release and disposable OS acceptance. Automated gates and separate installer lanes are present, not execution evidence. |

Final unsigned artifact SHA-256:

- NSIS: `ce40d469bd6fec823a7433b773486730ebf6d28e920925a37819ec75c1adaa91`
- MSI: `95d7b01aba873abb3835907b11de093c571c06cf58fb8960c7326af69ff0229e`

One full frontend rerun exposed a document-language effect-timing race; language synchronization was moved to a layout effect, then the complete 84-test suite passed. Rust build/test retries during parallel editing were not counted as passes until the final integrated source compiled. The stale automatic-backup guard is implemented and reviewed, but a forced mid-run scheduling race was not separately injected.

The older observations below are historical and do not qualify newer artifacts.

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
