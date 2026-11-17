# Release notes — AUTHENTICATOR DESK 0.1.0

Date: 2026-11-18. Channel: local build from this repository; no signing, no
auto-update channel, and no online verification exist in this release.

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

## Known limits (see docs/known-limitations.md)

- Encrypted third-party backups (Aegis/2FAS/andOTP/Ente) and FreeOTP are
  detected and rejected — no decryption in this release.
- Windows Hello, camera capture, signing, and auto-update are not
  implemented or verified.

## Bundles

Windows NSIS (`-setup.exe`) and MSI installers are produced by
`pnpm tauri build` and by the tagged `release.yml` workflow. Artifacts are
unsigned; hashes are recorded by `scripts/verify-release.ps1` output.
