# Test report

## 2026-11-21 — 0.3.1 edit/migration integrity qualification

This entry records the current round, starting at `6d13f6d` with source version 0.3.0 and preparing 0.3.1. It supersedes neither historical results nor their limitations. At the time of recording, the code has not yet been pushed or tagged; remote CI and publication must be observed separately for the delivered SHA. The five-project design/format investigation and prioritized backlog are in [this review](research/2026-11-21-edit-integrity-review.md).

### Final current-source local checks

| Check | Observed result and boundary |
|---|---|
| Frontend | **248 passed / 26 files**, zero failures; includes old-save success/failure across lock/unlock, same-session workspace recovery, authenticated-details editing, wrong-account rejection, optional issuer, HOTP advance/Edit exclusion and positive/negative CodeView offset contracts. |
| Node release/security scripts | **40 passed**, zero skipped/failures. New exact-tag-SHA policy test first failed, then passed after the minimal workflow fix. |
| Rust default suite | **64 library + 64 integration = 128 passed**, zero failures. One subprocess helper is ignored in ordinary discovery and invoked by the passing parent lease test. Aegis integration target now has 24 tests. |
| Desktop/OTP/scaffold features | **9 passed** with `--features desktop,signed-updater --test desktop --test otp_commands --test scaffold_contract`; some overlap default tests. |
| Signed-updater boundaries | **4 passed / 1 ignored** with `--features signed-updater --lib update::signed::tests`. The ignored real-key staged-release verifier requires actual signed release assets; it was not replaced by a mock success. |
| TypeScript / Vite | `pnpm build` passed; 148 modules; CSS 34.79 kB (gzip 8.24 kB), JS 453.28 kB (gzip 138.79 kB). These are sizes, not measured speed improvements. |
| Version / staged hygiene | `node scripts/release.mjs version` verified 0.3.1 in all four version fields; staged `git diff --check` passed. Gitleaks 8.24.3 `git --pre-commit --staged --redact` found no leaks in this patch. This is not a clean historical/full-tree scan claim; the seven historical public-vector matches recorded below remain separate. |
| Windows packaging | `RUSTUP_TOOLCHAIN=stable pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis,msi -- --locked` exited 0 and produced both installers. |
| npm audit | Official-registry `pnpm audit --registry=https://registry.npmjs.org --json`, followed by the fail-closed report verifier: no advisories, no active exceptions, both exit 0. The configured mirror did not implement the audit endpoint, so its error was not treated as a clean report. Dependencies were not changed afterward. |
| Rust dependency audit | Completed with **six allowed warnings**, not zero advisories: unmaintained chrono 0.4.45, proc-macro-error 1.0.4, zbus_names 4.3.5, zvariant/derive 5.15.1; glib 0.18.5 unsoundness. See advisory IDs in the historical section below. The Windows target excludes glib; Linux desktop is not qualified. |
| Strict non-CI lint checks | Whole-repository `cargo fmt --check` **failed** on existing formatting. `cargo clippy --all-targets --features desktop,signed-updater -- -D warnings` **failed** with 14 library / 15 library-test diagnostics in unchanged accounts, backup, commands, import/base64, import/otpauth, OTP and security-audit source. No suppression, broad reformatting or gate reduction was applied. |

The version-named Rust installation is partially broken (`missing manifest` / existing cargo component conflict). Local commands used installed `stable`, verified as the same pinned rustc 1.99.1 commit `58cf8f9c3`; the repository toolchain pin was not changed. The first packaging command failed because the user's existing process held the default release EXE open. That process was not stopped. Only the explicit-target retry succeeded.

The integrated regression log is `call_6RBKt1v9wc9sIPh6iWAcQH1J-stdout.log`; packaging log is `call_XUWWV7W2tChfXdO6VlLkZaDI__rk16__dzoBwLga6afds_ip-stdout.log` in this task's local log directory. Failing runs are not counted as successful runs.

### Real native UI acceptance — synthetic data only

The explicit-target 0.3.1 EXE was launched with a new temporary `AUTHDESK_DATA_DIR` and independently identified process. The existing default-target app and its vault were not operated on. Using the real Windows accessibility tree and WebView2 UI, the following completed:

1. Create a new synthetic vault with a test-only password.
2. Add an account with empty issuer, account name `synthetic:public-vector`, and the public RFC 4648 `foobar` Base32 vector; edit only its notes without replacing the secret.
3. Select a local synthetic plain Aegis file; preview/commit two rows named `alice:prod` and `public:hotp`. Both names remain intact in preview and the resulting account cards.
4. Explicitly advance the HOTP account from counter 0 to 1, open Edit, observe counter `1`, change notes and save successfully.
5. Lock: account names and codes disappear. Unlock: reopen HOTP Edit and observe counter `1` and the saved note together.
6. Cancel editing and lock the synthetic vault at the end.

