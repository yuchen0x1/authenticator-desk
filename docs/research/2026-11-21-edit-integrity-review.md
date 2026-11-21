# Edit and migration integrity review — 2026-11-21

## Scope and method

This round started from `fix/public-fixture-scan` at `6d13f6d`, with source version 0.3.0. The verified public repository is `yuchen0x1/authenticator-desk`; its actual default branch is `main`. Two generated desktop/windows schema files were already dirty and are excluded from the delivery. Existing worktrees, user data, historical fixtures and running production processes are not disposable test resources.

The review read application lifecycle, account forms/cards, native account service, OTP time contracts, migration adapters, backup/security boundaries, release scripts/workflows, tests and historical evidence. Research below used official repository metadata, README/license files and the named implementation/specification files. No competitor application was built, run or benchmarked. Repository activity is not proof of product quality. No GPL/AGPL implementation, artwork, branding or private credentials were copied into this MIT project.

## Project baseline

- React/TypeScript renders redacted views; Rust owns secret material, OTP generation, encrypted storage and transactional mutations. Tauri exposes narrow native IPC, not an HTTP account server.
- SQLite schema 1, Argon2id and AES-256-GCM; per-vault OS writer lease; lock/generation/revision-bound pending operations.
- TOTP/HOTP, full-u64 decimal-string counters, explicit HOTP advance, search/categories/tags/favorites/order, migration previews and encrypted `.authdesk` backups.
- Four locales, two themes, Windows tray/shortcuts/quick panel and ownership-aware clipboard cleanup. Browser-only mode deliberately reports the missing native backend.
- Manual signed updates and isolated NSIS/MSI release gates already exist. Their presence is not evidence that a particular release passed them.

## Five official open-source references

