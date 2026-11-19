# Updates and recovery

## Windows users

Install v0.2.0 manually over v0.1.0 using the same installer family. Save an encrypted backup and close AUTHENTICATOR DESK first. The existing schema and data directory are compatible with the prior release. Download from [GitHub Releases](https://github.com/yuchen0x1/authenticator-desk/releases/latest). Compare the installer to SHA256SUMS.txt.

On a signed x64 installation, open Settings → Application updates and select Check for updates. The dialog shows the version, date, notes and size. Later closes the prompt. A check runs only on request. The installer starts after download and validation. Windows may present its own UI or request confirmation.

If a download fails, check the connection and retry. The application will obtain the complete installer on each try. If installation or launch fails, preserve the application data folder and your encrypted backup. Obtain the previous matching EXE or MSI from Releases, verify its checksum where supplied, uninstall the failing application without deleting vault data, and reinstall the previous version. If necessary, restore into a separate encrypted vault before replacing live data. The current release keeps the schema at 1. Review schema compatibility before downgrading a future version.

## Update trust

The only updater endpoint is the stable GitHub Release for this repository. The native client accepts GitHub HTTPS redirects and uses the embedded release key to verify the metadata envelope. It checks the product version, Windows build and installer family. The package must match the manifest's size and SHA-256 and pass Tauri signature and signed-version verification before the installer runs. The vault is locked before exit. The WebView receives progress, version and release notes. It has no shell, filesystem or direct updater-plugin capability.

Checks and downloads have time and size limits. Failed checks and downloads leave the working install available. Tauri starts and restarts the application through the platform installer after validation. An installer that fails after handoff may require manual recovery. Authenticode signing uses a separate Windows publisher identity.

## Maintainer setup

A release tag on main runs the Windows and portable-core verification pipeline. The release environment contains TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD. AUTHDESK_UPDATER_PUBLIC_KEY is a public environment variable. Restrict the environment to version tags. Retain an offline, encrypted copy of the same key and its recovery instructions. If a key is lost or rotated, users must obtain a fresh trusted installer with the new public key.

Each signed tag needs docs/releases/vVERSION.md. The build emits x64 NSIS/MSI packages and version-bound signatures. The release script generates SHA256SUMS.txt, release-assets.json and a signed update.json. A draft is published only after all assets are re-downloaded and checked. The published v0.1.0 artifacts remain in release history.