Real screenshots were emitted for new-vault, codes/import result, locked state and persisted HOTP edit state. They were not fabricated or substituted with browser mocks. No real account secret was used; no copy-code action, notification, autostart registration, production backup or production restore was performed. Browser-only backend-unavailable behavior is not evidence of native functionality. Native UI acceptance does not replace deterministic delayed-response regressions, clean-runner installers or a genuine old-version signed update.

### Local final artifacts — not official release assets

Paths are below `src-tauri/target/x86_64-pc-windows-msvc/release/`. All three are Authenticode **NotSigned**, independently checked with Windows signature inspection. No updater private key was provided to this local build.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `authenticator-desk.exe` | 13,881,856 | `f09e4191c6f325a8273156e3bec9fe37d0ffb2648b251e0f4c4f43438883ab57` |
| `bundle/nsis/AUTHENTICATOR DESK_0.3.1_x64-setup.exe` | 3,673,175 | `61b419d8a03ed41111ff9194a56aad23a8ce3567946df9c4450ccf4074adda71` |
| `bundle/msi/AUTHENTICATOR DESK_0.3.1_x64_en-US.msi` | 5,214,208 | `722f5e5d3b3feb97f6824cfd1adcfc744b809ada88f5c0000291e594113b9dc3` |

These hashes identify only the inspected local builds. No local installer was installed over the existing application; fresh-runner NSIS/MSI lifecycle, signing and download verification remain separate remote gates. The existing `release-assets/` directory contains older artifacts and was not reused or emptied.

### Exposure audit — 1. Overview

The read-only heuristic scanner rechecked final `dist/` (3 files, one JS artifact) and the final standalone EXE. Web: 100/A, no findings. Native: 76/C, two high labels and one informational record. These relative triage scores are not security certification, exploitability proof or a claim of resistance to reverse engineering.

### Exposure audit — 2. Findings

| ID | Scanner severity | Evidence | Reviewed meaning |
|---|---|---|---|
| PE-SIGN | High | PE `DataDirectory[SECURITY]` empty; Windows signature status NotSigned | Local publisher identity is unqualified. Minisign update signing is separate. |
| LEAK-INTRANET | High | Embedded `http://localhost:1420` development-configuration string | Not evidence of a private-network service or a production listener. |
| PE-INFO | Info | PE32+, machine 0x8664, DllCharacteristics 0xc160 | Native header metadata; no new hardening claim. |

### Exposure audit — 3. Evidence and remediation

Commands used `python -I .../exposure_scan.py dist --profile web --json` and the final EXE with `--profile native --json`. Authenticode was checked separately for the EXE and both installers. A real publisher certificate, pinned thumbprint, timestamp service and mandatory policy are needed before claiming publisher signing; no certificate was invented or secret read. The localhost finding is a false inference about network exposure, not a reason to move local vault data to a gateway. No obfuscation, anti-debugging or new concealment layer was added.

### Exposure audit — 4. Client-side limits

This is open-source offline authenticator software. Its algorithms are not secrets. Unlocked seed access by a compromised OS/debugger cannot be solved with client obfuscation; do not replace vault-at-rest protection with a promise of runtime secrecy. There is no server subscription gate to relocate. Release private keys remain in the protected build environment; only the verifier public key belongs in clients. A static scan does not prove those runtime boundaries.

### Exposure audit — 5. Remediation order

P0: keep real credentials, vaults and private signing keys out of the repository and artifacts. P1: pass exact-source hosted Security/CI, real-key asset verification and separate installer lifecycle gates before release promotion. P2: qualify publisher signing, clipboard history/cloud exclusions and platform-specific resilience independently.

### Remote configuration repair and outstanding delivery gates

The scheduled Maintenance job has no `release` environment, so the public verifier that existed only at environment scope was unavailable there. The same **non-secret** `AUTHDESK_UPDATER_PUBLIC_KEY` was set at repository-variable scope and read back; equality with the release-environment value was verified (SHA-256 fingerprint `93cad7603c5953f268d3dba13d14938dcfd2210ceef8de4f7b8844cb26f8618d`). Private keys, secrets, environment deployment restrictions and signature verification were not changed. This fixes variable visibility, not the previously observed empty v0.2.0 release assets.

Commit/main push, exact-source hosted results, a new tag, release creation, seven downloaded signed assets, stable update metadata, and a genuine prior-version signed update must each be recorded separately. At this entry's recording point, no v0.3.1 tag or Release has been created. Do not infer publication from a candidate release-notes file.

## 2026-11-21 — hosted Security failure and public-fixture correction

