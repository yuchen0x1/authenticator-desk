use serde::Serialize;
use tauri::{AppHandle, WebviewWindow, ipc::Channel};

#[derive(Serialize)]
pub struct UpdateVersion {
    version: String,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateError {
    Unavailable,
    Busy,
    Network,
    InvalidRelease,
    Incompatible,
    Integrity,
    Install,
}

#[derive(Clone, Serialize)]
pub struct ReleaseInfo {
    version: String,
    date: String,
    notes: String,
    bytes: usize,
}

#[derive(Serialize)]
pub struct DownloadProgress {
    received: usize,
    total: usize,
}

#[tauri::command]
pub fn update_version(app: AppHandle) -> UpdateVersion {
    UpdateVersion {
        version: app.package_info().version.to_string(),
        available: enabled(),
    }
}

fn enabled() -> bool {
    #[cfg(feature = "signed-updater")]
    return signed::available();
    #[cfg(not(feature = "signed-updater"))]
    false
}

#[tauri::command]
pub async fn check_update(window: WebviewWindow) -> Result<Option<ReleaseInfo>, UpdateError> {
    if window.label() != "main" || !enabled() {
        return Err(UpdateError::Unavailable);
    }
    #[cfg(feature = "signed-updater")]
    return signed::check(&window).await;
    #[cfg(not(feature = "signed-updater"))]
    Err(UpdateError::Unavailable)
}

#[tauri::command]
pub async fn install_update(
    window: WebviewWindow,
    version: String,
    progress: Channel<DownloadProgress>,
) -> Result<(), UpdateError> {
    if window.label() != "main" || !enabled() {
        return Err(UpdateError::Unavailable);
    }
    #[cfg(feature = "signed-updater")]
    return signed::install(&window, &version, progress).await;
    #[cfg(not(feature = "signed-updater"))]
    {
        let _ = (version, progress);
        Err(UpdateError::Unavailable)
    }
}

#[cfg(feature = "signed-updater")]
pub use signed::configure;

#[cfg(feature = "signed-updater")]
mod signed {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use minisign_verify::{PublicKey, Signature};
    use reqwest::{Client, redirect};
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::time::{Duration, Instant};
    use tauri::{Manager, async_runtime::Mutex, utils::config::BundleType};
    use tauri_plugin_updater::{Update, UpdaterExt};
    use url::Url;

    const KEY: &str = match option_env!("AUTHDESK_UPDATER_PUBLIC_KEY") {
        Some(key) => key,
        None => "",
    };
    const RELEASE_ROOT: &str = "https://github.com/yuchen0x1/authenticator-desk/releases/";
    const MANIFEST_LIMIT: usize = 96 * 1024;
    const PACKAGE_LIMIT: usize = 64 * 1024 * 1024;

    struct Candidate {
        update: Update,
        info: ReleaseInfo,
        sha256: String,
        checked_at: Instant,
    }

    #[derive(Deserialize)]
    struct Envelope {
        payload: String,
        signature: String,
    }

    #[derive(Deserialize)]
    struct Metadata {
        schema: u32,
        version: String,
        pub_date: String,
        release_date: String,
        notes: String,
        min_windows_build: u32,
        platforms: std::collections::BTreeMap<String, Asset>,
    }

    #[derive(Deserialize)]
    struct Asset {
        url: Url,
        bytes: usize,
        sha256: String,
    }

    pub fn configure(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
        builder
            .manage(Mutex::<Option<Candidate>>::default())
            .plugin(tauri_plugin_updater::Builder::new().pubkey(KEY).build())
    }

    fn installer() -> Option<(&'static str, &'static str)> {
        match tauri::utils::platform::bundle_type() {
            Some(BundleType::Nsis) => Some(("windows-x86_64-nsis", "-setup.exe")),
            Some(BundleType::Msi) => Some(("windows-x86_64-msi", ".msi")),
            _ => None,
        }
    }

    pub fn available() -> bool {
        !KEY.is_empty() && cfg!(target_arch = "x86_64") && installer().is_some()
    }

    fn release_url(url: &Url) -> bool {
        url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && match url.host_str() {
                Some("github.com") => url.as_str().starts_with(RELEASE_ROOT),
                Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com") => {
                    true
                }
                _ => false,
            }
    }

