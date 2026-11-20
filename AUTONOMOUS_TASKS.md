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
