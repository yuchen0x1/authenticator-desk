# Project status

Living maintenance record, updated 2026-11-21 (Asia/Shanghai). This is not release evidence; the [dated test report](docs/test-report.md) distinguishes observed results from remaining acceptance.

## Snapshot

| Item | Value |
| --- | --- |
| Product | AUTHENTICATOR DESK — offline-first Windows desktop TOTP/HOTP authenticator |
| Stack | React 19 + TypeScript + Vite + Tailwind; Rust 2024 + Tauri 2 |
| Storage | Local SQLite, schema 1; Argon2id + AES-256-GCM authenticated records |
| Network | No sync account or telemetry; bounded GitHub HTTPS for deliberate signed-update requests |
| Prepared version | 0.2.1; package, Tauri and Cargo versions agree |
| Default branch | `main`, verified through GitHub repository metadata |
| CI | Portable Rust core on Linux/macOS; Windows tests/build; separate fresh-runner NSIS/MSI lifecycle matrix |
| Pinned tools | Node 26.13.1, pnpm 11.20.0, Rust 1.99.1, cargo-audit 0.23.0 |

## Delivery state

The remote release state and subsequent CI results must be checked separately from local tests. At this round's initial remote inspection, `v0.2.0` was published with zero assets and there was no `v0.2.1` release. An empty release entry is not an installable update. See [release notes](docs/releases/v0.2.1.md) and [release history](docs/releases/release-notes.md).

The working tree already contained a substantial uncommitted safety batch. It was preserved outside the repository before this round; this round also added the vault writer lease, corrected UTC clock handling, corrupt-account recovery, import diagnostics and isolated installer checks. Do not attribute the entire inherited batch to this round.

## Current local qualification

- Frontend: 219 tests in 25 files; TypeScript and production build passed.
- Rust core: 55 library + 53 integration tests passed. One child-process helper is ignored in discovery and explicitly invoked by its passing parent regression.
- Signed-updater boundaries: four passed; real-key staged artifact test not run locally.
- Node release-trust tests: 38 passed after the local installer-family selector regression was added.
- Final Windows x64 EXE/MSI built; local artifacts have no Authenticode publisher signature. Runtime acceptance uses synthetic data only.
- Dependency audit: npm reported zero advisories; Rust audit exited successfully with six documented warnings. This does not establish absence of vulnerabilities.

## Boundaries

- The per-vault OS-held lease coordinates cooperating clients, not older versions or arbitrary external file replacement. Exit every version before upgrading.
- No genuine prior-version signed live upgrade or final local installer lifecycle was executed. A real instance was running; installation changes belong on disposable machines.
- Updater/minisign verification is not Windows Authenticode publisher identity.
- Known product follow-ups and their evidence are tracked in [issues](AUTONOMOUS_ISSUES.md) and the [backlog](AUTONOMOUS_TASKS.md).

## 0.3.0 / 2026-11-21 — latest local evidence addendum

This append-only addendum supersedes the 0.2.1 snapshot above for this round only. Historical counts and claims remain unchanged. It is local qualification evidence, not a release-completion statement.

| Item | Latest observed result |
| --- | --- |
| Prepared source version | 0.3.0 verified; implementation committed locally as `8768935d538a1a85f7076f7fd7f10ca41ddbffff`; remote CI, tag, push and release are not established by this local snapshot |
| Frontend and Node | Vitest **238 passed in 26 files**; Node release-trust suite **39 passed** |
| TypeScript / build / hygiene | `tsc --noEmit`, Vite production build (148 modules), version check and `git diff --check` passed |
| Rust default suite | **64 library + 63 integration = 127 passed**; one subprocess lease helper is ignored in discovery and invoked by its passing parent |
| Desktop feature lane | **5 passed**, including the two desktop tests repeated by the default integration run |
| Signed-updater lane | **4 passed**; one real signed-artifact verification test intentionally ignored because release-key artifacts are not present locally |
| Dependency audit | npm reported no known vulnerabilities; Rust audit exited 0 with six allowed warnings, recorded in the dated test report |
| Windows packaging | 0.3.0 NSIS and MSI were built successfully. They are unsigned local packages and are not formal release assets |

The native UI broker was unavailable for this round. Only an isolated explicit-target artifact process `Responding`/title smoke was observed; its isolated directory stayed empty, no vault was created or unlocked, and no complete GUI acceptance or upgrade acceptance is claimed. Final artifact hashes, audit findings, and the release-owner completion fields are in [the dated test report](docs/test-report.md). The 0.3.0 unresolved risks and non-implementation backlog are in [issues](AUTONOMOUS_ISSUES.md) and [tasks](AUTONOMOUS_TASKS.md).
