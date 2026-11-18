# AUTHENTICATOR DESK

AUTHENTICATOR DESK is an offline-first Windows desktop authenticator built with Tauri 2, Rust, React, TypeScript, Tailwind CSS, and SQLite.

The Rust core owns an encrypted SQLite vault (Argon2id + AES-256-GCM), a
verified TOTP/HOTP engine (RFC 4226/6238 vectors), otpauth URI parsing,
encrypted `.authdesk` backups with restore previews, a security audit, and
clipboard protection. The React UI receives only typed, redacted view
models. The desktop shell ships tray, hotkey, autostart, and notification
integrations behind the `desktop` feature.

## Latest release

<!-- github-release:start -->

**[v0.1.0](https://github.com/yuchen0x1/authenticator-desk/releases/tag/v0.1.0)** · 2026-11-18

Authenticator Desk 是一款离线桌面验证器。首次启动时，用主密码创建本机保险库，即可添加双重验证账户。日常登录时，按 Ctrl+Shift+Q 就能查看收藏账户的验证码和剩余时间。

[Release history](https://github.com/yuchen0x1/authenticator-desk/releases)

<!-- github-release:end -->

Download the Windows x64 [EXE or MSI installer from GitHub Releases](https://github.com/yuchen0x1/authenticator-desk/releases/latest).

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

## Verification

```powershell
pnpm test -- --run
pnpm exec tsc --noEmit
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --features desktop --bin authenticator-desk
```

The default Rust feature set tests core code without linking the desktop runtime. The `desktop` feature builds the real Tauri entry point when packaging metadata and the Windows icon are present. The main capability grants no filesystem or shell permissions.

## Data location

Production data lives in Tauri's application data directory. The backend
accepts an `AUTHDESK_DATA_DIR` override so isolated test vaults can run
elsewhere; the frontend never chooses arbitrary storage paths.

## License

Original source code is licensed under the MIT License. See [LICENSE](LICENSE).
