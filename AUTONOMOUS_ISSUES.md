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

## 0.3.0 / 2026-11-21 — current disposition

The previous section records the safety-evolution baseline. The following source-review findings have been addressed in the 0.3.0 working tree and locally regression-tested; this is not remote or release verification:

- Failed account edit retains replacement-secret intent while clearing the sensitive input, avoiding a false successful replacement.
- A future-T0 account is isolated from available-code display batches; protected single-account generation still rejects before T0.
- Aegis/andOTP/2FAS explicit malformed numeric values are rejected; format-specific omitted-field defaults remain, including legal 2FAS nullable fields and the documented counter/period defaults.
- Clipboard expiry uses one cancellable worker with bounded retry/backoff and token/sequence ownership checks; it does not replay copy or clear a later user-owned value.
- Automatic backup health now distinguishes encrypted last-success data, which survives restart and disable, from attempt/failure status local to the active session/configuration/generation. Enabling a new configuration resets its schedule/`last_run`; disabling preserves last success. UI refresh and error details are covered in local tests.
- Feature-gated desktop integration contracts are executed in the Windows CI configuration, though no new-source hosted run is asserted here.

The following are the current unresolved boundaries and follow-ups, prioritized by delivery/correctness impact rather than severity claims:

1. **P1 — release qualification.** Exact-source remote Security/CI, signed publication and asset verification remain unproven. Complete native GUI workflow acceptance and genuine old-version signed update → reopen-vault acceptance are not done. Do not promote unsigned local packages as release assets or use baseline-SHA CI as qualification.
2. **P3 — distribution identity.** Authenticode publisher identity is absent (`NotSigned`) for all three final local Windows artifacts. Updater signatures do not provide Windows publisher identity. Provisioning/signing requires the authorized release process; this is a publisher/distribution gate, not a proven application bypass.
3. **P2 — write/backup limits.** Shared account/category write-capacity checks and backup-size/retention-limit consistency still need a closed, transactional contract.
4. **P2 — privacy hardening.** Windows clipboard history/cloud opt-out behavior has not been implemented/accepted. Plaintext JSON/URL/QR/HMAC intermediate copies need inventory and zeroization; this is a hardening task, not evidence of a recovered seed or at-rest encryption failure.
5. **P2 — restore/import correctness and resilience.** Provide classified settings/backup restore behavior when there are zero accounts; classify non-conflict import commit failures; qualify backup publication durability on FAT/exFAT/network filesystems. Current hard-link behavior is not evidence those filesystems are supported.
6. **P3 — usability.** Improve backup's default filename collision experience; allow editing accounts that have no issuer; expose/default algorithm and font-size controls where appropriate.
7. **P3 — dependency maintenance.** Five unmaintained Rust dependency warnings and the `glib` unsoundness warning remain (see dated audit evidence). The Windows target observation does not establish Linux desktop safety or support; the warnings are not by themselves a proven application exploit.

Detailed next steps and closure evidence are tracked in [the current backlog](AUTONOMOUS_TASKS.md); local test and artifact boundaries are in [the test report](docs/test-report.md).
