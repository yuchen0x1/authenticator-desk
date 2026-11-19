# Contributing

Use a short-lived branch from main and open a pull request. CI runs the frontend, portable Rust core and Windows packaging checks. Keep migrations compatible with old vaults and test import changes with synthetic data. Document user-visible changes with the version's release notes.

Main is the release source. Maintainers merge a tested PR, bump the package, Cargo and Tauri versions together, and tag a verified main commit. Use vMAJOR.MINOR.PATCH for a stable version and vMAJOR.MINOR.PATCH-rc.N for a release candidate. A tagged workflow verifies the commit, builds and smoke-tests the installers, then verifies uploaded assets before publishing the draft. For a failed upload, inspect the draft and workflow before resuming. Releases keep their original assets. Publish a new tag for corrections or hotfixes.

See the [README](README.md) for setup and the [maintenance record](docs/maintenance.md) for platform and release gates.