The first new-source push reached `main` at `7fbb53785317723c848371601afdcf3145c6f89e`. [Security run 44833817210](https://github.com/yuchen0x1/authenticator-desk/actions/runs/44833817210) failed: Gitleaks 8.24.3 flagged one public test-vector literal in the new `AccountDialog.test.tsx` assertion (`generic-api-key`). This is a real failed gate, not evidence of a leaked user credential. It was not ignored or rerun unchanged. The test now uses the public RFC 4648 Base32 `foobar` vector through a named fixture; production code and scan rules are unchanged. The dialog suite passed **17/17**, TypeScript and diff checks passed, and same-version Gitleaks scanning of that corrected file found no leaks.

A portable official Gitleaks 8.24.3 Windows archive was downloaded into a new isolated directory and verified against the release checksum (`3f1a35578631dbfe633cc5b49e6c906e55ff14a4bfd7336a10fb27fe33b6dcd2`); it was not installed globally. A broader scan of the 268 tracked-file snapshot found **seven existing public-vector matches in six files**, all unchanged from baseline `60dae05`: `src/lib/schemas.test.ts`, the Aegis/andOTP/2FAS plain fixtures, and Rust `qr.rs`/`accounts.rs`. That broader scan exited 1; it is not reported as clean. Original format fixtures were retained, and no allowlist, history rewrite or scan-rule reduction was added. A clean incremental gate cannot establish a clean historical/full-tree scan.

GitHub's open Dependabot alert 3 is `glib` 0.18.5, `GHSA-wrw7-89jp-8q8g` / `RUSTSEC-2024-0429`, not a newly introduced dependency. `cargo tree --locked --target x86_64-pc-windows-msvc --features signed-updater -i glib` printed no reverse dependency for that target. The alert remains open; this target observation does not fix the dependency or establish Linux desktop safety. New-source hosted checks after the fixture correction still require separate observation. No v0.3.0 tag or release is established by this entry.

## 2026-11-21 — 0.3.0 final local regression and explicit-target packaging

This append-only entry records this round's local results. It does not establish remote CI for the exact pending source, a signed release, a Git tag, publication, complete native GUI acceptance, or an old-version live upgrade. No baseline-SHA remote CI result is used to qualify the newer source. The app source version check reported 0.3.0. Local Windows build and regression stdout logs both exited 0.

| Check | Observed result / scope |
|---|---|
| Frontend suite | Vitest: **238 passed in 26 files**, no failures. |
| Node release-trust tests | **39 passed**, zero skipped or failed. |
| TypeScript / Vite | `tsc --noEmit` and production build passed; 148 modules transformed. Output: CSS 34.79 kB (gzip 8.24 kB), JS 452.68 kB (gzip 138.62 kB). These are artifact sizes, not a speed claim. |
| Version / diff hygiene | 0.3.0 version check and `git diff --check` passed after restoring generated-schema state; the original 2FAS fixture has no diff. |
| Rust default suite | **64 library + 63 integration = 127 passed**, zero failures. One subprocess lease helper is ignored in ordinary discovery and explicitly invoked by the passing parent regression. |
| Desktop feature | **5 passed**. Two default desktop integration tests repeat in this feature run; they are not additional unique tests. |
| Signed-updater boundaries | **4 passed**; one staged-real-signature artifact test was intentionally ignored because the local run did not have the required release-key artifacts. |
| Targeted Rust formatting | `rustfmt --edition 2024 --check` for the clipboard production/test files and future-T0 test passed. This is not a whole-repository formatting claim. |
| Automatic-backup status semantics | Encrypted last-success survives restart and disable; attempt/failure is session/configuration/generation-local; enabling a new configuration resets schedule/`last_run`. Covered by the local regression suite. |
| Dependency audit | `pnpm audit` reported no known vulnerabilities. Windows-target Rust audit exited 0 with six allowed warnings; exact findings are listed below. An exit-0 allowlist is not a zero-advisory or vulnerability-free result. |
| Explicit-target Windows build | `pnpm tauri build --ci --target x86_64-pc-windows-msvc -- --locked` completed and built both NSIS and MSI. The packages are unsigned local outputs, not formal release assets. |

### Final explicit-target local artifacts

| Artifact | Bytes | SHA-256 | Authenticode |
|---|---:|---|---|
| `authenticator-desk.exe` | 13,874,176 | `DFE1B6BF74B8900AE5D942A1A1BABD3828E7D3054842A796D7404671B190130A` | NotSigned |
| NSIS `AUTHENTICATOR DESK_0.3.0_x64-setup.exe` | 3,673,264 | `3249CD1457FCD29392C62AC3BB7C53501AA9BE6FF1B8C70284E55FDFAD3D75BA` | NotSigned |
| MSI `AUTHENTICATOR DESK_0.3.0_x64_en-US.msi` | 5,210,112 | `F79E73340D656FB12616B849087C53BC3EC5922AAC8516406F0452D3F7797CB2` | NotSigned |

Hashes identify only these local artifacts. They are not a published signed build and cannot substitute for a signed release manifest or installer signatures.

### Native and exposure checks

The final explicit-target app was started with an isolated data directory. The process responded, its title matched the expected application title, and the isolated directory remained empty. No vault was created or unlocked; the real user process was not touched. This is a process-start smoke test only, not a GUI workflow or upgrade acceptance.

The native UI broker remained unavailable (connection/socket unavailable while starting), so full native clicks and complete desktop acceptance were not performed. A real localhost:1431 browser DOM check, without mocks, showed the desktop security backend unavailable and instructed opening the desktop app. It does not establish browser-mode support. No genuine prior-version signed install → update → reopen-vault test was run.

A heuristic scan of final `dist/` (3 files, 1 JavaScript artifact) scored 100/A with no findings. The native executable scored 76/C and reported: **PE-SIGN** (no Authenticode publisher signature), **LEAK-INTRANET** (the embedded `http://localhost:1420` development-configuration string; not evidence of a real intranet service or active listener), and informational **PE-INFO** (PE32+, machine `0x8664`, `DllCharacteristics 0xc160`). These are triage signals, not a security certification, exploitability assessment or proof of resistance to reverse engineering. The project is open source; algorithm concealment is not a security boundary. Updater/minisign signing, if configured for a release, is distinct from Authenticode publisher identity.

### Dependency-audit warnings (captured task log)

The audit completed successfully and reported no known npm vulnerabilities. `cargo audit` scanned 578 crate dependencies and exited 0 with **six allowed warnings**:

- Unmaintained `chrono` 0.4.45 (`RUSTSEC-2026-0543`), `proc-macro-error` 1.0.4 (`RUSTSEC-2024-0370`), `zbus_names` 4.3.5 (`RUSTSEC-2026-0371`), `zvariant` 5.15.1 (`RUSTSEC-2026-0370`), and `zvariant_derive` 5.15.1 (`RUSTSEC-2026-0372`).
- `glib` 0.18.5 unsound iterator implementations (`RUSTSEC-2024-0429`). The prior Windows-target graph review found `glib` absent from that target; this does not qualify Linux desktop use.

Audit stdout was captured in the task session's `call_iHHsDAjsdYIMhPpwfCg2a1hQ__rk16__DOdaNMvdesq1qlO7-stdout.log`. Do not treat allowed warnings as fixed or as proof of no dependency risk.

### Build warnings and release-owner follow-up

Observed build warnings were: Tauri bundle identifier `com.authenticatordesk.app` ends in `.app` (not recommended due to macOS bundle-extension collision), and unsigned-build Rust `UpdateError` variants `Busy`, `Network`, `InvalidRelease`, `Incompatible`, `Integrity`, and `Install` are dead code. The regression log also emitted Git LF-to-CRLF notices for generated desktop/windows schema and clipboard files; generated desktop/windows schema files were restored byte-for-byte from backup, the transient ACL/updater generated difference was removed, and the post-restore diff check passed. The implementation, including the follow-up public-vector fixture below, is committed locally as `8768935d538a1a85f7076f7fd7f10ca41ddbffff`; remote delivery is not established by this local snapshot.

Before any release claim, the release owner must append exact-source commit and tag state, new-source remote Security/CI run URLs/results (including NSIS/MSI clean-runner lifecycle and signed artifact checks), publication/assets/signature/checksum verification, completed native GUI acceptance, and genuine old-version signed update/reopen-vault evidence where required. Do not use remote CI authenticated against a baseline SHA as evidence for this source. Current Git/Tag/Release state is **not complete**.

### Follow-up test-fixture scanner hygiene

After the integrated run, the future-T0 test fixture was changed to derive its public RFC test seed with `otp::encode_base32(...).as_str()` rather than embedding the known scanner-triggering literal. This public test vector is not a user credential or leak. The first targeted compile exposed an `E0277` serialization mismatch for `Zeroizing<String>`; using `.as_str()` corrected the fixture, after which `cargo test --test future_t0` passed **1/1**, targeted `rustfmt --edition 2024 --check` passed, and `git diff --check` passed. This changed test fixture only; production code and the already-built artifacts are unchanged. A local gitleaks executable was unavailable; a basic added-line screen for private-key, GitHub-token and AWS-ID patterns found no matches, but that is not equivalent to Gitleaks or hosted Security. The exact current source still requires its own remote Security run.

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