    async fn download(
        url: Url,
        limit: usize,
        mut on_progress: impl FnMut(usize),
    ) -> Result<Vec<u8>, UpdateError> {
        if !release_url(&url) {
            return Err(UpdateError::InvalidRelease);
        }
        let mut response = Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(180))
            .redirect(redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 || !release_url(attempt.url()) {
                    attempt.error("unexpected updater redirect")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|_| UpdateError::Network)?
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|_| UpdateError::Network)?;
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err(UpdateError::Integrity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| UpdateError::Network)? {
            if chunk.len() > limit - bytes.len() {
                return Err(UpdateError::Integrity);
            }
            bytes.extend_from_slice(&chunk);
            on_progress(bytes.len());
        }
        Ok(bytes)
    }

    fn decode_key_material(value: &str) -> Result<String, UpdateError> {
        STANDARD
            .decode(value.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or(UpdateError::Integrity)
    }

    fn validate_envelope(bytes: &[u8], key: &str) -> Result<String, UpdateError> {
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|_| UpdateError::InvalidRelease)?;
        let key =
            PublicKey::decode(&decode_key_material(key)?).map_err(|_| UpdateError::Integrity)?;
        let signature = Signature::decode(&decode_key_material(&envelope.signature)?)
            .map_err(|_| UpdateError::Integrity)?;
        key.verify(envelope.payload.as_bytes(), &signature, true)
            .map_err(|_| UpdateError::Integrity)?;
        Ok(envelope.payload)
    }

    pub async fn check(window: &WebviewWindow) -> Result<Option<ReleaseInfo>, UpdateError> {
        let app = window.app_handle();
        let state = app.state::<Mutex<Option<Candidate>>>();
        let mut pending = state.try_lock().map_err(|_| UpdateError::Busy)?;
        *pending = None;
        let (target, extension) = installer().ok_or(UpdateError::Unavailable)?;
        let endpoint = Url::parse(&format!("{RELEASE_ROOT}latest/download/update.json"))
            .map_err(|_| UpdateError::InvalidRelease)?;
        let bytes = download(endpoint, MANIFEST_LIMIT, |_| {}).await?;
        let payload = validate_envelope(&bytes, KEY)?;
        let metadata: Metadata =
            serde_json::from_str(&payload).map_err(|_| UpdateError::InvalidRelease)?;
        if metadata.schema != 1
            || metadata.notes.len() > 8192
            || chrono::DateTime::parse_from_rfc3339(&metadata.pub_date).is_err()
            || chrono::NaiveDate::parse_from_str(&metadata.release_date, "%Y-%m-%d").is_err()
        {
            return Err(UpdateError::InvalidRelease);
        }
        let latest = metadata
            .version
            .parse::<semver::Version>()
            .map_err(|_| UpdateError::InvalidRelease)?;
        if !latest.pre.is_empty() || !latest.build.is_empty() {
            return Err(UpdateError::InvalidRelease);
        }
        if latest <= app.package_info().version {
            return Ok(None);
        }
        if windows_version::OsVersion::current().build < metadata.min_windows_build {
            return Err(UpdateError::Incompatible);
        }
        let asset = metadata
            .platforms
            .get(target)
            .ok_or(UpdateError::Incompatible)?;
        let expected = format!(
            "{RELEASE_ROOT}download/v{latest}/AuthenticatorDesk-{latest}-windows-x64{extension}"
        );
        if asset.url.as_str() != expected
            || !(1..=PACKAGE_LIMIT).contains(&asset.bytes)
            || asset.sha256.len() != 64
            || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(UpdateError::InvalidRelease);
        }
        let value = serde_json::from_str(&payload).map_err(|_| UpdateError::InvalidRelease)?;
        let update = app
            .updater_builder()
            .target(target)
            .on_before_exit({
                let app = app.clone();
                move || {
                    let _ = app.state::<crate::AppState>().lock();
                }
            })
            .build()
            .and_then(|updater| updater.restore_update(value))
            .map_err(|_| UpdateError::InvalidRelease)?
            .ok_or(UpdateError::InvalidRelease)?;
        let info = ReleaseInfo {
            version: metadata.version,
            date: metadata.release_date,
            notes: metadata.notes,
            bytes: asset.bytes,
        };
        let visible = info.clone();
        *pending = Some(Candidate {
            update,
            info,
            sha256: asset.sha256.clone(),
            checked_at: Instant::now(),
        });
        Ok(Some(visible))
    }

    pub async fn install(
        window: &WebviewWindow,
        version: &str,
        progress: Channel<DownloadProgress>,
    ) -> Result<(), UpdateError> {
        let state = window.state::<Mutex<Option<Candidate>>>();
        let pending = state.try_lock().map_err(|_| UpdateError::Busy)?;
        let candidate = pending
            .as_ref()
            .filter(|candidate| {
                candidate.info.version == version
                    && candidate.checked_at.elapsed() < Duration::from_secs(900)
            })
            .ok_or(UpdateError::InvalidRelease)?;
        let total = candidate.info.bytes;
        let bytes = download(candidate.update.download_url.clone(), total, |received| {
            let _ = progress.send(DownloadProgress { received, total });
        })
        .await?;
        if bytes.len() != total || digest(&bytes) != candidate.sha256 {
            return Err(UpdateError::Integrity);
        }
        candidate
            .update
            .verify(&bytes)
            .map_err(|_| UpdateError::Integrity)?;
        candidate
            .update
            .install(bytes)
            .map_err(|_| UpdateError::Install)
    }

    fn digest(data: &[u8]) -> String {
        use std::fmt::Write;
        let mut output = String::with_capacity(64);
        for byte in Sha256::digest(data) {
            write!(output, "{byte:02x}").expect("formatting into a String cannot fail");
        }
        output
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Metadata, PublicKey, Signature, decode_key_material, digest, release_url,
            validate_envelope,
        };
        use url::Url;

        #[test]
        fn accepts_only_expected_https_release_hosts() {
            for url in [
                "https://github.com/yuchen0x1/authenticator-desk/releases/latest/download/update.json",
                "https://release-assets.githubusercontent.com/github-production-release-asset/fixture",
            ] {
                assert!(release_url(&Url::parse(url).unwrap()));
            }
            for url in [
                "http://github.com/yuchen0x1/authenticator-desk/releases/latest/download/update.json",
                "https://github.com/other/project/releases/latest/download/update.json",
                "https://github.com.invalid/yuchen0x1/authenticator-desk/releases/latest/download/update.json",
                "https://x@release-assets.githubusercontent.com/object",
                "https://release-assets.githubusercontent.com:8443/object",
            ] {
                assert!(!release_url(&Url::parse(url).unwrap()));
            }
        }

        #[test]
        fn malformed_signature_and_envelope_are_rejected() {
            assert!(validate_envelope(b"untrusted data", "bad public key").is_err());
            assert!(
                validate_envelope(
                    br#"{"payload":"{}","signature":"invalid"}"#,
                    "bad public key",
                )
                .is_err()
            );
        }

        #[test]
        fn sha256_matches_known_bytes() {
            assert_eq!(
                digest(b"abc"),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        }

        #[test]
        fn release_metadata_matches_tauri_static_manifest_shape() {
            let release: tauri_plugin_updater::RemoteRelease = serde_json::from_value(
                serde_json::json!({
                    "version": "0.2.1",
                    "pub_date": "2026-11-18T00:00:00Z",
                    "notes": "Update notes",
                    "platforms": {
                        "windows-x86_64-nsis": {
                            "url": "https://github.com/yuchen0x1/authenticator-desk/releases/download/v0.2.1/AuthenticatorDesk-0.2.1-windows-x64-setup.exe",
                            "signature": "encoded signature",
                            "bytes": 1024,
                            "sha256": "0".repeat(64)
                        }
                    }
                }),
            )
            .unwrap();

            assert_eq!(release.version.to_string(), "0.2.1");
            assert_eq!(release.notes.as_deref(), Some("Update notes"));
            assert_eq!(
                release
                    .download_url("windows-x86_64-nsis")
                    .unwrap()
                    .as_str(),
                "https://github.com/yuchen0x1/authenticator-desk/releases/download/v0.2.1/AuthenticatorDesk-0.2.1-windows-x64-setup.exe"
            );
            assert!(release.signature("windows-x86_64-nsis").is_ok());
        }

        #[test]
        #[ignore = "reads release assets built with a signing key"]
        fn verify_staged_release() {
            if std::env::var("AUTHDESK_VERIFY_STAGED_RELEASE").as_deref() != Ok("1") {
                panic!("set AUTHDESK_VERIFY_STAGED_RELEASE=1 after staging signed assets");
            }
            let key = std::env::var("AUTHDESK_VERIFY_PUBLIC_KEY").unwrap();
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../release-assets");
            let bytes = std::fs::read(dir.join("update.json")).unwrap();
            let payload = validate_envelope(&bytes, &key)
                .unwrap_or_else(|_| panic!("invalid signed update manifest"));
            let envelope: super::Envelope = serde_json::from_slice(&bytes).unwrap();
            let altered = serde_json::json!({ "payload": payload.clone() + " ", "signature": envelope.signature });
            assert!(validate_envelope(&serde_json::to_vec(&altered).unwrap(), &key).is_err());
            let public_key = PublicKey::decode(&decode_key_material(&key).ok().unwrap()).unwrap();
            let metadata: Metadata = serde_json::from_str(&payload).unwrap();
            let expected_version = format!("version:{}", metadata.version);
            let raw: serde_json::Value = serde_json::from_str(&payload).unwrap();
            for (target, asset) in metadata.platforms {
                let filename = asset.url.path_segments().unwrap().next_back().unwrap();
                let mut bytes = std::fs::read(dir.join(filename)).unwrap();
                let signature = raw["platforms"][target]["signature"].as_str().unwrap();
                let signature =
                    Signature::decode(&decode_key_material(signature).ok().unwrap()).unwrap();
                assert!(
                    signature
                        .trusted_comment()
                        .split('\t')
                        .any(|field| field == expected_version)
                );
                assert_eq!(bytes.len(), asset.bytes);
                assert_eq!(digest(&bytes), asset.sha256);
                assert!(public_key.verify(&bytes, &signature, true).is_ok());
                bytes[0] ^= 1;
                assert!(public_key.verify(&bytes, &signature, true).is_err());
            }
        }
    }
}
