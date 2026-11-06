//! The desktop app: settings, the right-click entry and the commands the UI
//! calls. All real work lives in `core`.

use crate::core::{self, Env, Item, Leftover, Record};
use crate::{menu, trash};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

const MAX_RECENT: usize = 6;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    lang: String,
    theme: String,
    /// "#RRGGBB" for buttons and switches, or empty for the built-in color.
    accent: String,
    /// Folders scanned lately, newest first.
    recent: Vec<String>,
    /// Windows: the Explorer entry is wanted (macOS manages its own).
    context_menu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lang: "auto".into(), theme: "auto".into(), accent: String::new(), recent: vec![], context_menu: true }
    }
}

impl Settings {
    fn clean(mut self) -> Self {
        let hex = self.accent.len() == 7 && self.accent.starts_with('#') && self.accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !hex {
            self.accent.clear();
        }
        let mut seen = std::collections::HashSet::new();
        self.recent.retain(|p| !p.trim().is_empty() && seen.insert(p.clone()));
        self.recent.truncate(MAX_RECENT);
        self
    }
}

/// The last scan, kept here so that cleaning acts on what was shown, by
/// position, and never on paths the page sends.
#[derive(Serialize, Clone, Default)]
struct Found {
    roots: Vec<String>,
    /// [folder, reason] for folders Shed would not scan.
    refused: Vec<[String; 2]>,
    items: Vec<Item>,
}

struct Store {
    env: Env,
    settings: Mutex<Settings>,
    found: Mutex<Found>,
    /// Folders handed over by Finder or Explorer, waiting for the page.
    pending: Mutex<Vec<String>>,
}

impl Store {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    #[cfg(windows)]
    fn zh(&self) -> bool {
        match self.settings().lang.as_str() {
            "zh" => true,
            "en" => false,
            _ => sys_locale_zh(),
        }
    }
}

/// Only Windows needs this: for the label of the Explorer entry.
#[cfg(windows)]
fn sys_locale_zh() -> bool {
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-UICulture).Name"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).starts_with("zh"))
}

fn settings_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn save_settings(app: &AppHandle, s: &Settings) {
    if let Some(p) = settings_file(app) {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(p, serde_json::to_vec_pretty(s).unwrap_or_default());
    }
}

#[cfg(windows)]
fn menu_label(zh: bool) -> &'static str {
    if zh {
        "用 Shed 清理"
    } else {
        "Clean with Shed"
    }
}

/// Windows: make the registry match the setting (and the language).
fn sync_context_menu(store: &Store) {
    #[cfg(windows)]
    {
        let s = store.settings();
        menu::set(s.context_menu, menu_label(store.zh()));
    }
    let _ = store;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    version: String,
    platform: &'static str,
    home: String,
    settings: Settings,
    /// Usual project folders that exist here, for one-click scans.
    places: Vec<String>,
}

// ------------------------------------------------------------------ commands

#[tauri::command]
fn info(app: AppHandle, store: State<Store>) -> Info {
    let h = &store.env.home;
    let places = ["Projects", "Developer", "Desktop", "Documents", "code", "dev", "src", "GitHub", "repos", "workspace"]
        .iter()
        .map(|d| h.join(d))
        .filter(|p| p.is_dir())
        .map(|p| core::s(&p))
        .collect();
    Info {
        version: app.package_info().version.to_string(),
        platform: if cfg!(target_os = "macos") { "macos" } else if cfg!(windows) { "windows" } else { "linux" },
        home: core::s(h),
        settings: store.settings(),
        places,
    }
}

#[tauri::command]
fn set_settings(app: AppHandle, store: State<Store>, settings: Settings) -> Settings {
    let settings = settings.clean();
    save_settings(&app, &settings);
    *store.settings.lock().unwrap() = settings.clone();
    sync_context_menu(&store);
    settings
}

#[tauri::command]
async fn scan(app: AppHandle, store: State<'_, Store>, roots: Vec<String>) -> Result<Found, ()> {
    let mut found = Found::default();
    for root in &roots {
        let s = core::scan(&store.env, Path::new(root));
        found.roots.push(s.root.clone());
        match s.refused {
            Some(why) => found.refused.push([s.root, why]),
            None => found.items.extend(s.items),
        }
    }
    found.items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    {
        let mut st = store.settings.lock().unwrap();
        for r in found.roots.iter().rev().filter(|r| !found.refused.iter().any(|[f, _]| f == *r)) {
            st.recent.insert(0, r.clone());
        }
        *st = st.clone().clean();
        save_settings(&app, &st);
    }
    *store.found.lock().unwrap() = found.clone();
    Ok(found)
}

