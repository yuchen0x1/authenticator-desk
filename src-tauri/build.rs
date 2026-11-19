fn main() {
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    println!("cargo:rerun-if-env-changed=AUTHDESK_UPDATER_PUBLIC_KEY");
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}
