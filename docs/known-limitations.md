# Known limitations

Recorded on 2026-11-18.

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

## Operational

- The update check is user-initiated. TOTP/HOTP uses local time and there is no sync service, account, or telemetry.
- If the SQLite vault is corrupted beyond the integrity check, restore is
  possible only from a previous encrypted backup.
