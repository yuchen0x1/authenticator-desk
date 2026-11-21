# Updates and recovery

## Windows users

When a verified release with installers is available, upgrade v0.1.0 manually using the same installer family. Save an encrypted backup and fully exit every AUTHENTICATOR DESK process first, including older versions and tray instances. The per-vault writer guard in 0.2.1 only coordinates clients that implement it; an older client must not remain open against the same vault. The schema remains at 1. Download from [GitHub Releases](https://github.com/yuchen0x1/authenticator-desk/releases/latest) and compare the installer to SHA256SUMS.txt. A published release entry without installers is not an installable update.

On a signed x64 installation, open Settings → Application updates and select Check for updates. The dialog shows the version, date, notes and size. Later closes the prompt. A check runs only on request. The installer starts after download and validation. Windows may present its own UI or request confirmation.

If a download fails, check the connection and retry. The application will obtain the complete installer on each try. If installation or launch fails, preserve the application data folder and your encrypted backup. Obtain the previous matching EXE or MSI from Releases, verify its checksum where supplied, uninstall the failing application without deleting vault data, and reinstall the previous version. If necessary, restore into a separate encrypted vault before replacing live data. The current release keeps the schema at 1. Review schema compatibility before downgrading a future version.

## Update trust

The only updater endpoint is the stable GitHub Release for this repository. The native client accepts GitHub HTTPS redirects and uses the embedded release key to verify the metadata envelope. It checks the product version, Windows build and installer family. The package must match the manifest's size and SHA-256 and pass Tauri signature and signed-version verification before the installer runs. The vault is locked before exit. The WebView receives progress, version and release notes. It has no shell, filesystem or direct updater-plugin capability.

Checks and downloads have time and size limits. Failed checks and downloads leave the working install available. Tauri starts and restarts the application through the platform installer after validation. An installer that fails after handoff may require manual recovery. Authenticode signing uses a separate Windows publisher identity.

## Maintainer setup

A release tag on main runs the Windows and portable-core verification pipeline. The release environment contains TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD. AUTHDESK_UPDATER_PUBLIC_KEY is a public environment variable. Restrict the environment to version tags. Retain an offline, encrypted copy of the same key and its recovery instructions. If a key is lost or rotated, users must obtain a fresh trusted installer with the new public key.

Each signed tag needs docs/releases/vVERSION.md. The build emits x64 NSIS/MSI packages and version-bound signatures. The release script generates SHA256SUMS.txt, release-assets.json and a signed update.json. A draft is published only after all assets are re-downloaded and checked. The published v0.1.0 artifacts remain in release history.

The signed path now validates the calling repository/event/tag/workflow before entering the release environment and again before signing. The Security workflow is a required dependency of release verification. The known historical npm advisory exception has been removed: failed or unrecognized audit reports block the release. Stable promotion is serialized and must advance past every existing stable version. Signed assets are cryptographically verified against the separately supplied public key after upload and after publication, and the stable public update manifest must match the published bytes.

Compilation and packaging run without the updater private key in the child environment. The standalone signer signs only the final installers. Optional Windows publisher signing uses an already-provisioned certificate: set release environment variables AUTHDESK_PUBLISHER_THUMBPRINT (the pinned certificate), AUTHDESK_TIMESTAMP_URL (an HTTPS RFC3161 service), and AUTHDESK_REQUIRE_AUTHENTICODE=1 to fail closed if publisher signing is unavailable. Tauri applies Authenticode before updater signatures; the gate verifies the expected publisher and timestamp. A configured public thumbprint is not itself a signing credential. No certificate is created or impersonated by the build.

Use an empty release-assets directory when staging; archive older local output instead of mixing versions. scripts/verify-release.ps1 runs local locked tests and unsigned packaging only. Its optional `-InstallerSmoke -InstallerFamily NSIS` or `-InstallerSmoke -InstallerFamily MSI` changes Windows installer registration and belongs on a clean disposable VM/snapshot, one family per machine state. CI runs the two families on separate fresh runners. Those checks do not replace a real prior-version signed update, certificate qualification, or supported desktop OS acceptance.
