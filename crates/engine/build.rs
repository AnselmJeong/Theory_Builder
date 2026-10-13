fn main() {
    println!("cargo:rerun-if-changed=../../native/extract.swift");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let path =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("theory-extract");
        let status = std::process::Command::new("/usr/bin/swiftc")
            .args(["-O", "../../native/extract.swift", "-o"])
            .arg(&path)
            .status()
            .expect("swiftc is required to bundle PDF extraction");
        assert!(status.success(), "PDF helper compilation failed");
        println!("cargo:rustc-env=THEORY_EXTRACT_BUILD={}", path.display());
    }
}
