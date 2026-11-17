# AUTHENTICATOR DESK

AUTHENTICATOR DESK is an offline-first Windows desktop authenticator built with Tauri 2, Rust, React, TypeScript, Tailwind CSS, and SQLite.

This repository currently contains the secure application scaffold: a locked desktop shell, a typed frontend-to-Rust command boundary, safe error payloads, and the initial migration surface. Account and vault workflows are intentionally unavailable until their secure backend services are implemented.

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

The browser shell explicitly reports that the desktop security backend is unavailable. Run the desktop application for Tauri IPC after the packaging icon is supplied:

```powershell
pnpm tauri dev
```

The current scaffold intentionally leaves Windows bundling disabled until `src-tauri/icons/icon.ico` and the remaining packaging metadata are supplied in Task 9. Browser-shell tests and core Rust tests do not require that asset.

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

Production data will use Tauri's application data directory for `com.authenticatordesk.app`. A future backend-only `AUTHDESK_DATA_DIR` override may be used for isolated test vaults; the frontend will never choose arbitrary storage paths.

## License

Original source code is licensed under the MIT License. See [LICENSE](LICENSE).
