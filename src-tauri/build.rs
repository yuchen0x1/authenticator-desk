fn main() {
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some()
        && std::path::Path::new("icons/icon.ico").exists()
    {
        tauri_build::build();
    }
}
