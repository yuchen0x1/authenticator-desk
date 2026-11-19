# Maintenance and platform decisions

## Audit baseline

2026-11-18 · Windows 11 x64 · main at 81578b9 · MIT. GitHub is public, the default branch is main, and v0.1.0 ships x64 NSIS and MSI. Issues and private vulnerability reporting are enabled. Discussions and Pages are disabled. Wiki is enabled. The baseline had no rulesets, repository Actions secrets or variables. Secret scanning and push protection were enabled. The repository default token was read-only. Keep docs in Git and use Issues for product requests.

| Layer | Baseline function and contract |
|---|---|
| Rust application | TOTP/HOTP with SHA-1/256/512, local clock, redacted RPC responses and explicit HOTP advancement |
| Vault and database | SQLite schema 1; master password, Argon2id and AES-GCM; lock and idle expiry; metadata, order, categories, tags, favorites, audit and search |
| Import and export | URI/link paste, Google migration QR, QR image, plaintext Aegis, 2FAS and andOTP, encrypted .authdesk backup, restore preview/merge/replace, automatic backup and retention |
| React/WebView2 | Overview, codes, accounts, favorites, categories, import, backup, security, activity, and settings; four locales and two themes |
| Windows integration | NSIS/MSI, tray, shortcuts, favorites window, startup, test notification, and sequence-owned clipboard cleanup |
| Service boundaries | Local Tauri IPC and native dialogs. There is no LAN listener, HTTP API, database server or account service. |

The baseline passed 18 frontend and 74 Rust tests and the Vite production build. GitHub's v0.1.0 NSIS asset matched its SHA-256 digest and started with an isolated data directory. The observed screen was the new-vault form. The historical [test report](test-report.md) and [acceptance checklist](acceptance-checklist.md) document earlier checks against their own builds.

Audit gaps included v0.1.0's unsigned distribution, absent updater, mutable workflow references, missing asset checksum files, browser and Rust advisory failures, and platform docs that overclaimed the nature of the React input boundary. Windows clipboard protection uses ownership/sequence, which the non-Windows implementation currently lacks. App sessions, import/restore staging and OTP code views are split between the Rust core and frontend. The SQLite identifier, paths and schema remain compatible with v0.1.0.

## Platform feasibility

A or B enters product development. C is deferred with a concrete gate. D conflicts with the trust model. Every native desktop port needs a secure clipboard contract, isolated vault, file picker, lock lifecycle, accessible input, distribution and update acceptance.

| Target | Grade | Compatibility, experience and release constraints |
|---|---|---|
| Windows 11 x64 | A | Full core and native WebView2/clipboard integration. Hosted MSVC packaging and a local native session are available. NSIS/MSI through Releases. |
| Windows 10 x64 | C | Existing installers may run with WebView2. Archived OS versions and the lack of a Windows 10 acceptance machine make support cost disproportionate. No new minimum imposed on the existing app. |
| Windows ARM64 | C | Rust/Windows APIs are portable. Native WebView2 and safe clipboard need an ARM device and native install/upgrade acceptance. |
| Linux generally | C | Portable OTP and vault verified independently. GTK/WebKit desktop currently pulls glib 0.18.5; upstream fix is on 0.20+. Ownership-protected clipboard, Wayland global shortcuts and tray need environment-specific work. |
| Ubuntu | C | Highest-value Linux starting point. Needs patched GTK stack, desktop acceptance on X11/Wayland, DEB ownership plus optional AppImage updates. WSL is useful for core tests. |
| Debian | C | Shared Ubuntu packaging work plus minimum GLIBC/WebKitGTK distribution checks. Apt-owned installs need repository/package updates. |
| Fedora | C | RPM dependency and library checks plus Wayland lifecycle acceptance. DNF owns RPM updates. |
| Arch | C | Rolling environment and multiple compositor/session behaviors require an independent release and acceptance path. Use distro package maintenance if adoption warrants it. |
| macOS Intel | C | Core is suitable. Requires OS-native clipboard ownership, shortcuts, Keychain decision, Intel hardware and Developer ID signing/notarization. |
| macOS Apple Silicon | C | Higher potential than Intel. Same port work and certificate chain plus independent native signing, DMG and upgrade acceptance. |
| Android phone | C | OTP core is viable. A separate touch UI, camera flow, Keystore/screen lifecycle, scoped storage, signed APK and upgrade device are required. |
| Android tablet | C | Depends on Android foundations plus two-pane layout, rotation, keyboard and multiwindow handling. |
| Android foldables | C | Depends on Android foundations plus posture, hinge, lifecycle and fold/unfold hardware tests. |
| iOS | C | Rust logic is reusable. Keychain access groups, QR capture, local file handling, lock lifecycle, App Store identity and device verification are substantial. |
| iPadOS | C | iOS prerequisites plus split-screen, keyboard and file-restore acceptance. Uses App Store updates. |
| Desktop Web | D | Browser storage and clipboard lack this app's native clearing/ownership and vault isolation contract. Server storage changes the product's trust boundary. |
| Mobile Web | D | Inherits Web limits plus background lock, clipboard and storage eviction risks. |
| Responsive Web | D | Layout can adapt. Browser vault and clipboard guarantees are the relevant blockers. |
| PWA | D | Service-worker caching handles assets. Vault storage durability, clipboard ownership and local release trust would require a different product architecture. |
| Chrome extension | C | Useful for website pairing. Least-privilege host access, signed native pairing/consent and store review are prerequisites. |
| Edge extension | C | Chromium reuse is feasible with separate store signing, install and permission verification. |
| Firefox extension | C | Independent review, metadata and native messaging validation are needed. |
| Docker/NAS | D | Hosting seeds behind a service introduces remote access, credentials, backups and exposure that conflict with the local desktop boundary. |
| Private hosting | D | Requires a different account/service and threat model. |
| Cloud server | D | Centralizes authenticator seeds and duplicates a mature self-hosted product category. |
| CLI | C | OTP core is reusable. Terminal, shell history and stdout can expose secrets and codes. Scripted access requires independent consent and output design. |

References for the assessments include the [ecosystem scan](research/ecosystem.md), [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [Tauri updater](https://v2.tauri.app/plugin/updater/), and [GTK advisory](https://github.com/advisories/GHSA-wrw7-89jp-8q8g).

## Change and release gates

Use short-lived branches and tested PRs into main. A tagged main commit re-runs the release checks. Portable core jobs validate Linux and macOS algorithms and vault behavior. Product release gates apply to Windows only. A compatibility target joins the release matrix once it has independent native evidence and an owner for distribution, updates and support.

When changing vault behavior, verify old binary or backup compatibility and include restore/rollback evidence. When changing installers, verify NSIS and MSI separately. When changing updates, test discovery, deferral, incomplete downloads, network failure, wrong signatures or hashes, version and OS checks, installing, restart, opening an existing vault, and manual recovery. Record boundaries of CI fixtures and actual devices.

See [updating.md](updating.md) for update trust and rollback. The release notes and final acceptance evidence record the actual status of each new tag. Use Dependabot and the Security workflow for dependency review. Audit upstream changes during each larger release and use the scheduled maintenance scan for release integrity and upstream metadata changes.
