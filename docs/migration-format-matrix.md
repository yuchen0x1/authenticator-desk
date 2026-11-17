# Migration format matrix

Checked 2026-11-17/18 (UTC+8). Upstream references below come from
`docs/research/format-and-competitor-research.md`; the pinned commits there are
the provenance for each observed field contract. AUTHENTICATOR DESK implements
independent readers from the documented field shapes — no third-party parser or
crypto code is copied, and GPL/AGPL sources are used as references only. The
implementation stays MIT.

All fixtures under `src-tauri/tests/fixtures/import/` are synthetic and carry
only public demo secrets (`JBSWY3DPEHPK3PXP`, the RFC 4226 reference secret
`GEZDGNBVGY3TQOJQ…`). They were generated for this repository and encode no
real account data. Google Authenticator migration payloads in the fixtures are
hand-encoded protobuf built from the documented field numbers.

## Supported imports

| Format | Detects | Read shape | Encrypted behavior | Fixture |
|---|---|---|---|---|
| Google Authenticator migration (`google_migration`) | Text lines starting with `otpauth-migration://` | Bounded protobuf decode of the `data` parameter; per-line batch assembly; `SHA1/256/512`, 6/8 digits, `HOTP`/`TOTP`, counter | Not encrypted by design (QR transport) | `google_migration_multi.txt`, `google_migration_malformed.txt` |
| Aegis plain JSON (`aegis`) | JSON object with `db`, or Aegis-shaped `header`+`data` (deskOTP plain) | `db.entries[]`: `type`, `name`, `issuer`, `info.secret/algo/digits/period/counter`; defaults SHA1/6/30; Aegis db version 1–3 | `header.slots`/`header.params` non-null or `data` present → `unsupported_encrypted_format`; no bypass | `aegis_plain.json`, `aegis_encrypted.json` |
| 2FAS plain JSON (`twofas`) | JSON object with `services` | `services[]` with nested `otp` (`tokenType`, `algorithm`, `digits`, `period`, `counter`, `account`, `issuer`, `secret`); schemaVersion 1–4; outer `type` never read | `servicesEncrypted` present or non-null `reference` → `unsupported_encrypted_format` | `twofas_plain.json`, `twofas_encrypted.json` |
| andOTP plain JSON (`andotp`) | JSON array of objects with `secret`, or `.aes` envelope | Array entries: `secret`, `issuer`, `label`, `type` (`TOTP`/`HOTP`/`STEAM`), `algorithm`, `digits`, `period`/`counter`, `tags` | `.aes` envelope (BE iteration count 140000–160000 + salt + GCM body) → `unsupported_encrypted_format`; `.gpg` never attempted | `andotp_plain.json`, `andotp_encrypted.aes` |
| Ente plain list / generic otpauth list (`otpauth_list`) | First non-empty line starts with `otpauth://` | One URI per line via the strict otpauth parser | Encrypted Ente JSON → `unsupported_encrypted_format` | `ente_plain.txt`, `otpauth_mixed.txt` |

## Recognized but rejected

| Format | Detects | Behavior | Reason | Fixture |
|---|---|---|---|---|
| Aegis encrypted vault | `header.slots`/`header.params` non-null | `unsupported_encrypted_format` | scrypt password-slot + AES-GCM recovery needs verified vectors; no bypass attempted | `aegis_encrypted.json` |
| 2FAS encrypted services | `servicesEncrypted` string | `unsupported_encrypted_format` | PBKDF2/AES-GCM envelope needs verified vectors | `twofas_encrypted.json` |
| andOTP `.aes` / `.gpg` | Binary envelope or non-JSON content | `unsupported_encrypted_format` | Same; extension alone is never trusted | `andotp_encrypted.aes` |
| Ente encrypted export | JSON with `encryptedData`+`kdfParams`+`encryptionNonce` | `unsupported_encrypted_format` | Argon2id + XChaCha20-Poly1305 secret stream needs vectors | `ente_encrypted.json` |
| FreeOTP 2.x backup | Java serialization magic `AC ED 00 05`, or XML `<map>` dump | `unsupported_encrypted_format`, even with a password | Token material is Android Keystore-bound; no portable plaintext restore exists | `freeotp_backup.xml`, `freeotp_serialized.bin` |

Encrypted formats are rejected with the stable error code
`unsupported_encrypted_format` whether or not a password is supplied; no
password guessing, key caching, or envelope bypass is implemented.

## Intentional non-mappings

- STEAM entries (Aegis `steam`, 2FAS `tokenType: "Steam"`, andOTP `STEAM`) are
  reported as unsupported rows (`unsupported_steam` / `unsupported_token_type`)
  with their issuer/account preserved. Steam codes use a custom alphabet, so
  they are never silently converted to digit TOTP.
- Aegis UUIDs, groups, and icons; 2FAS group/order/icon metadata; andOTP
  thumbnails are provenance only and never become local identity.
- Hotp counters default to `0` only when a source omits them; 2FAS HOTP rows
  without a counter are marked invalid (`missing_counter`) instead of guessing.

## Upstream references (field contracts only)

- Google Authenticator migration protobuf: field numbers per
  `mobile/apps/auth/protos/googleauth.proto` in ente/ente at
  `8a17181b38a6bc91e27a442e5b715f6fe9bf56c9` (AGPL — reference only).
- Aegis plain/encrypted JSON: `VaultFile.java`, `Vault.java`, `OtpInfo.java`,
  `TotpInfo.java`, `HotpInfo.java` in beemdevelopment/Aegis at
  `d6f4e5925a97e4e91593f1542085eae03432a759` (GPL-3.0 — reference only).
- 2FAS: `BackupContent.kt`, `BackupService.kt`, `BackupCipherImpl.kt` in
  twofas/2fas-android at `4ad83b2b94967993db55dd36dcb2e258646eb7b6` (GPL-3.0 —
  reference only).
- andOTP: `Entry.java`, `PlainTextBackupTask.java`, `EncryptedBackupTask.java`
  in andOTP/andOTP at `6c54b8811f950375c774b2eefebcf1f9fa13d433` (MIT;
  archived/unmaintained — legacy format source).
- Ente: `export/ente.dart`, `local_backup_service.dart` in ente/ente at the
  commit above (AGPL — reference only).
- FreeOTP: `BACKUP.md`, `TokenPersistence.java`, `TokenBackupAgent.java` in
  freeotp/freeotp-android at `8ad601d1ecb5599e5e3bf4651ede20471dddfce3`
  (Apache-2.0 — reference only).

## Import pipeline invariants

1. Files are bounded to 2 MiB and 1000 rows before parsing; secrets are
   handled in zeroizing buffers and never logged.
2. Detection is content-based and ordered (migration URI → FreeOTP binary/XML
   → andOTP → Ente encrypted → Aegis → 2FAS → plain otpauth list); unknown
   content fails with `invalid_input`.
3. Every row is classified before commit (`new` / `duplicate` / `invalid`
   with a stable error code); only rows still marked `new` can be committed,
   transactionally, inside the encrypted vault.
