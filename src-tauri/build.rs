fn main() {
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    println!("cargo:rerun-if-env-changed=AUTHDESK_UPDATER_PUBLIC_KEY");
    if std::env::var("PROFILE").as_deref() == Ok("release")
        && std::env::var("TARGET").is_ok_and(|target| target.ends_with("pc-windows-msvc"))
        && std::env::var_os("CARGO_FEATURE_DESKTOP").is_some()
    {
        // Keep local debug builds debuggable; release packages need no PDB reference.
        println!("cargo:rustc-link-arg-bin=authenticator-desk=/DEBUG:NONE");
    }
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}
