//! The desktop app: settings, the right-click entries and the commands the UI
//! calls. All real work lives in `core`.

use crate::core::{self, Env, Item, Leftover, Record};
use crate::{menu, trash};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt;
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
    /// The right-click entries (Finder extension, Explorer verbs) are wanted.
    context_menu: bool,
    /// Start in the menu bar when you log in.
    launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lang: "auto".into(), theme: "auto".into(), accent: String::new(), recent: vec![], context_menu: true, launch_at_login: false }
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
/// position or id, and never on paths the page sends.
#[derive(Serialize, Clone, Default)]
struct Found {
    roots: Vec<String>,
    /// The whole computer was scanned (and `leftovers` filled in).
    computer: bool,
    /// [folder, reason] for folders Shed would not scan.
    refused: Vec<[String; 2]>,
    items: Vec<Item>,
    leftovers: Vec<Leftover>,
}

/// What Finder, Explorer or a shed:// link asked for, waiting for the page.
#[derive(Serialize, Clone, Default, Debug, PartialEq)]
struct Request {
    computer: bool,
    folders: Vec<String>,
}

struct Store {
    env: Env,
    settings: Mutex<Settings>,
    found: Mutex<Found>,
    tray: Mutex<Option<[MenuItem<tauri::Wry>; 4]>>,
}

/// Managed from the very start: a shed:// link that launched the app
/// arrives before `setup` has run.
struct Pending(Mutex<Request>);

impl Store {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
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

/// The menu bar and the Explorer entries speak the app's language.
fn zh(s: &Settings) -> bool {
    match s.lang.as_str() {
        "zh" => true,
        "en" => false,
        _ => sys_locale_zh(),
    }
}

fn sys_locale_zh() -> bool {
    ["LC_ALL", "LANG"].iter().any(|k| std::env::var(k).is_ok_and(|v| v.starts_with("zh"))) || {
        #[cfg(target_os = "macos")]
        let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output();
        #[cfg(windows)]
        let out = std::process::Command::new("powershell").args(["-NoProfile", "-Command", "(Get-UICulture).Name"]).output();
        #[cfg(not(any(target_os = "macos", windows)))]
        let out: std::io::Result<std::process::Output> = Err(std::io::ErrorKind::Unsupported.into());
        // "(\n    \"zh-Hans-CN\"," on macOS, "zh-CN" on Windows: the first language.
        out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim_start_matches(['(', ' ', '\n', '"']).starts_with("zh"))
    }
}

/// Make the right-click entries match the setting, off the main thread
/// (pluginkit and reg take a moment).
fn sync_context_menu(s: &Settings) {
    let (on, zh) = (s.context_menu, zh(s));
    std::thread::spawn(move || menu::set(on, zh));
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
    let old = store.settings();
    if settings.context_menu != old.context_menu || settings.lang != old.lang {
        sync_context_menu(&settings);
    }
    if settings.launch_at_login != old.launch_at_login {
        let al = app.autolaunch();
        let _ = if settings.launch_at_login { al.enable() } else { al.disable() };
    }
    *store.settings.lock().unwrap() = settings.clone();
    relabel_tray(&store);
    settings
}

