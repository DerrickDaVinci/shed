fn main() {
    // The Finder extension ships inside Shed.app (bundle.macOS.files).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for f in ["build.sh", "ShedFinder.m", "Info.plist", "ShedFinder.entitlements"] {
            println!("cargo:rerun-if-changed=macos/finder/{f}");
        }
        let ok = std::process::Command::new("sh").arg("macos/finder/build.sh").status().is_ok_and(|s| s.success());
        assert!(ok, "building the Finder extension failed (macos/finder/build.sh)");
    }
    tauri_build::build()
}
