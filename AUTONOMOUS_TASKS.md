# Autonomous task backlog

Updated 2026-11-21 (Asia/Shanghai). Source-review findings are not represented as live exploit or acceptance results. See [issues](AUTONOMOUS_ISSUES.md) and [test evidence](docs/test-report.md).

## P0 / P1 — delivery qualification

- [ ] Qualify the exact pushed commit through remote Security and CI, including both fresh-runner installer lifecycles. Historical green runs do not qualify new changes.
- [ ] Publish only after all release gates and required publication authorization. Verify the resulting tag, assets, signatures, checksums and public update manifest. A local unsigned build is not a published signed release.
- [ ] Exercise a genuine old-version → signed update → reopen-vault path on a disposable Windows installation. Keep installer families consistent and preserve synthetic data.

## P2 — correctness and resilience

- [ ] Isolate a valid future-`T0` account from the rest of a code batch. Add before/at-boundary regressions; retain single-account validation rather than silently using a wrong time.
- [ ] Retry ownership-aware clipboard expiry cleanup after transient contention, without clearing later user clipboard data; inject a fail-once clipboard backend.
- [ ] Reject malformed legacy numeric import fields rather than silently defaulting negative/string counters; preserve documented absent-field defaults and add format fixtures.
- [ ] Expose automatic-backup last attempt/error separately from last success and refresh status while the page remains open; never expose backup passwords or paths in errors.
- [ ] Qualify backup no-overwrite publication on FAT32/exFAT/network filesystems before claiming support; current publication uses hard links.
- [ ] Reproduce dependency-upgrade PR failures individually before accepting major action or `rfd` changes. Keep action SHA pinning and desktop compilation checks.

## P3 — platform and usability

- [ ] Decide whether a non-secret locale hint should localize cold locked startup; encrypted settings remain authoritative after unlock.
- [ ] Qualify publisher identity through a securely provisioned Authenticode certificate/signer if required. A thumbprint alone is not a signing credential. Do not purchase services or change external billing without authorization.
- [ ] Complete tray, shortcut conflict, autostart, notification and real clipboard acceptance in a disposable Windows session.
- [ ] Plan GTK/dependency migration before any Linux desktop release. Portable Rust CI is not Linux/macOS desktop acceptance.
- [ ] Address unmaintained dependency warnings through measured compatibility upgrades; do not label warning-only dependencies vulnerability-free.

## Completed in this round, locally verified

- [x] Follow current UTC after clock correction/resume with RFC-vector regressions.
- [x] Prevent competing cooperative processes from caching/writing with stale vault keys across password rotation.
- [x] Offer restricted recovery after authenticated account hydration failure and allow confirmed Replace from a valid backup over corrupt account rows.
- [x] Preserve localized failure reasons alongside named invalid import rows.
- [x] Separate NSIS/MSI CI lifecycles and require one explicit family per local disposable-VM smoke run.
- [x] Recheck seven real open-source projects using primary source files; preserve license boundaries and avoid copying code.

## 0.3.0 / 2026-11-21 — local closure and non-implementation backlog

The checked items below are local source/regression work only. They do not close remote CI, signed publication, native GUI acceptance or upgrade acceptance. Historical backlog entries above are retained as evidence of their earlier state.

### Completed in the 0.3.0 working tree and locally verified

- [x] Preserve secret-replacement intent after a failed account edit while clearing the sensitive input; require re-entry or explicit cancellation before retry.
- [x] Isolate future-T0 accounts from mixed available-code display batches while keeping protected single-account pre-T0 rejection.
- [x] Apply source-specific numeric import rules: reject present malformed Aegis/andOTP/2FAS values, retain omitted-field defaults, accept legal 2FAS nullable numeric fields, and preserve HOTP counter precision.
- [x] Replace per-copy clipboard expiry tasks with one cancellable worker, bounded backoff and token/sequence ownership checks.
- [x] Add automatic-backup health: retain the encrypted last-success timestamp in existing `last_run`; keep attempt/failure state local to the current session/configuration/generation; preserve the last-success record across restart; when a new configuration is enabled, reset its schedule/`last_run`; disabling keeps the last-success record. Refreshes preserve drafts and do not count as activity.
- [x] Execute the feature-gated desktop contracts in CI rather than only compiling the feature.

### Added follow-up backlog — not implemented in this round

No new P0 or proven cryptographic bypass is assigned. These are impact-ordered work items, not severity findings.

#### P2 — correctness, privacy and durability

- [ ] Close the shared account/category write-capacity and backup-size/retention-limit contract at every transactional write.
- [ ] Implement and accept Windows clipboard-history/cloud opt-out behavior; TTL ownership cannot revoke copies already retained by the OS.
- [ ] Inventory intermediate plaintext JSON/URL/QR/HMAC copies and add zeroization where the ownership/lifetime contract permits; do not claim a vault-at-rest break without evidence.
- [ ] Define zero-account category/settings backup-restore behavior and explicit Replace consequences in the UI.
- [ ] Classify non-conflict import-commit failures distinctly from duplicate rows and validation failures.
- [ ] Qualify no-overwrite backup publication and crash durability on FAT/exFAT/network filesystems; current hard-link behavior is not support evidence.
- [ ] Run a genuine old-version signed install → update → reopen-vault test on a disposable Windows system.

#### P3 — usability and publisher identity

- [ ] Improve backup default-filename collision UX.
- [ ] Allow editing imported accounts with no issuer.
- [ ] Decide and expose default algorithm/font-size controls without silently changing existing account semantics.
- [ ] Provision and verify Authenticode publisher identity through the authorized signing process; a thumbprint alone is not a credential.

### Release-owner fields still required

- [ ] Record the exact source commit, 0.3.0 tag and remote Security/CI results for this source (not a baseline SHA).
- [ ] Record release publication, installer-family lifecycle results, signed manifest/checksum/signature verification and the real signed-artifact test.
- [ ] Record complete native GUI/updater acceptance or its explicit disposition; the local broker-unavailable smoke does not close it.
