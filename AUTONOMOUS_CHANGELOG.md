# Autonomous development log

Newest first. Each round records what was inspected, what changed, what was verified,
and what was deliberately not claimed.

## 2026-11-21 — safety evolution and final local qualification

**Preserved and inspected.** Started on `fix/v0.2.1-safety-acceptance` at `ca06984` with a substantial existing dirty tree. Protected the tracked binary diff and individually listed untracked files in an external temporary snapshot; directory-status entries were not recursively copied. No reset/clean, real-vault mutation, or original-process termination was performed. Fetched default `main` at `a0ed772`; its tree matched the starting branch base.

**Research.** Rechecked seven real projects (Aegis, Ente Auth, 2FAS, KeePassXC, Authenticator Extension, Stratum and Authme) against pinned upstream commits and primary format/source files. Findings, license boundaries and non-benchmark caveats are in `docs/research/ecosystem.md`. No third-party code was copied.

**Changed in this round.** Fixed persistent TOTP drift after UTC correction/resume; added a per-vault OS-held single-writer lease with a real subprocess regression; added restricted authenticated recovery navigation and confirmed backup replacement over damaged account rows; retained named-row import diagnostics across four locales; separated NSIS/MSI fresh-runner CI and local single-family smoke selection. Reviewed the inherited safety batch without claiming authorship of it.

**Observed.** 219 frontend tests, TypeScript/Vite build, 108 Rust core tests, four signed-updater boundary tests, and 38 Node trust tests passed. A subprocess helper is ignored in default discovery but explicitly invoked by its parent; the real-key staged-release test remains unrun locally. Final unsigned Windows EXE/MSI were rebuilt. Native UI acceptance used an isolated temporary vault and public RFC test seed: create, mixed valid/invalid import preview, selective import, code/countdown, hide/copy-disable, lock and re-unlock. See the current dated `docs/test-report.md` entry for artifact hashes and scope.

**Not claimed.** No performance speedup, independent security certification, Authenticode identity, final local installer lifecycle or genuine prior-version live update. Remote integration/publication is separate from these local observations and must be verified against actual remote state.

## Inherited historical entry — not this round's verification

The entry below was already present when this round began. Its counts and actions are historical claims, not independently re-established evidence. In particular, `SECURITY_AUDIT.md` and `RELEASE_HISTORY.md` were not present at inspection; current references use the existing security/test/release documentation instead.

### 2026-11-21 — prior record: qualify and deliver the 0.2.1 safety batch

**Inspected.** Local branch `chore/prepare-v0.2.1` (61 modified tracked files plus new
tests/scripts, uncommitted), `main` at `a0ed772`, tags `v0.1.0`/`v0.2.0`, seven open
Dependabot PRs, one open Dependabot advisory (`glib`, Linux-only), release/security
workflow configuration, and the release environment's signing configuration.

**Found.**

1. The whole 0.2.1 batch was uncommitted, so the remote `main` still contains the
   defects the batch fixes.
2. The prepared branch failed seven frontend tests: bulk-selection assertions still used
   the old checkbox accessible name (`Selected Alpha`) while `AccountCard` now labels
   rows `Select: Issuer · account`. Test-only staleness, no product regression.
3. `v0.2.0` is published with no assets because its release run failed the frontend step.
4. Local `cargo` resolves to a partially installed 1.99.1 toolchain; `stable` carries the
   same compiler, so local Rust verification used `RUSTUP_TOOLCHAIN=stable`.
5. `pnpm audit` fails closed locally because the configured registry
   (`registry.npmmirror.com`) has no audit endpoint; the public registry reports zero
   advisories for production and development dependencies.
6. `glib` RUSTSEC-2024-0429 is reported by the lockfile audit but is not in the Windows
   dependency graph (`cargo tree --target x86_64-pc-windows-msvc -i glib` matches nothing).

**Changed.** Updated the seven stale assertions to the real accessible names, keeping
every selection/confirmation/IPC/checked-state expectation. Added this record set
(`PROJECT_STATUS.md`, `AUTONOMOUS_TASKS.md`, `AUTONOMOUS_CHANGELOG.md`,
`AUTONOMOUS_ISSUES.md`, `SECURITY_AUDIT.md`, `RELEASE_HISTORY.md`) and a dated
`docs/test-report.md` section with this round's observed numbers.

**Verified.** 174 frontend tests in 22 files, `pnpm build`, 50 library + 45 integration
Rust tests, 4 signed-updater tests (1 ignored by design), 36 Node release-trust tests,
`cargo check --features signed-updater --all-targets`, production and development audits
at zero advisories.

**Not claimed.** No installer execution, no live upgrade from a prior version, no
Authenticode publisher identity.

**Preservation note.** The original uncommitted worktree (88 files plus a binary diff)
was copied to a temporary directory before branching, so no pre-existing edit could be
lost by this round.

## 2026-11-21 — 0.3.0 reliability follow-up and evidence closeout

This is a new append-only evidence entry. It does not rewrite the inherited 0.2.1 history, claim authorship of inherited changes, or mark the release complete.

**Changed in the 0.3.0 working tree.** Account-edit retries retain replacement-secret intent while clearing the secret input; future-T0 display failures are isolated; Aegis/andOTP/2FAS imports validate present numeric values source-specifically (including legal 2FAS nulls and documented defaults); clipboard expiry uses a single cancellable retry worker with bounded backoff and ownership checks; automatic-backup health persists encrypted last-success while attempt/failure status remains session/configuration/generation-local. Enabling a new configuration resets its schedule/`last_run`; disabling preserves last success. Feature-gated desktop integration tests now execute in CI.

**Observed locally.** Vitest 238/26 files, Node 39, TypeScript/build/version/diff checks passed; Rust default suite 64 library + 63 integration (127 passed; one ignored subprocess helper invoked by its passing parent); desktop feature 5 passed (including two tests repeated by default discovery); signed-updater 4 passed with one staged-signature test intentionally ignored. Explicit-target Windows build produced 0.3.0 NSIS/MSI, all three final artifacts Authenticode `NotSigned`. Dependency audit exited 0 with no known npm vulnerabilities and six allowed Rust audit warnings. Full GUI acceptance was blocked by the unavailable native UI broker; an isolated final-process responding/title smoke created no files and did not create or unlock a vault. See `docs/test-report.md` for exact hashes, warning IDs and test boundaries.

**Git boundary.** The implementation is locally committed as `8768935d538a1a85f7076f7fd7f10ca41ddbffff`.

**Not claimed by this local snapshot.** Remote CI for this exact source, tag/push/release, signed publication, complete GUI/installer lifecycle or genuine old-version signed update/reopen-vault acceptance. Local packages are not formal release assets. Follow-up items and release-owner fields are listed in `AUTONOMOUS_ISSUES.md` and `AUTONOMOUS_TASKS.md`.
