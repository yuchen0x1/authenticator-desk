# Open issues and unresolved risks

Updated 2026-11-21 (Asia/Shanghai). Entries distinguish observed remote/local results from source-review findings. Priorities and closure evidence are in the [backlog](AUTONOMOUS_TASKS.md).

## Release / distribution

1. **Empty historical release.** At this round's remote inspection, published `v0.2.0` had zero assets. A release entry without installers is not an installable update. New verified assets require the normal qualified release path; do not fabricate success or silently rewrite historical evidence.
2. **No Authenticode publisher identity.** Local final EXE/MSI report `NotSigned`. Minisign/updater signatures are a different trust mechanism. Publisher signing requires secure certificate/private-key provisioning or an authorized signing service, not just a configured thumbprint. No paid service was changed.
3. **No live signed upgrade evidence.** Tests verify metadata and installer signatures with synthetic fixtures; real staged assets are checked by a separate gated test. A genuine install-old → update → reopen-vault run remains open.
4. **Final installer lifecycle not exercised on the user machine.** A real application instance was running. NSIS and MSI are now separate fresh-runner CI jobs; the local helper requires one family per clean disposable VM/snapshot. Wiring/syntax checks are not install evidence.

## Dependencies / tooling

5. **Rust audit warnings.** The Windows-target audit exited successfully with six warnings: unmaintained `chrono`, `proc-macro-error`, `zbus_names`, `zvariant`, `zvariant_derive`, plus `glib` 0.18.5 unsoundness (RUSTSEC-2024-0429). `glib` was absent from the inspected Windows target graph; this does not qualify a future Linux desktop runtime. See the [test report](docs/test-report.md).
6. **Dependency upgrade PRs need individual reproduction.** Earlier records identify failing automated upgrade lanes; this round did not qualify or merge those upgrades. No automatic closure or bulk dependency bump is justified.
7. **Partial local named Rust toolchain.** `1.99.1-x86_64-pc-windows-msvc` recovery fails with a cargo executable conflict. Local commands used `RUSTUP_TOOLCHAIN=stable`, independently verified as the same rustc 1.99.1. Global toolchain repair was not attempted.
8. **Local registry lacks an audit endpoint.** Use the explicit public npm registry for audits. Unrecognized/failed reports remain a failure, never an empty clean result.

## Source-evidenced product follow-ups

9. A future valid `T0` can fail the all-account code batch before that entry starts (`accounts/service.rs`, `otp/time.rs`). It needs per-account failure isolation, not a fabricated code.
10. Scheduled clipboard clearing makes one attempt; transient Windows clipboard contention can postpone cleanup until another trigger. Ownership checks must be retained in any retry.
11. Automatic backup failures have no last-attempt/error UI, and status does not continuously refresh while the page remains open.
12. Some malformed numeric legacy-import fields fall back to defaults; negative/string HOTP counters require explicit rejection coverage.
13. Cold locked startup uses the default locale until encrypted settings load.
14. Backup publication uses hard links; FAT32/exFAT/network filesystem behavior is unqualified.

## Intentional boundaries

- The writer lease coordinates cooperating canonical-path clients, not older clients, deliberate hard-link aliases or arbitrary external file replacement. Never delete a live `.lock` sidecar; quit all versions before upgrading.
- Clipboard ownership protection and released desktop integration are Windows-only; shared-core CI does not establish full Linux/macOS support.
- The product is local-only, with no sync service, recovery email or telemetry. Losing passwords without a usable backup is unrecoverable.