fn progress(app: &AppHandle) -> impl Fn(usize, usize) + Sync + '_ {
    move |done, total| {
        let _ = app.emit("progress", (done, total));
    }
}

/// Moves the items with these positions in the last scan to the Trash.
#[tauri::command]
async fn clean(app: AppHandle, store: State<'_, Store>, ids: Vec<usize>) -> Result<Record, ()> {
    let found = store.found.lock().unwrap().clone();
    let items: Vec<Item> = ids.iter().filter_map(|&i| found.items.get(i)).filter(|i| i.blocked.is_none()).cloned().collect();
    Ok(core::clean_items(&store.env, &found.roots.join(", "), &items, &trash::to_trash, &progress(&app)))
}

#[tauri::command]
async fn leftovers(store: State<'_, Store>) -> Result<Vec<Leftover>, ()> {
    Ok(core::leftovers(&store.env))
}

#[tauri::command]
async fn clean_leftovers(app: AppHandle, store: State<'_, Store>, ids: Vec<String>) -> Result<Record, ()> {
    Ok(core::clean_leftovers(&store.env, &ids, &trash::to_trash, &progress(&app)))
}

#[tauri::command]
fn history(store: State<Store>) -> Vec<Record> {
    core::history(&store.env)
}

#[tauri::command]
async fn undo(store: State<'_, Store>, time: u64) -> Result<Record, String> {
    core::undo(&store.env, time, &trash::restore)
}

#[tauri::command]
fn take_pending(store: State<Store>) -> Vec<String> {
    std::mem::take(&mut *store.pending.lock().unwrap())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog().file().blocking_pick_folder().and_then(|f| f.into_path().ok()).map(|p| core::s(&p))
}

#[tauri::command]
fn reveal(path: String) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = path;
}

#[tauri::command]
fn open_trash(store: State<Store>) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(store.env.home.join(".Trash")).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg("shell:RecycleBinFolder").spawn();
    let _ = store;
}

// ------------------------------------------------------------------ folders from outside

/// Folders after `--open` (Explorer passes one per launch).
fn open_args(args: &[String]) -> Vec<String> {
    args.windows(2).filter(|w| w[0] == "--open").map(|w| w[1].clone()).collect()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Hands folders to the page: queued, then announced, so none is lost
/// whether or not the page has loaded yet.
fn open_folders(app: &AppHandle, paths: Vec<String>) {
    show_main(app);
    if paths.is_empty() {
        return;
    }
    app.state::<Store>().pending.lock().unwrap().extend(paths);
    let _ = app.emit("open-folders", ());
}

pub fn run() {
    let Some(env) = Env::real() else {
        eprintln!("cannot find the home folder");
        return;
    };
    let first = open_args(&std::env::args().collect::<Vec<_>>());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| open_folders(app, open_args(&argv))))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let settings = settings_file(&handle)
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
                .unwrap_or_default()
                .clean();
            app.manage(Store { env, settings: Mutex::new(settings), found: Mutex::default(), pending: Mutex::new(first) });
            sync_context_menu(&app.state::<Store>());
            #[cfg(target_os = "macos")]
            {
                let h = handle.clone();
                menu::register(move |paths| {
                    let app = h.clone();
                    let _ = h.run_on_main_thread(move || open_folders(&app, paths));
                });
            }
            show_main(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![info, set_settings, scan, clean, leftovers, clean_leftovers, history, undo, take_pending, pick_folder, reveal, open_trash])
        .build(tauri::generate_context!())
        .expect("error while running Shed")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app);
            }
            let _ = (app, event);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_args_from_explorer() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(open_args(&a(&["shed", "--open", "C:\\code\\app"])), ["C:\\code\\app"]);
        assert!(open_args(&a(&["shed", "--open"])).is_empty());
        assert!(open_args(&a(&["shed"])).is_empty());
    }

    #[test]
    fn settings_are_cleaned() {
        let s = Settings { accent: "red".into(), recent: vec!["/a".into(), "".into(), "/a".into(), "/b".into()], ..Settings::default() }.clean();
        assert_eq!(s.accent, "");
        assert_eq!(s.recent, ["/a", "/b"]);
        let s = serde_json::from_str::<Settings>(r#"{"lang":"zh"}"#).unwrap().clean();
        assert!(s.context_menu && s.theme == "auto");
    }
}