/// Scans folders, or with `computer` the whole home folder plus what AI
/// tools and package managers keep outside it.
#[tauri::command]
async fn scan(app: AppHandle, store: State<'_, Store>, roots: Vec<String>, computer: bool) -> Result<Found, ()> {
    let roots = if computer { vec![core::s(&store.env.home)] } else { roots };
    let mut found = Found { computer, ..Default::default() };
    for root in &roots {
        let s = core::scan(&store.env, Path::new(root));
        found.roots.push(s.root.clone());
        match s.refused {
            Some(why) => found.refused.push([s.root, why]),
            None => found.items.extend(s.items),
        }
    }
    found.items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    if computer {
        found.leftovers = core::leftovers(&store.env);
    } else {
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

/// Moves the items with these positions in the last scan, and the leftovers
/// with these ids, to the Trash.
#[tauri::command]
async fn clean(app: AppHandle, store: State<'_, Store>, ids: Vec<usize>, leftover_ids: Vec<String>) -> Result<Record, ()> {
    let found = store.found.lock().unwrap().clone();
    let items: Vec<Item> = ids.iter().filter_map(|&i| found.items.get(i)).filter(|i| i.blocked.is_none()).cloned().collect();
    let leftover_ids: Vec<String> = leftover_ids.into_iter().filter(|id| found.leftovers.iter().any(|l| &l.id == id)).collect();
    let root = if found.computer { String::new() } else { found.roots.join(", ") };
    Ok(core::clean(&store.env, &root, &items, &leftover_ids, &trash::to_trash, &progress(&app)))
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
fn take_pending(pending: State<Pending>) -> Request {
    std::mem::take(&mut *pending.0.lock().unwrap())
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

// ------------------------------------------------------------------ requests from outside

/// Explorer starts Shed with `--computer` (the Desktop) or `--open <folder>`.
fn from_args(args: &[String]) -> Request {
    Request { computer: args.iter().any(|a| a == "--computer"), folders: args.windows(2).filter(|w| w[0] == "--open").map(|w| w[1].clone()).collect() }
}

/// The Finder extension opens `shed://computer` or `shed://open?path=…`.
fn from_link(link: &str) -> Request {
    let Some(rest) = link.strip_prefix("shed://") else { return Request::default() };
    let (what, query) = rest.split_once('?').unwrap_or((rest, ""));
    match what.trim_end_matches('/') {
        "computer" => Request { computer: true, folders: vec![] },
        "open" => Request { computer: false, folders: query.split('&').filter_map(|kv| kv.strip_prefix("path=")).map(percent_decode).filter(|p| !p.is_empty()).collect() },
        _ => Request::default(),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%')
            .then(|| b.get(i + 1..i + 3))
            .flatten()
            .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
            .and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Hands a request to the page: queued, then announced, so none is lost
/// whether or not the page has loaded yet.
fn handle(app: &AppHandle, req: Request) {
    show_main(app);
    if req == Request::default() {
        return;
    }
    let pending = app.state::<Pending>();
    let mut p = pending.0.lock().unwrap();
    p.computer |= req.computer;
    p.folders.extend(req.folders);
    drop(p);
    let _ = app.emit("open-request", ());
}

// ------------------------------------------------------------------ menu bar

fn tray_labels(zh: bool) -> [&'static str; 4] {
    if zh {
        ["清理整台电脑…", "打开 Shed", "撤销上一次清理", "退出 Shed"]
    } else {
        ["Clean This Computer…", "Open Shed", "Undo Last Clean", "Quit Shed"]
    }
}

fn relabel_tray(store: &Store) {
    if let Some(items) = store.tray.lock().unwrap().as_ref() {
        for (item, label) in items.iter().zip(tray_labels(zh(&store.settings()))) {
            let _ = item.set_text(label);
        }
    }
}

fn undo_last(app: &AppHandle) {
    let store = app.state::<Store>();
    let result = match core::history(&store.env).into_iter().find(|r| !r.undone) {
        Some(r) => core::undo(&store.env, r.time, &trash::restore),
        None => Err("nothing".into()),
    };
    show_main(app);
    let _ = app.emit("undone", result);
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let store = app.state::<Store>();
    let [l_all, l_open, l_undo, l_quit] = tray_labels(zh(&store.settings()));
    let all = MenuItem::with_id(app, "all", l_all, true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", l_open, true, None::<&str>)?;
    let undo = MenuItem::with_id(app, "undo", l_undo, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", l_quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&all, &open, &undo, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Shed")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "all" => handle(app, Request { computer: true, folders: vec![] }),
            "open" => show_main(app),
            "undo" => undo_last(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    *store.tray.lock().unwrap() = Some([all, open, undo, quit]);
    Ok(())
}

pub fn run() {
    let Some(env) = Env::real() else {
        eprintln!("cannot find the home folder");
        return;
    };
    let first = from_args(&std::env::args().collect::<Vec<_>>());
    tauri::Builder::default()
        .manage(Pending(Mutex::new(first)))
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| handle(app, from_args(&argv))))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(move |app| {
            // A menu bar app: no Dock icon, the window comes and goes.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let settings = settings_file(&handle)
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
                .unwrap_or_default()
                .clean();
            sync_context_menu(&settings);
            app.manage(Store { env, settings: Mutex::new(settings), found: Mutex::default(), tray: Mutex::new(None) });
            build_tray(app)?;
            // Started at login: wait in the menu bar.
            if !std::env::args().any(|a| a == "--hidden") {
                show_main(&handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![info, set_settings, scan, clean, history, undo, take_pending, pick_folder, reveal, open_trash])
        .on_window_event(|window, event| {
            // Closing the window keeps Shed in the menu bar.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running Shed")
        .run(|app, event| match event {
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => show_main(app),
            // shed:// links (macOS delivers them as an "open URL" event).
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Opened { urls } => urls.iter().for_each(|u| handle(app, from_link(u.as_str()))),
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_from_explorer() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_args(&a(&["shed", "--open", "C:\\code\\app"])).folders, ["C:\\code\\app"]);
        assert!(from_args(&a(&["shed", "--open"])).folders.is_empty());
        assert!(from_args(&a(&["shed", "--computer"])).computer);
        assert_eq!(from_args(&a(&["shed"])), Request::default());
    }

    #[test]
    fn requests_from_links() {
        assert!(from_link("shed://computer").computer);
        assert!(from_link("shed://computer/").computer);
        let r = from_link("shed://open?path=/Users/me/My%20App%20%26%20Co&path=/tmp/a%2Bb%25");
        assert_eq!(r.folders, ["/Users/me/My App & Co", "/tmp/a+b%"]);
        assert_eq!(from_link("shed://open?path=%E4%B8%AD%E6%96%87").folders, ["中文"]);
        assert_eq!(from_link("shed://delete?path=/"), Request::default());
        assert_eq!(from_link("https://example.com"), Request::default());
        assert_eq!(percent_decode("bad%zz%4"), "bad%zz%4");
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
