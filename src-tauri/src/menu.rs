//! The right-click entries.
//!
//! macOS: a Finder extension (macos/finder, built by build.rs and shipped in
//! Shed.app/Contents/PlugIns) adds "Clean This Mac with Shed" to the Desktop
//! and every folder's background menu, and "Clean with Shed" on folders. It
//! opens shed:// links; the app handles them. Here we only switch it on/off.
//!
//! Windows: verbs in the current user's registry for folders, folder
//! backgrounds and the Desktop background, starting Shed with
//! `--open <folder>` or `--computer`; a running Shed picks them up through
//! single-instance.

#[cfg(target_os = "macos")]
pub fn set(on: bool, _zh: bool) {
    use std::process::Command;
    // .../Shed.app/Contents/MacOS/shed -> .../Shed.app/Contents/PlugIns/ShedFinder.appex
    let appex = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.parent()?.join("PlugIns/ShedFinder.appex")));
    let Some(appex) = appex.filter(|p| p.exists()) else { return }; // not running from the bundle
    let _ = Command::new("pluginkit").arg("-a").arg(&appex).status();
    let _ = Command::new("pluginkit").args(["-e", if on { "use" } else { "ignore" }, "-i", "app.shed.desktop.finder"]).status();
}

#[cfg(windows)]
pub fn set(on: bool, zh: bool) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let reg = |args: &[&str]| Command::new("reg").args(args).creation_flags(0x0800_0000).output().is_ok_and(|o| o.status.success());
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().into_owned();
    let (folder, computer) = if zh { ("用 Shed 清理", "用 Shed 清理这台电脑") } else { ("Clean with Shed", "Clean This PC with Shed") };
    let keys = [
        (r"HKCU\Software\Classes\Directory\shell\Shed", folder, "--open \"%1\""),
        (r"HKCU\Software\Classes\Directory\Background\shell\Shed", folder, "--open \"%V\""),
        (r"HKCU\Software\Classes\DesktopBackground\Shell\Shed", computer, "--computer"),
    ];
    for (key, label, args) in keys {
        if !on {
            if reg(&["query", key]) {
                reg(&["delete", key, "/f"]);
            }
            continue;
        }
        let icon = format!("\"{exe}\",0");
        let cmd = format!("\"{exe}\" {args}");
        let command = format!(r"{key}\command");
        let _ = reg(&["add", key, "/ve", "/d", label, "/f"]) && reg(&["add", key, "/v", "Icon", "/d", &icon, "/f"]) && reg(&["add", &command, "/ve", "/d", &cmd, "/f"]);
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn set(_on: bool, _zh: bool) {}
