# AUTHENTICATOR DESK

AUTHENTICATOR DESK is an offline-first Windows desktop authenticator built with Tauri 2, Rust, React, TypeScript, Tailwind CSS, and SQLite.

The Rust core owns an encrypted SQLite vault (Argon2id + AES-256-GCM), a
verified TOTP/HOTP engine (RFC 4226/6238 vectors), otpauth URI parsing,
encrypted `.authdesk` backups with restore previews, a security audit, and
clipboard protection. The React UI receives only typed, redacted view
models. The desktop shell ships tray, hotkey, autostart, and notification
integrations behind the `desktop` feature.

## Latest release

[![Latest stable release](https://img.shields.io/github/v/release/yuchen0x1/authenticator-desk)](https://github.com/yuchen0x1/authenticator-desk/releases/latest)
[![CI](https://github.com/yuchen0x1/authenticator-desk/actions/workflows/ci.yml/badge.svg)](https://github.com/yuchen0x1/authenticator-desk/actions/workflows/ci.yml)

[Download for Windows x64](https://github.com/yuchen0x1/authenticator-desk/releases/latest) · [Release history](docs/releases/release-notes.md)

Choose EXE for the per-user installation or MSI if you currently use MSI. Windows 11 x64 is the desktop acceptance target. The CI also exercises the shared Rust core on Linux and macOS.

![AUTHENTICATOR DESK codes page](screenshots/03-codes.png)

Add an account or import a QR image, then open favorite codes with Ctrl+Shift+Q. The protected clipboard clears only the value still owned by the app. Keep an encrypted backup from the Backup page so you can restore the vault on another Windows installation.

## Updating

Users on v0.1.0 install the next release manually using the same installer family. Signed builds provide **Settings → Application updates** with release notes, download progress and deliberate installation. See the [update and recovery guide](docs/updating.md).

## Import and migration

- Textarea paste and file import both route through the same
  preview → select → transactional commit flow; secrets never leave the
  Rust core during preview.
- Supported readers: plain `otpauth://` lists (including Ente plaintext
  exports), Google Authenticator migration URIs, Aegis plain JSON, 2FAS
  plain JSON (schema 1–4), and andOTP plain JSON.
- Encrypted Aegis/2FAS/andOTP/Ente backups and FreeOTP backups are detected
  and rejected with `unsupported_encrypted_format`; no bypass is attempted.
  See [docs/migration-format-matrix.md](docs/migration-format-matrix.md).

## Documentation

- [Maintenance and platform status](docs/maintenance.md)
- [Authenticator ecosystem research and backlog](docs/research/ecosystem.md)
- [Contribution workflow](CONTRIBUTING.md) and [private security reporting](SECURITY.md)
- [docs/security-model.md](docs/security-model.md) — trust boundaries,
  cryptography, lifecycle, and network posture
- [docs/migration-format-matrix.md](docs/migration-format-matrix.md) —
  per-format field contracts, fixture provenance, and license notes
- [docs/test-report.md](docs/test-report.md) — observed verification results
- [docs/known-limitations.md](docs/known-limitations.md) — what this release
  does not claim

## Development

Prerequisites:

- Node.js 24 or newer and pnpm 11
- Rust stable with the MSVC toolchain
- Visual Studio 2022 C++ Build Tools
- Microsoft Edge WebView2 Runtime

```powershell
pnpm install
pnpm dev
```

The browser shell explicitly reports that the desktop security backend is unavailable. Run the desktop application for full Tauri IPC:

```powershell
pnpm tauri dev
```

## Publishing release notes

Write `docs/releases/<tag>.md` with the changes, Windows downloads and upgrade details, then update the [release history](docs/releases/release-notes.md). A tag from main triggers the signed release workflow after the version and packaging gates pass.

## Verification

```powershell
pnpm exec vitest run
pnpm exec tsc --noEmit
pnpm build
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml --features signed-updater --bin authenticator-desk
```

The default Rust feature set tests core code without linking the desktop runtime. The `desktop` feature builds the real Tauri entry point when packaging metadata and the Windows icon are present. The main capability grants no filesystem or shell permissions.

## Data location

Production data lives in Tauri's application data directory. The backend
accepts an `AUTHDESK_DATA_DIR` override so isolated test vaults can run
elsewhere; the frontend never chooses arbitrary storage paths.

## License

Original source code is licensed under the MIT License. See [LICENSE](LICENSE).

Report product issues through [GitHub Issues](https://github.com/yuchen0x1/authenticator-desk/issues/new/choose). If you forgot the vault or backup password, use a backup whose password you know. There is no password recovery service.
