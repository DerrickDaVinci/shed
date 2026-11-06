//! The right-click entry. On macOS a Finder service ("Clean with Shed" under
//! Quick Actions / Services) declared in Info.plist: Finder hands the selected
//! folders to the running app, or launches it first. On Windows a verb for
//! folders in the current user's registry, which starts Shed with
//! `--open <folder>`; a running Shed picks it up through single-instance.

#[cfg(target_os = "macos")]
pub use mac::register;

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{define_class, msg_send, ClassType, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeFileURL, NSUpdateDynamicServices};
    use objc2_foundation::{NSString, NSURL};
    use std::sync::OnceLock;

    type Handler = Box<dyn Fn(Vec<String>) + Send + Sync>;
    static HANDLER: OnceLock<Handler> = OnceLock::new();

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "ShedServiceProvider"]
        struct Provider;

        impl Provider {
            /// NSMessage "cleanFolders" in Info.plist.
            #[unsafe(method(cleanFolders:userData:error:))]
            fn clean_folders(&self, pb: &NSPasteboard, _data: Option<&NSString>, _error: *mut *mut NSString) {
                if let Some(h) = HANDLER.get() {
                    h(file_paths(pb));
                }
            }
        }
    );

    fn file_paths(pb: &NSPasteboard) -> Vec<String> {
        let ty = unsafe { NSPasteboardTypeFileURL };
        let Some(items) = pb.pasteboardItems() else { return vec![] };
        items
            .iter()
            .filter_map(|it| {
                let text = it.stringForType(ty)?;
                let url = NSURL::URLWithString(&text)?;
                // Finder may send file reference URLs (file:///.file/id=…).
                Some(url.filePathURL()?.path()?.to_string())
            })
            .collect()
    }

    /// Call once on the main thread while the app starts.
    pub fn register(handler: impl Fn(Vec<String>) + Send + Sync + 'static) {
        let _ = HANDLER.set(Box::new(handler));
        let Some(mtm) = MainThreadMarker::new() else { return };
        let provider: Retained<Provider> = unsafe { msg_send![Provider::class(), new] };
        let obj: &AnyObject = &provider;
        unsafe { NSApplication::sharedApplication(mtm).setServicesProvider(Some(obj)) };
        std::mem::forget(provider); // lives as long as the app
        NSUpdateDynamicServices();
    }
}

#[cfg(windows)]
pub use win::set;

#[cfg(windows)]
mod win {
    use std::process::Command;

    const KEYS: [(&str, &str); 2] = [
        // Right-click on a folder…
        (r"HKCU\Software\Classes\Directory\shell\Shed", "%1"),
        // …and on the empty space inside an open folder.
        (r"HKCU\Software\Classes\Directory\Background\shell\Shed", "%V"),
    ];

    fn reg(args: &[&str]) -> bool {
        use std::os::windows::process::CommandExt;
        Command::new("reg").args(args).creation_flags(0x0800_0000).output().is_ok_and(|o| o.status.success())
    }

    /// Adds (with this label) or removes the entry. Re-adding refreshes the
    /// path, so a moved or updated Shed keeps working.
    pub fn set(on: bool, label: &str) -> bool {
        let Ok(exe) = std::env::current_exe() else { return false };
        let exe = exe.to_string_lossy().into_owned();
        KEYS.iter().all(|(key, arg)| {
            if !on {
                return !reg(&["query", key]) || reg(&["delete", key, "/f"]);
            }
            let icon = format!("\"{exe}\",0");
            let cmd = format!("\"{exe}\" --open \"{arg}\"");
            let command = format!(r"{key}\command");
            reg(&["add", key, "/ve", "/d", label, "/f"]) && reg(&["add", key, "/v", "Icon", "/d", &icon, "/f"]) && reg(&["add", &command, "/ve", "/d", &cmd, "/f"])
        })
    }
}