| Reference and inspected revision | License checked | Files actually inspected | Useful design observation | Decision for this project |
|---|---|---|---|---|
| [Aegis](https://github.com/beemdevelopment/Aegis), `d6f4e5925a97e4e91593f1542085eae03432a759` | GPL-3.0 | README, `docs/vault.md`, `VaultRepository.java` excerpt | Android/Java; vault specification stores issuer/name independently. AtomicFile write/finish/fail preserves the old file on failed writes. | Preserve literal colons in Aegis native names. Consider atomic desktop-preference persistence separately. Do not copy GPL code. |
| [Ente Auth](https://github.com/ente/ente), `9754ed23a28d745651c5e3443e1805ee377b7849` | AGPL-3.0 | README, `mobile/apps/auth/lib/services/local_backup_service.dart` excerpt | Flutter/Dart; offline codes plus optional encrypted sync. Local backup distinguishes manual/automatic work and records success/retention after a successful write. | Keep this product local-only; retain explicit backup attempt/success semantics. Cloud sync is not a patch-sized addition. |
| [2FAS Android](https://github.com/twofas/2fas-android), `4ad83b2b94967993db55dd36dcb2e258646eb7b6` | GPL-3.0; graphics have separate terms | README, `core/cipher/src/main/java/com/twofasapp/cipher/backup/BackupCipherImpl.kt` | Kotlin modules separate cipher/backup responsibilities and explicit password/key-derivation/decryption outcomes. | Keep unsupported encrypted formats explicitly rejected; do not misreport them as empty successful imports or reuse artwork. |
| [2fast](https://github.com/2fast-team/2fast), `2d1007d57fb4fbf0878f8c03c83919681d347b43` | GPL-3.0 | README, `Project2FA.Shared/Services/Importer/BackupImporterService.cs` | C#/XAML local encrypted-file model, native Windows distribution, format-specific importer dispatch and explicit failure outcomes. | Keep adapter-specific contracts and per-row preview/commit validation; do not widen support merely because a format is recognized. |
| [KeePassXC](https://github.com/keepassxreboot/keepassxc), `cd42236540c1cfca332c0f051d981acfdacf7b4e` | COPYING: GPL-2 or GPL-3, with file/third-party exceptions; API metadata alone was inconclusive | README, COPYING, `src/gui/Clipboard.cpp` | C++/Qt local vault. Clipboard ownership/content checks prevent clearing a later user copy; Windows history/cloud exclusions are a separate OS concern. | Preserve current token/sequence checks. History/cloud exclusion needs independent Windows implementation/acceptance; it is not solved by reading this source. |

### Cross-project comparison

- **Interaction:** mobile projects emphasize QR and touch; native desktop references emphasize keyboard, clipboard and explicit file operations. This round improves existing Add/Edit and migration paths instead of transplanting a different interface.
- **Storage and trust:** a local encrypted vault, optional cloud sync and password-manager databases are different trust models. No server, telemetry, remote seed storage or subscription gate was introduced.
- **Security:** mature designs make write success, ownership and unsupported operations explicit. The selected work makes UI completion as session-specific as native mutation authorization.
- **Architecture:** retain Rust ownership of authenticated data and adapter-specific native validation. An edit starts from `AccountDetails.account`, not a potentially stale list view; later refreshes must not overwrite an active draft.
- **Release:** installer signing, updater signatures and publisher certificates are distinct. A release must identify an exact checked commit and pass its own remote gates.
- **Performance:** no comparative startup, CPU, memory or latency measurements were collected. Timer/IPC structure is a future profiling lead, not a confirmed bottleneck or claimed optimization.

## Prioritized findings and decisions

| Priority | Finding | Reproduction / risk | Disposition |
|---|---|---|---|
| P0 | No confirmed password bypass, secret disclosure or destructive vault corruption found in this scoped review | Absence of a finding is not a whole-product security proof | Preserve crypto/schema and existing fail-closed gates |
| P1 | Old account-save success closes a new draft after lock/unlock | Delayed create response survives old dialog lifetime | Fixed with vault-generation and monotonic dialog-generation checks; success/failure and same-session recovery regressions |
| P1 | Edit form submits stale list metadata, including HOTP counter | Details already contain a newer authenticated snapshot; editing notes could submit a rejected stale counter | Initialize all form fields once from matching authenticated details; reject wrong IDs; block Edit while local advance is pending |
| P2 | Aegis native names are truncated at `:` | `alice:prod` became `prod`, despite independent issuer/name fields | Fixed independently from the published format; parser, preview, commit and native GUI evidence |
| P2 | Empty optional issuer cannot be edited | Frontend required a value allowed by native validation/import | Removed only the issuer-required constraint; name/secret/OTP checks retained |
| P2 | T0 and expiry use different time bases for nonzero CodeView offsets | Synthetic positive/negative-offset contracts | Unified frontend time basis in card and quick panel. Current native clock emits offset zero; this is not new network time synchronization |
| P2 | Release creation omitted an explicit target SHA | Release metadata could refer to a moving default branch rather than the checked tag commit | Added `--target "$GITHUB_SHA"` with a policy regression, retaining tag/security/signature/draft/download gates |
| P2 | Maintenance lacks environment-scoped public verifier | Scheduled main job has no release environment, while the public key existed only there | Copied the same non-secret public key to repository variable scope; read-back equality verified; no private-key changes |
| P2 | `desktop.json` is written directly rather than atomically | Crash/write failure can lose preferences; no proof of vault corruption | Deferred: add temporary write, sync, atomic replace and injected-failure tests |
| P2 | Strict whole-repo Clippy/rustfmt fail on existing source | Tool output recorded; not current CI gates | Deferred with explicit evidence; no broad reformatting, lint suppression or reduced gate |
| P3 | Timer/IPC scaling lacks measurement | Structural observation only | Profile representative synthetic account counts before changing scheduling |

The backend already rejects HOTP counter rollback transactionally. This round fixes stale frontend submissions; it does not establish that persisted counters were previously allowed to regress. A save that completed natively is never replayed to compensate for a stale UI response.

## Compatibility and validation boundaries

No database migration, schema change, backup-format change, IPC signature change or dependency upgrade. Existing source version 0.3.0 becomes candidate 0.3.1; the last observed public stable entry was v0.2.0 with no assets. Unreleased source history is not a verified download channel.

See [the current test report](../test-report.md) for exact counts, native GUI evidence, unsigned local artifact hashes, dependency warnings and remaining release gates. Main push, a new tag, signed publication, clean-runner installation and genuine old-version update are separate milestones; none is inferred from local test success.

## Next bounded upgrade unit

1. Atomic desktop preferences with injected disk/rename failures, preserving current configuration format.
2. Consistent account/category capacity limits across CRUD, import and backup decoding.
3. Multi-QR migration completeness and zero-account restore UX, with explicit invalid/cancel/stale cases.
4. Windows clipboard history/cloud exclusion, qualified on a real OS without claiming retrospective cleanup.
5. Non-NTFS backup durability and real signed old-version update/reopen-vault acceptance.
6. Dedicated formatting/lint cleanup commit, with no semantic weakening and full regression afterward.
