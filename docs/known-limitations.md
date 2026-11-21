# Known limitations

Updated on 2026-11-21 (Asia/Shanghai). Dated historical acceptance remains in the test report.

## Open dependency security advisory

GitHub Dependabot alert [#3](https://github.com/yuchen0x1/authenticator-desk/security/dependabot/3) remains **open** for [`GHSA-wrw7-89jp-8q8g` / `RUSTSEC-2024-0429`](https://rustsec.org/advisories/RUSTSEC-2024-0429.html): `glib 0.18.5` has an unsound `VariantStrIter` implementation. The Linux GTK/WebKitGTK/tray dependency graph includes this package. It is not present in the `x86_64-pc-windows-msvc` all-features dependency graph; that target-specific observation is not a fix to the lockfile or Linux dependency chain.

On 2026-11-21, an isolated update to compatible Tauri 2.12.7 / Wry 0.57.3 / Tao 0.37.3 still resolved GLib 0.18.5. GTK 0.18 requires GLib `^0.18`, so a standalone GLib bump to 0.20+ cannot satisfy the current graph. Coordinated upstream migration is tracked in [Tauri](https://github.com/tauri-apps/tauri/pull/16170), [Wry](https://github.com/tauri-apps/wry/pull/1843), [tray-icon](https://github.com/tauri-apps/tray-icon/pull/369) and [muda](https://github.com/tauri-apps/muda/pull/403). No unreviewed dependency fork or advisory dismissal is used here.

`cargo audit` classifies this as an `unsound` warning. A successful default audit/CI exit code does **not** establish that this issue is resolved: `cargo audit --deny unsound` still fails. Other dependency upgrade PRs do not fix this advisory. Do not interpret Windows-only qualification as Linux desktop security approval.

## Encrypted third-party migration formats

- Encrypted Aegis vaults, encrypted 2FAS services, andOTP `.aes`/`.gpg`,
  encrypted Ente exports, and FreeOTP backups are detected and rejected with
  `unsupported_encrypted_format`. No decryption, password prompting, or
  bypass is implemented; adapters will be added only with verified test
  vectors. Rejection happens whether or not a password is supplied.
- FreeOTP 2.x is rejected in all shapes: token material is Android
  Keystore-bound and has no portable plaintext export.

## OTP semantics

- STEAM tokens (Aegis/2FAS/andOTP) are reported as unsupported rows instead
  of being converted; Steam uses a custom code alphabet.
- The Google Authenticator migration format carries no period; imported
  TOTP entries use the default 30-second period.
- HOTP counters are persisted monotonically through the explicit advance
  command; there is no automatic counter resync against a server.

## Platform features not verified in this release

- Windows Hello is not implemented or verified; the settings page states
  this instead of exposing a toggle that would pretend otherwise.
- Camera capture for QR import is not implemented; image-file QR import is
  the supported path.
- Windows installers lack an Authenticode publisher certificate. Updater signatures use the project release key. v0.1.0 requires a manual upgrade.
- The isolated Windows session exercised the favorites and lock shortcuts. Manual checks remain open for tray clicks, shortcut conflicts with other apps, startup at sign-in, Windows toast visibility and protected copy through the system clipboard. See the [acceptance checklist](acceptance-checklist.md).

## Backup constraints

- Losing a backup password permanently loses that backup; there is no
  recovery key, escrow, or password reset.
- Automatic backups are encrypted with the same `.authdesk` envelope and
  require the backup password to restore; they are not a DPAPI-managed
  convenience copy.
- Automatic retention only deletes files proven by this vault's encrypted
  ownership manifest and digest. Legacy-unclaimed, foreign, manual or changed
  files are intentionally preserved. Non-Windows builds currently preserve
  all backup files because portable exact-handle deletion is unavailable.
- New automatic backups add authenticated ownership metadata; current code
  reads legacy backups, but older app versions are not promised to read new
  automatic-backup metadata. Keep a prior known-good backup before downgrading.

## Operational

- The update check is user-initiated. TOTP/HOTP uses local time and there is no sync service, account, or telemetry.
- Recovery requires a known-good encrypted backup and its password. A valid
  unlock/settings load with damaged account rows offers a restricted
  Backup/Security surface; confirmed Replace does not decrypt outgoing account
  rows. It does not repair arbitrary SQLite corruption, a damaged vault header,
  or encrypted settings that cannot be loaded. Preserve the original file and
  restore to a separate vault when normal recovery cannot start.
- The single-writer lease protects cooperating clients using the same canonical
  path, including while their UI is locked. Older versions, deliberate hard-link
  aliases and external replacement of database/sidecar files are outside that
  contract. Fully exit all versions before upgrading; do not remove `.lock`
  sidecars while an owning process is alive.
- Source review identified remaining follow-ups, not completed acceptance:
  a future `T0` entry can fail an entire code batch before its start time;
  malformed numeric fields in some legacy JSON imports can fall back to defaults;
  transient Windows clipboard contention has no scheduled-clear retry; automatic
  backup failures lack a last-attempt/error UI; and cold locked launch uses the
  default locale until encrypted settings are available. See the prioritized
  [backlog](../AUTONOMOUS_TASKS.md).
- Backup publication uses hard-link no-overwrite semantics. FAT32/exFAT and network
  filesystem compatibility have not been qualified; do not assume they work.

## 0.3.0 / 2026-11-21 addendum

This addendum records the current local-source boundaries; the dated historical entries above remain intact.

- Automatic-backup last-success metadata is encrypted and survives restarts and disabling the feature. Attempt/failure status is limited to the current session/configuration/generation. Enabling a new configuration resets its schedule/`last_run`; disabling preserves the last-success record. This is not proof of unattended end-to-end backup recovery on every target filesystem.
- Secret replacement retry, future-T0 display isolation, source-specific numeric import validation, and ownership-aware bounded clipboard expiry retry were locally regression-tested. They are not proof of broader OS-level erasure or native user-flow acceptance.
- Full native GUI acceptance was not completed: the UI broker could not connect. An isolated explicit-target executable process responded with the expected title and created no files; it did not create/unlock a vault and the real user process was not touched.
- The local 0.3.0 Windows executable and NSIS/MSI outputs have no Authenticode signature and are not published release assets. No genuine signed prior-version upgrade/reopen-vault test was run.
- Shared transactional capacity/backup-limit enforcement, zero-account settings/backup restore UX, import error classification, backup filename collision UX, editing accounts without an issuer, Windows clipboard-history/cloud opt-out, intermediate plaintext-copy zeroization, default algorithm/font-size choices, and non-NTFS/network durability remain follow-ups. See the current [backlog](../AUTONOMOUS_TASKS.md).
