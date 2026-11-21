# AUTHENTICATOR DESK release notes

## v0.3.2

- Remember the authenticated language preference during locked cold starts, including the favorites panel; reject invalid hints and stale saves.
- Complete favorite-event and native file-filter translations and provide four complete README editions.
- Update validated native and GitHub Actions dependencies; retain security-update checks.
- Add safe password-rotation assertion diagnostics without changing production authentication or lease semantics.
- Vault schema, encryption, backup formats, application identifier and storage paths are unchanged.
- The Linux GTK `glib` advisory and the unconfirmed intermittent macOS regression remain explicitly disclosed. [Release scope, publication gates and limitations](v0.3.2.md). Published availability is determined by the [GitHub release](https://github.com/yuchen0x1/authenticator-desk/releases/tag/v0.3.2), not by this source entry.

## v0.3.1 (published 2026-11-21)

Published from tag commit `7ae5151e3041487f9e46ac4cec943fbd20553daf` with seven release assets. See the [GitHub release](https://github.com/yuchen0x1/authenticator-desk/releases/tag/v0.3.1). The installers have updater/minisign signatures but no Windows Authenticode publisher certificate. Later source changes do not replace these immutable release assets.

- Bind account-save completion to the originating vault session and dialog lifetime.
- Initialize edits from authenticated details; keep full HOTP counter precision and block Edit during a local advance.
- Allow empty optional issuer fields and preserve colons in plain Aegis native account names.
- Align T0 and expiry time bases in cards and favorites; pin release creation to the verified tag SHA.
- Includes the 0.3.0/0.2.1 source below, which was not released separately. [Release notes, verification and publication boundaries](v0.3.1.md)

## v0.3.0 (not released separately; included in v0.3.1)

- Show session-bound automatic-backup attempts/failures separately from last success, with safe periodic/manual refresh.
- Isolate future-T0 accounts from other displayed codes and show a localized pending state.
- Require explicit retry/cancel intent after failed secret replacement.
- Validate legacy numeric import fields strictly without losing 64-bit HOTP precision.
- Retry clipboard expiry safely using one cancellable worker and bounded backoff.
- Execute desktop-feature contracts in CI; retain signed release and independent installer gates.
- Includes the unreleased 0.2.1 safety fixes below. [Release details and known limits](v0.3.0.md)

## v0.2.1 (unreleased separately; included in v0.3.0)

- Limit Vitest to TypeScript test/spec files under `src/`, keeping Node.js test scripts out of the frontend runner.
- Run the release-context security tests independently with `node --test` in CI.
- Implement the trusted-invocation allowlist for this repository's version-tag pushes and matching release workflow reference.
- Add OS-held vault writer exclusion, UTC-correction handling, authenticated corrupt-account recovery, cross-window privacy and bounded migration/backup lifecycle fixes.
- Run NSIS and MSI lifecycle checks on separate fresh runners. [Patch details and qualification boundaries](v0.2.1.md)

## v0.2.0

At the 2026-11-21 remote inspection, this published release entry had **zero assets**. The feature description below is historical scope, not proof of available verified downloads or a working public update channel. Do not treat an empty release as an installable update.

- Check for signed Windows updates in Settings. Review the release notes, date and download size before installing.
- Release downloads include SHA-256 files and separate signed packages for EXE and MSI installations.
- Install manually when updating from v0.1.0. [Release details](v0.2.0.md) · [Update and recovery guide](../updating.md)

## v0.1.0

Windows x64 · 2026-11-18

## Desktop workflow

- Press Ctrl+Shift+Q to open favorite accounts with live codes and countdowns. The panel follows the saved language, hides on lock and reloads after unlock.
- Save tray preferences, startup behavior, notifications and global shortcuts in Settings. Send a test notification after enabling and saving notifications.
- Choose a QR image on the Import page to preview accounts. Copy a code from the account list or favorites panel using the protected clipboard.

## Highlights

- Offline-first desktop authenticator: encrypted SQLite vault protected by
  Argon2id and AES-256-GCM, with lock on demand, on idle, and on exit.
- TOTP and HOTP with HMAC-SHA1/SHA-256/SHA-512, 6/7/8 digits, configurable
  period and T0; verified against RFC 4226 and RFC 6238 test vectors.
- Imports: otpauth URI paste and file import with per-row
  preview/duplicate/invalid classification and transactional commit —
  including Google Authenticator migration URIs, Aegis plain JSON, 2FAS
  plain JSON, andOTP plain JSON, and plain otpauth lists (Ente plaintext).
- Encrypted `.authdesk` backups with independent Argon2id salt, restore
  preview, merge/replace confirmation, and retention-managed automatic
  backups.
- Security audit surface: integrity check, duplicate and invalid-config
  detection, plaintext-file scan, backup and time status.
- Clipboard protection with ownership tokens and TTL/lock/exit cleanup.
- Desktop shell: tray show/lock/quit, quick panel that refuses codes while
  locked, global hotkeys, autostart option, and notifications.
- Localization: zh-CN, zh-TW, en, ja with runtime switching.

## Known limits

- Encrypted third-party backups (Aegis/2FAS/andOTP/Ente) and FreeOTP are
  detected and rejected — no decryption in this release.
- Windows Hello, camera capture, signing, and auto-update are not
  implemented or verified.

## Bundles

Choose the EXE setup or the MSI installer from the release assets. Both are unsigned. GitHub lists the SHA-256 digest of each uploaded file. The [acceptance checklist](https://github.com/yuchen0x1/authenticator-desk/blob/v0.1.0/docs/acceptance-checklist.md) records the separate local build.
