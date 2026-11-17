# Known limitations

Honest list as of 2026-11-18. Anything not listed as done in the test report
should be assumed not implemented.

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
- No code signing, no auto-update channel, and no online release
  verification. Installer and upgrade evidence is pending the Task 9
  acceptance run; absence of that evidence means those claims are not made.
- Global shortcuts, tray interactions, autostart, and notifications compile
  and are registered behind the `desktop` feature, but interactive Windows
  validation (click behavior, shortcut conflicts, Toast delivery) still
  requires the packaged-app evidence run in Task 9.

## Backup constraints

- Losing a backup password permanently loses that backup; there is no
  recovery key, escrow, or password reset.
- Automatic backups are encrypted with the same `.authdesk` envelope and
  require the backup password to restore; they are not a DPAPI-managed
  convenience copy.

## Operational

- Network use is limited to an optional, explicit time calibration request.
  There is no sync service, account, or telemetry.
- If the SQLite vault is corrupted beyond the integrity check, restore is
  possible only from a previous encrypted backup.
