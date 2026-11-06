//! What Shed can clean and how. The app and the command line share all of it.
//!
//! Two kinds of things are found:
//!   - build output inside projects (`scan`): node_modules next to a
//!     package.json, target next to a Cargo.toml, a Python virtualenv…
//!   - leftovers of AI coding tools and download caches (`leftovers`).
//!
//! Nothing is ever deleted: cleaning moves folders to the Trash (Recycle Bin
//! on Windows) and records where they went, so every clean can be undone.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Env {
    pub home: PathBuf,
    /// ~/Library/Application Support on macOS, %APPDATA% on Windows.
    pub app_data: PathBuf,
    /// ~/Library/Caches on macOS, %LOCALAPPDATA% on Windows.
    pub local: PathBuf,
    pub documents: PathBuf,
    /// Our own folder: the history of cleans.
    pub data: PathBuf,
}

impl Env {
    pub fn real() -> Option<Env> {
        let app_data = dirs::data_dir()?;
        let home = dirs::home_dir()?;
        let local = if cfg!(windows) { dirs::data_local_dir()? } else { dirs::cache_dir()? };
        let documents = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));
        Some(Env { data: app_data.join("app.shed.desktop"), app_data, local, documents, home })
    }
}

pub fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The real path, without Windows' `\\?\` prefix (git and people read it).
pub fn canon(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match c.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => c,
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn mtime(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(windows)]
fn command(prog: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(prog);
    c.creation_flags(0x0800_0000); // no console window
    c
}
#[cfg(not(windows))]
fn command(prog: &str) -> Command {
    Command::new(prog)
}

// ------------------------------------------------------------------ rules

/// A kind of project, known by a file (or folder) at its root, and the
/// folders inside it that its tools rebuild. The table started from kondo's
/// (MIT, github.com/tbillington/kondo) and adds web frameworks, PlatformIO
/// and Xcode. `*.ext` matches by suffix; `a/b` is a folder further down.
struct Rule {
    kind: &'static str,
    markers: &'static [&'static str],
    dirs: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule { kind: "Rust", markers: &["Cargo.toml"], dirs: &["target", ".xwin-cache"] },
    Rule {
        kind: "Node",
        markers: &["package.json"],
        dirs: &[
            "node_modules", ".next", ".nuxt", ".output", ".svelte-kit", ".turbo", ".parcel-cache", ".angular", ".expo", ".docusaurus",
            ".vite", "dist", "build", "out",
            // React Native
            "android/build", "android/.gradle", "ios/build", "ios/Pods",
        ],
    },
    Rule {
        kind: "Python",
        markers: &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt", "Pipfile", "uv.lock", "*.py", "*.ipynb"],
        dirs: &[".pytest_cache", ".mypy_cache", ".ruff_cache", ".tox", ".nox", "__pypackages__", ".ipynb_checkpoints", "build", "dist"],
    },
    Rule { kind: "Pixi", markers: &["pixi.toml"], dirs: &[".pixi"] },
    Rule { kind: "PlatformIO", markers: &["platformio.ini"], dirs: &[".pio"] },
    Rule { kind: "Gradle", markers: &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"], dirs: &[".gradle", "build"] },
    Rule { kind: "Maven", markers: &["pom.xml"], dirs: &["target"] },
    Rule { kind: "SBT", markers: &["build.sbt"], dirs: &["target", "project/target"] },
    Rule { kind: "CMake", markers: &["CMakeLists.txt"], dirs: &["build", "cmake-build-debug", "cmake-build-release"] },
    Rule { kind: "Swift", markers: &["Package.swift"], dirs: &[".build", ".swiftpm"] },
    Rule { kind: "Xcode", markers: &["*.xcodeproj", "*.xcworkspace"], dirs: &["DerivedData", "build"] },
    Rule { kind: "CocoaPods", markers: &["Podfile"], dirs: &["Pods"] },
    Rule { kind: "Flutter", markers: &["pubspec.yaml"], dirs: &[".dart_tool", "build", "linux/flutter/ephemeral", "windows/flutter/ephemeral"] },
    Rule { kind: "Haskell", markers: &["stack.yaml"], dirs: &[".stack-work"] },
    Rule { kind: "Haskell", markers: &["cabal.project"], dirs: &["dist-newstyle"] },
    Rule { kind: "Elixir", markers: &["mix.exs"], dirs: &["_build", ".elixir_ls", ".elixir-tools", ".lexical"] },
    Rule { kind: "Zig", markers: &["build.zig"], dirs: &["zig-cache", ".zig-cache", "zig-out"] },
    Rule { kind: "Godot", markers: &["project.godot"], dirs: &[".godot"] },
    Rule { kind: ".NET", markers: &["*.csproj", "*.fsproj", "*.sln"], dirs: &["bin", "obj"] },
    Rule { kind: "Unity", markers: &["Assembly-CSharp.csproj"], dirs: &["Library", "Temp", "Obj", "Logs", "MemoryCaptures", "Build", "Builds"] },
    Rule { kind: "Unreal", markers: &["*.uproject"], dirs: &["Binaries", "Build", "Saved", "DerivedDataCache", "Intermediate"] },
    Rule { kind: "PHP", markers: &["composer.json"], dirs: &["vendor"] },
    Rule { kind: "Terraform", markers: &[".terraform.lock.hcl"], dirs: &[".terraform"] },
    Rule { kind: "Turborepo", markers: &["turbo.json"], dirs: &[".turbo"] },
];

/// Folder names that are often build output but can just as well be the
/// work itself (a site's `build`, a vendored library, a game's `Saved`
/// configs). They are only ticked when git says they are ignored.
const MAYBE: &[&str] = &["build", "dist", "out", "bin", "obj", "vendor", "Build", "Builds", "Binaries", "Saved", "Logs", "zig-out"];

/// Never looked into: version control, app bundles, the Trash, and
/// node_modules (it is found or not, but never searched: every package in it
/// looks like a project of its own).
fn never_enter(name: &str) -> bool {
    matches!(name, ".git" | ".hg" | ".svn" | ".Trash" | ".Trashes" | "$RECYCLE.BIN" | "System Volume Information" | "node_modules" | "site-packages")
        || [".app", ".photoslibrary", ".framework", ".xcarchive"].iter().any(|e| name.ends_with(e))
}

fn matches_marker(names: &[String], marker: &str) -> bool {
    match marker.strip_prefix('*') {
        Some(suffix) => names.iter().any(|n| n.ends_with(suffix) && n.len() > suffix.len()),
        None => names.iter().any(|n| n == marker),
    }
}

fn rules_for(names: &[String]) -> Vec<&'static Rule> {
    RULES.iter().filter(|r| r.markers.iter().any(|m| matches_marker(names, m))).collect()
}

fn entry_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

/// A real folder, not a link to one.
fn is_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

fn is_venv(p: &Path) -> bool {
    p.join("pyvenv.cfg").is_file()
}

/// What `path` is, if Shed may clean it: (kind, folder name as the rule
/// spells it, the project it belongs to). Used for a folder picked directly
/// and to check every folder again right before it is moved.
fn classify(path: &Path) -> Option<(&'static str, String, PathBuf)> {
    if !is_dir(path) {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().into_owned();
    let parent = path.parent()?;
    if name == "__pycache__" {
        return Some(("Python", name, parent.into()));
    }
    if is_venv(path) {
        return Some(("venv", name, parent.into()));
    }
    // The project may be up to three levels up (android/build, project/target…).
    let mut root = parent;
    let mut rel = vec![name];
    for _ in 0..3 {
        let joined = rel.iter().rev().cloned().collect::<Vec<_>>().join("/");
        if let Some(r) = rules_for(&entry_names(root)).into_iter().find(|r| r.dirs.contains(&joined.as_str())) {
            return Some((r.kind, joined, root.into()));
        }
        rel.push(root.file_name()?.to_string_lossy().into_owned());
        root = root.parent()?;
    }
    None
}

// ------------------------------------------------------------------ scan

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Item {
    /// The folder name ("node_modules", "src-tauri/target" shows as "target").
    pub name: String,
    /// "Rust", "Node", "venv"…
    pub kind: String,
    /// The project folder it belongs to.
    pub project: String,
    /// Usually one folder; all __pycache__ folders of a project are one item.
    pub paths: Vec<String>,
    pub bytes: u64,
    /// Ticked in the preview.
    pub selected: bool,
    /// Why it cannot be cleaned at all ("tracked").
    pub blocked: Option<String>,
    /// Codes the UI explains: ignored, maybe, recent, busy, noManifest, worktree.
    pub notes: Vec<String>,
    /// Programs running inside the project.
    pub busy: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Scan {
    pub root: String,
    pub items: Vec<Item>,
    /// Why the folder was not scanned ("system", "library", "missing").
    pub refused: Option<String>,
}

struct Found {
    path: PathBuf,
    kind: &'static str,
    name: String,
    project: PathBuf,
    maybe: bool,
}

const MAX_DEPTH: usize = 24;

/// Folders Shed refuses to scan: the system, other users, app data.
fn refuse(env: &Env, root: &Path) -> Option<&'static str> {
    if !root.is_dir() {
        return Some("missing");
    }
    if env.home.starts_with(root) && env.home != root {
        return Some("system"); // "/", "/Users", "C:\"
    }
    let system: &[&str] = if cfg!(windows) {
        &["C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)", "C:\\ProgramData"]
    } else {
        &["/System", "/Library", "/Applications", "/usr", "/bin", "/sbin", "/private", "/opt", "/etc", "/var", "/cores"]
    };
    let ours = root.starts_with(&env.home) || root.starts_with(canon(&std::env::temp_dir()));
    if !ours && system.iter().any(|p| root.starts_with(p)) {
        return Some("system");
    }
    if ["Library", "AppData"].iter().any(|d| root.starts_with(env.home.join(d))) {
        return Some("library");
    }
    None
}

struct Walk<'a> {
    env: &'a Env,
    root: PathBuf,
    /// The folder picked is one project (it has a .git or a project file),
    /// not a folder of projects.
    one_project: bool,
    found: Vec<Found>,
    taken: HashSet<PathBuf>,
}

impl Walk<'_> {
    /// The project an item is listed under: the folder picked, or the
    /// project folder in it, so that an app's node_modules and its
    /// src-tauri/target, or all of a repo's __pycache__, show together.
    fn project_of(&self, path: &Path) -> PathBuf {
        match path.strip_prefix(&self.root).ok().and_then(|r| r.components().next()) {
            Some(first) if !self.one_project && path != self.root.join(first) => self.root.join(first),
            _ => self.root.clone(),
        }
    }

    fn add(&mut self, path: PathBuf, kind: &'static str, name: String) {
        if self.taken.insert(path.clone()) {
            let maybe = MAYBE.contains(&name.rsplit('/').next().unwrap_or(&name));
            let project = self.project_of(&path);
            self.found.push(Found { path, kind, name, project, maybe });
        }
    }

    fn dir(&mut self, dir: &Path, depth: usize) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<(String, bool)> = rd
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().is_ok_and(|t| t.is_dir())))
            .collect();
        entries.sort();
        let names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
        for r in rules_for(&names) {
            for d in r.dirs {
                let p = dir.join(d);
                if is_dir(&p) {
                    self.add(p, r.kind, d.to_string());
                }
            }
        }
        let at_home = dir == self.env.home;
        for (name, is_dir) in entries {
            if !is_dir || never_enter(&name) {
                continue;
            }
            // Tool installs (~/.vscode, ~/.claude/plugins…) need their node_modules.
            if at_home && (name.starts_with('.') || name == "Library" || name == "AppData" || name == "Applications") {
                continue;
            }
            let sub = dir.join(&name);
            if self.taken.contains(&sub) {
                continue;
            }
            if name == "__pycache__" {
                self.add(sub, "Python", name);
            } else if is_venv(&sub) {
                self.add(sub, "venv", name);
            } else if depth < MAX_DEPTH {
                self.dir(&sub, depth + 1);
            }
        }
    }
}

/// Everything under `root` that Shed would clean, largest first.
pub fn scan(env: &Env, root: &Path) -> Scan {
    let root = canon(root);
    let mut out = Scan { root: s(&root), ..Default::default() };
    if let Some(why) = refuse(env, &root) {
        out.refused = Some(why.into());
        return out;
    }
    let names = entry_names(&root);
    let one_project = names.iter().any(|n| n == ".git") || !rules_for(&names).is_empty();
    let mut walk = Walk { env, root: root.clone(), one_project, found: vec![], taken: HashSet::new() };
    match classify(&root) {
        // The folder picked is itself build output (a node_modules, a .venv).
        Some((kind, name, project)) => {
            walk.add(root.clone(), kind, name);
            walk.found[0].project = project;
        }
        None => walk.dir(&root, 0),
    }
    out.items = measure(group(walk.found), &cwds());
    out
}

/// One item per folder, except that a project's __pycache__ folders become one.
fn group(found: Vec<Found>) -> Vec<(Item, bool)> {
    let mut items: Vec<(Item, bool)> = vec![];
    let mut caches: HashMap<PathBuf, usize> = HashMap::new();
    for f in found {
        if f.name == "__pycache__" {
            if let Some(&i) = caches.get(&f.project) {
                items[i].0.paths.push(s(&f.path));
                continue;
            }
            caches.insert(f.project.clone(), items.len());
        }
        let item = Item { name: f.name, kind: f.kind.into(), project: s(&f.project), paths: vec![s(&f.path)], ..Default::default() };
        items.push((item, f.maybe));
    }
    items
}

/// What git says about a folder: is anything in it tracked, is it ignored.
/// `None` when it is not in a repository or git is missing.
fn git_state(path: &Path) -> (bool, Option<bool>) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return (false, None) };
    let git = |args: &[&str]| command("git").arg("-C").arg(parent).args(args).arg("--").arg(name).stdin(Stdio::null()).stderr(Stdio::null()).output();
    let tracked = git(&["ls-files", "-z"]).is_ok_and(|o| o.status.success() && !o.stdout.is_empty());
    let ignored = git(&["check-ignore", "-q"]).ok().and_then(|o| match o.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    });
    (tracked, ignored)
}

const MANIFESTS: &[&str] = &["requirements.txt", "pyproject.toml", "Pipfile", "Pipfile.lock", "poetry.lock", "uv.lock", "setup.py", "environment.yml", "*requirements*.txt"];

const DAY: u64 = 86_400;

/// Sizes, git and the running programs decide what is ticked.
fn measure(mut items: Vec<(Item, bool)>, cwds: &[(String, String)]) -> Vec<Item> {
    let seen = Mutex::new(HashSet::new());
    par_each(&mut items, |(item, maybe)| judge(item, *maybe, cwds, &seen));
    let mut out: Vec<Item> = items.into_iter().map(|(i, _)| i).collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

/// Runs `f` on every item, a few threads pulling from one queue (sizes vary
/// wildly: one target folder can take longer than everything else).
fn par_each<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync) {
    let queue = Mutex::new(items.iter_mut());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                while let Some(item) = {
                    let next = queue.lock().unwrap().next();
                    next
                } {
                    f(item);
                }
            });
        }
    });
}

fn judge(item: &mut Item, maybe: bool, cwds: &[(String, String)], seen: &Mutex<HashSet<(u64, u64)>>) {
    let first = PathBuf::from(&item.paths[0]);
    for p in &item.paths {
        item.bytes += size(Path::new(p), seen);
    }
    item.selected = true;
    let note = |item: &mut Item, n: &str| item.notes.push(n.into());

    let (tracked, ignored) = if item.name == "__pycache__" { (false, None) } else { git_state(&first) };
    if tracked {
        item.blocked = Some("tracked".into());
        item.selected = false;
        return;
    }
    if ignored == Some(true) {
        note(item, "ignored");
    } else if maybe {
        note(item, "maybe");
        item.selected = false;
    }
    let listed = |d: &Path| MANIFESTS.iter().any(|m| matches_marker(&entry_names(d), m));
    if item.kind == "venv" && !first.parent().is_some_and(listed) && !listed(Path::new(&item.project)) {
        note(item, "noManifest");
        item.selected = false;
    }
    item.busy = busy_in(cwds, &item.project);
    if !item.busy.is_empty() {
        note(item, "busy");
        item.selected = false;
    }
    let modified = std::fs::metadata(&first).map(|m| mtime(&m)).unwrap_or(0);
    if now().saturating_sub(modified) < DAY {
        note(item, "recent");
    }
    let p = item.paths[0].replace('\\', "/");
    if [".claude/worktrees/", ".codex/worktrees/", ".cursor/worktrees/", "/.worktrees/"].iter().any(|w| p.contains(w)) {
        note(item, "worktree");
    }
}

/// Space a folder takes on disk. Hard links (pnpm, uv) count once.
pub fn size(path: &Path, seen: &Mutex<HashSet<(u64, u64)>>) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue }; // does not follow links
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += disk_bytes(&m, seen);
            }
        }
    }
    total
}

#[cfg(unix)]
fn disk_bytes(m: &std::fs::Metadata, seen: &Mutex<HashSet<(u64, u64)>>) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if m.nlink() > 1 && !seen.lock().unwrap().insert((m.dev(), m.ino())) {
        return 0;
    }
    m.blocks() * 512
}
#[cfg(not(unix))]
fn disk_bytes(m: &std::fs::Metadata, _: &Mutex<HashSet<(u64, u64)>>) -> u64 {
    m.len()
}

pub fn size_of(path: &Path) -> u64 {
    size(path, &Mutex::new(HashSet::new()))
}

// ------------------------------------------------------------------ what is running

/// Working directories of running programs (dev servers, agents, shells).
/// Deleting node_modules under a running dev server breaks it.
fn cwds() -> Vec<(String, String)> {
    if cfg!(windows) {
        return vec![]; // Windows refuses to move folders that are in use.
    }
    let out = command("lsof").args(["-nP", "-d", "cwd", "-Fpcn"]).stderr(Stdio::null()).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let me = std::process::id().to_string();
    let (mut pid, mut cmd, mut found) = (String::new(), String::new(), vec![]);
    for line in text.lines() {
        match line.split_at_checked(1) {
            Some(("p", v)) => pid = v.into(),
            Some(("c", v)) => cmd = v.into(),
            Some(("n", v)) if pid != me && cmd != "lsof" => found.push((cmd.clone(), v.to_string())),
            _ => {}
        }
    }
    found
}

fn inside(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir).is_some_and(|r| r.is_empty() || r.starts_with('/') || r.starts_with('\\'))
}

/// Programs working inside the project, other than shells and coding agents
/// (an agent waiting in the folder does not need node_modules; the dev
/// server or watcher it started does, and shows up on its own).
fn busy_in(cwds: &[(String, String)], project: &str) -> Vec<String> {
    // "disclaimer" wraps each Claude Code session of the Claude desktop app.
    const QUIET: &[&str] = &["zsh", "bash", "fish", "sh", "-zsh", "-bash", "login", "tmux", "screen", "nu", "claude", "disclaimer", "codex", "cursor-agent", "gemini", "aider"];
    let mut names: Vec<String> = cwds.iter().filter(|(n, c)| inside(c, project) && !QUIET.contains(&n.as_str())).map(|(n, _)| n.clone()).collect();
    names.sort();
    names.dedup();
    names
}

fn processes() -> Vec<String> {
    #[cfg(windows)]
    let out = command("tasklist").args(["/FO", "CSV", "/NH"]).output();
    #[cfg(not(windows))]
    let out = command("ps").args(["-axo", "command="]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines().map(|l| l.trim().trim_matches('"').to_string()).collect()
}

/// The program name of a `ps` command line or a `tasklist` row.
fn program(line: &str) -> &str {
    let exe = if cfg!(windows) { line.split("\",").next().unwrap_or("") } else { line.split(".app/").next().unwrap_or(line) };
    let exe = exe.split(" --").next().unwrap_or(exe);
    exe.rsplit(['/', '\\']).next().unwrap_or("").trim_end_matches(".exe").trim_end_matches(".app")
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Running {
    pub claude: bool,
    pub codex: bool,
}

fn running(procs: &[String]) -> Running {
    // Case matters: "Claude" is the desktop app, "claude" the Claude Code CLI.
    let names: Vec<&str> = procs.iter().map(|p| program(p)).collect();
    let has = |n: &str| names.contains(&n);
    Running { claude: has("Claude"), codex: has("Codex") || has("ChatGPT") || has("codex") }
}

// ------------------------------------------------------------------ leftovers

/// Things outside projects: what AI tools leave behind, download caches, and
/// (shown, never cleaned) conversation history.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Leftover {
    pub id: String,
    /// "agent", "cache" or "history".
    pub group: String,
    pub paths: Vec<String>,
    pub bytes: u64,
    pub selected: bool,
    /// "quitClaude", "quitCodex" or "keep" (history: Shed never cleans it).
    pub blocked: Option<String>,
}

fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|p| std::fs::symlink_metadata(p).is_ok()).collect()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// "2.1.309" < "2.1.317" < "2.10.0".
fn version_key(name: &str) -> Vec<u64> {
    name.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(0)).collect()
}

/// Every version folder but the newest and those a running program uses.
fn old_versions(dir: &Path, procs: &[String], keep: Option<&Path>) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = children(dir).into_iter().filter(|p| !version_key(&p.file_name().unwrap_or_default().to_string_lossy()).is_empty()).collect();
    all.sort_by_key(|p| version_key(&p.file_name().unwrap_or_default().to_string_lossy()));
    all.pop();
    all.retain(|p| {
        let ps = s(p);
        keep != Some(p.as_path()) && !procs.iter().any(|c| c.contains(&ps))
    });
    all
}

/// Candidate leftovers with their paths; sizes come later.
fn leftover_list(env: &Env, procs: &[String]) -> Vec<(&'static str, &'static str, Vec<PathBuf>)> {
    let h = &env.home;
    let claude = env.app_data.join("Claude");
    let local = &env.local;
    let mac = cfg!(target_os = "macos");
    let mut v: Vec<(&'static str, &'static str, Vec<PathBuf>)> = vec![];

    // The VM image ships compressed and is unpacked next to it; the archive
    // is only read again when the image is rebuilt (anthropics/claude-code#65577).
    let vm = claude.join("vm_bundles").join("claudevm.bundle");
    if vm.join("rootfs.img").is_file() {
        v.push(("claudeVmArchive", "agent", existing(vec![vm.join("rootfs.img.zst")])));
    }
    // The desktop app keeps every Claude Code it ever downloaded.
    v.push(("claudeCodeOld", "agent", old_versions(&claude.join("claude-code"), procs, None)));
    let cli = h.join(".local").join("share").join("claude").join("versions");
    let current = Some(canon(&h.join(".local").join("bin").join(if cfg!(windows) { "claude.exe" } else { "claude" })));
    v.push(("claudeCliOld", "agent", old_versions(&cli, procs, current.as_deref())));
    // Half-finished runtime downloads of the Codex app.
    let rt = children(&h.join(".cache").join("codex-runtimes"));
    let hour_ago = now().saturating_sub(3600);
    let stale = |p: &PathBuf| std::fs::metadata(p).is_ok_and(|m| mtime(&m) < hour_ago);
    v.push(("codexInstallTemp", "agent", rt.into_iter().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("codex-runtime-install-"))).filter(stale).collect()));
    if mac {
        v.push(("xcodeDerived", "agent", existing(vec![h.join("Library/Developer/Xcode/DerivedData")])));
    }

    let caches: Vec<(&'static str, Vec<PathBuf>)> = vec![
        ("npm", vec![if cfg!(windows) { local.join("npm-cache").join("_cacache") } else { h.join(".npm").join("_cacache") }]),
        ("pnpm", vec![if mac { h.join("Library/pnpm/store") } else { local.join("pnpm").join("store") }]),
        ("yarn", vec![if mac { local.join("Yarn") } else { local.join("Yarn").join("Cache") }]),
        ("cargo", vec![h.join(".cargo/registry/cache"), h.join(".cargo/registry/src"), h.join(".cargo/git/checkouts")]),
        ("pip", vec![if mac { local.join("pip") } else { local.join("pip").join("Cache") }]),
        ("uv", vec![if cfg!(windows) { local.join("uv").join("cache") } else { h.join(".cache").join("uv") }]),
        ("gradle", vec![h.join(".gradle").join("caches")]),
        ("playwright", vec![local.join("ms-playwright")]),
        ("goBuild", vec![local.join("go-build")]),
        ("homebrew", if mac { vec![local.join("Homebrew")] } else { vec![] }),
        ("cocoapods", if mac { vec![local.join("CocoaPods")] } else { vec![] }),
    ];
    v.extend(caches.into_iter().map(|(id, p)| (id, "cache", existing(p))));

    v.push(("codexSessions", "history", existing(vec![h.join(".codex/sessions"), h.join(".codex/archived_sessions")])));
    v.push(("claudeSessions", "history", existing(vec![h.join(".claude/projects")])));
    v.push(("codexScratch", "history", existing(vec![env.documents.join("Codex")])));
    v.retain(|(_, _, p)| !p.is_empty());
    v
}

/// The leftovers as they stand now, without sizes.
fn assess(env: &Env) -> Vec<Leftover> {
    let procs = processes();
    let run = running(&procs);
    leftover_list(env, &procs)
        .into_iter()
        .map(|(id, group, paths)| {
            let blocked = match id {
                _ if group == "history" => Some("keep"),
                "claudeVmArchive" | "claudeCodeOld" if run.claude => Some("quitClaude"),
                "codexInstallTemp" if run.codex => Some("quitCodex"),
                _ => None,
            };
            let selected = group == "agent" && blocked.is_none() && id != "claudeVmArchive";
            Leftover { id: id.into(), group: group.into(), paths: paths.iter().map(|p| s(p)).collect(), selected, blocked: blocked.map(Into::into), bytes: 0 }
        })
        .collect()
}

pub fn leftovers(env: &Env) -> Vec<Leftover> {
    let mut out = assess(env);
    par_each(&mut out, |l| l.bytes = l.paths.iter().map(|p| leftover_size(Path::new(p))).sum());
    out.retain(|l| l.bytes > 0);
    out.sort_by(|a, b| a.group.cmp(&b.group).then(b.bytes.cmp(&a.bytes)));
    out
}

fn leftover_size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_file() => disk_bytes(&m, &Mutex::new(HashSet::new())),
        Ok(_) => size_of(p),
        Err(_) => 0,
    }
}

// ------------------------------------------------------------------ cleaning

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Trashed {
    pub path: String,
    /// Where macOS put it in the Trash (Windows finds it by its old path).
    pub trash: Option<String>,
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Failed {
    pub path: String,
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Record {
    /// Milliseconds; also the record's id.
    pub time: u64,
    /// The folder that was scanned, or "" for leftovers.
    pub root: String,
    pub done: Vec<Trashed>,
    pub failed: Vec<Failed>,
    pub undone: bool,
}

impl Record {
    pub fn bytes(&self) -> u64 {
        self.done.iter().map(|t| t.bytes).sum()
    }
}

/// Moves a folder to the Trash and says where it went.
pub type Trasher<'a> = &'a (dyn Fn(&Path) -> Result<Option<PathBuf>, String> + Sync);
pub type Progress<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// What may be cleaned right now, checked again at the moment of cleaning:
/// a folder Shed recognises, not a link, nothing in it tracked by git.
pub fn still_cleanable(path: &Path) -> Result<(), String> {
    if !is_dir(path) {
        return Err("gone".into());
    }
    if classify(path).is_none() {
        return Err("unknown".into());
    }
    if git_state(path).0 {
        return Err("tracked".into());
    }
    Ok(())
}

pub fn clean_items(env: &Env, root: &str, items: &[Item], trash: Trasher, progress: Progress) -> Record {
    let paths: Vec<(PathBuf, u64)> = items
        .iter()
        .flat_map(|i| {
            // ponytail: merged __pycache__ items split their size evenly; it only feeds the history's totals.
            let each = i.bytes / i.paths.len().max(1) as u64;
            i.paths.iter().map(move |p| (PathBuf::from(p), each))
        })
        .collect();
    run(env, root, paths, &|p| still_cleanable(p), trash, progress)
}

pub fn clean_leftovers(env: &Env, ids: &[String], trash: Trasher, progress: Progress) -> Record {
    let fresh = assess(env);
    let paths: Vec<(PathBuf, u64)> = fresh
        .iter()
        .filter(|l| ids.contains(&l.id) && l.blocked.is_none())
        .flat_map(|l| l.paths.iter().map(|p| (PathBuf::from(p), leftover_size(Path::new(p)))))
        .collect();
    // The list was just built from fixed locations, so it is its own check.
    run(env, "", paths, &|p| if std::fs::symlink_metadata(p).is_ok() { Ok(()) } else { Err("gone".into()) }, trash, progress)
}

type Check<'a> = &'a dyn Fn(&Path) -> Result<(), String>;

fn run(env: &Env, root: &str, paths: Vec<(PathBuf, u64)>, check: Check, trash: Trasher, progress: Progress) -> Record {
    let mut rec = Record { time: now_ms(), root: root.into(), ..Default::default() };
    let total = paths.len();
    for (i, (p, bytes)) in paths.into_iter().enumerate() {
        progress(i, total);
        match check(&p).and_then(|_| trash(&p)) {
            Ok(to) => rec.done.push(Trashed { path: s(&p), trash: to.map(|t| s(&t)), bytes }),
            Err(error) => rec.failed.push(Failed { path: s(&p), error }),
        }
    }
    progress(total, total);
    if !rec.done.is_empty() {
        save_record(env, &rec);
    }
    rec
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ------------------------------------------------------------------ history

const KEEP_RECORDS: usize = 200;

fn history_file(env: &Env) -> PathBuf {
    env.data.join("history.json")
}

/// Newest first.
pub fn history(env: &Env) -> Vec<Record> {
    std::fs::read(history_file(env)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write_history(env: &Env, h: &[Record]) {
    let _ = std::fs::create_dir_all(&env.data);
    let tmp = env.data.join("history.json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec_pretty(h).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, history_file(env));
    }
}

fn save_record(env: &Env, rec: &Record) {
    let mut h = history(env);
    h.insert(0, rec.clone());
    h.truncate(KEEP_RECORDS);
    write_history(env, &h);
}

/// Puts a clean back. `restore` brings one folder back from the Trash.
pub fn undo(env: &Env, time: u64, restore: &dyn Fn(&Trashed) -> Result<(), String>) -> Result<Record, String> {
    let mut h = history(env);
    let rec = h.iter_mut().find(|r| r.time == time).ok_or("not found")?;
    if rec.undone {
        return Err("already undone".into());
    }
    let mut back = Record { time: rec.time, root: rec.root.clone(), ..Default::default() };
    for t in &rec.done {
        if Path::new(&t.path).exists() {
            back.failed.push(Failed { path: t.path.clone(), error: "occupied".into() });
            continue;
        }
        match restore(t) {
            Ok(()) => back.done.push(t.clone()),
            Err(error) => back.failed.push(Failed { path: t.path.clone(), error }),
        }
    }
    rec.undone = true;
    write_history(env, &h);
    Ok(back)
}

/// Restores by moving the folder back from where the Trash put it.
pub fn restore_moved(t: &Trashed) -> Result<(), String> {
    let from = t.trash.as_deref().ok_or("emptied")?;
    if !Path::new(from).exists() {
        return Err("emptied".into());
    }
    std::fs::rename(from, &t.path).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tmp(name: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("shed-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(canon(&p))
    }
    fn file(p: &Path, bytes: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![b'x'; bytes]).unwrap();
    }
    fn env(home: &Path) -> Env {
        Env { home: home.into(), app_data: home.join("AppData"), local: home.join("Cache"), documents: home.join("Documents"), data: home.join("shed-data") }
    }
    fn names(scan: &Scan, root: &Path) -> Vec<String> {
        let mut v: Vec<String> = scan.items.iter().map(|i| i.paths[0].strip_prefix(&s(root)).unwrap().trim_start_matches(['/', '\\']).replace('\\', "/")).collect();
        v.sort();
        v
    }
    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git").arg("-C").arg(dir).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn finds_nested_projects_and_skips_what_is_not_build_output() {
        let t = tmp("nested");
        let r = t.0.join("Projects");
        // A Tauri app: package.json at the root, Cargo.toml one level down.
        file(&r.join("app/package.json"), 2);
        file(&r.join("app/node_modules/react/index.js"), 4096);
        file(&r.join("app/node_modules/react/package.json"), 2);
        file(&r.join("app/node_modules/react/dist/x.js"), 10); // never searched
        file(&r.join("app/src-tauri/Cargo.toml"), 2);
        file(&r.join("app/src-tauri/target/debug/app"), 8192);
        file(&r.join("app/src/target/notes.md"), 10); // no Cargo.toml next to it
        // Python with a virtualenv named anything, and caches in subfolders.
        file(&r.join("py/requirements.txt"), 2);
        file(&r.join("py/env/pyvenv.cfg"), 2);
        file(&r.join("py/pkg/__pycache__/a.pyc"), 100);
        file(&r.join("py/pkg/sub/__pycache__/b.pyc"), 100);
        file(&r.join("py/pkg/sub/mod.py"), 1); // a folder of scripts is still part of py
        // Plain folders called build or target without a project are left alone.
        file(&r.join("photos/build/a.jpg"), 10);
        // Script folders without a project file around them.
        file(&r.join("tools/a/x.py"), 1);
        file(&r.join("tools/a/__pycache__/x.pyc"), 10);
        file(&r.join("tools/b/y.py"), 1);
        file(&r.join("tools/b/__pycache__/y.pyc"), 10);
        // An agent worktree has its own node_modules.
        file(&r.join("app/.claude/worktrees/feat/package.json"), 2);
        file(&r.join("app/.claude/worktrees/feat/node_modules/x/i.js"), 10);
        // A link is never followed or cleaned.
        #[cfg(unix)]
        std::os::unix::fs::symlink(r.join("app/node_modules"), r.join("py/node_modules")).unwrap();

        let scan = scan(&env(&t.0), &r);
        assert_eq!(scan.refused, None);
        assert_eq!(names(&scan, &r), ["app/.claude/worktrees/feat/node_modules", "app/node_modules", "app/src-tauri/target", "py/env", "py/pkg/__pycache__", "tools/a/__pycache__"]);
        let pyc = scan.items.iter().find(|i| i.name == "__pycache__").unwrap();
        assert_eq!(pyc.paths.len(), 2, "one item for all of a project's caches");
        assert_eq!(pyc.project, s(&r.join("py")));
        let tools = scan.items.iter().find(|i| i.project.ends_with("tools")).unwrap();
        assert_eq!(tools.paths.len(), 2, "a folder of scripts is one project too");
        let wt = scan.items.iter().find(|i| i.paths[0].contains("worktrees")).unwrap();
        assert!(wt.notes.contains(&"worktree".to_string()));
        let target = scan.items.iter().find(|i| i.name == "target").unwrap();
        assert_eq!((target.kind.as_str(), target.project.as_str()), ("Rust", s(&r.join("app")).as_str()), "shown with the app around it");
        assert!(target.bytes >= 8192 && target.selected);
        assert!(scan.items.iter().find(|i| i.kind == "venv").unwrap().selected, "has requirements.txt");

        // Picking the app itself lists everything under it.
        let one = super::scan(&env(&t.0), &r.join("app"));
        assert!(one.items.iter().all(|i| i.project == s(&r.join("app"))));
    }

    #[test]
    fn a_folder_picked_directly_can_itself_be_build_output() {
        let t = tmp("direct");
        file(&t.0.join("p/Cargo.toml"), 2);
        file(&t.0.join("p/target/x"), 10);
        file(&t.0.join("q/android/build/x"), 10);
        file(&t.0.join("q/package.json"), 2);
        let e = env(&t.0);
        assert_eq!(names(&scan(&e, &t.0.join("p/target")), &t.0), ["p/target"]);
        assert_eq!(names(&scan(&e, &t.0.join("q/android/build")), &t.0), ["q/android/build"]);
        assert_eq!(classify(&t.0.join("q/android/build")).unwrap().1, "android/build");
        assert!(classify(&t.0.join("q/android")).is_none());
    }

    #[test]
    fn git_decides_about_folders_that_might_be_real_work() {
        let t = tmp("git");
        let p = t.0.join("site");
        file(&p.join("package.json"), 2);
        file(&p.join("dist/index.html"), 10); // ignored: build output
        file(&p.join("build/keep.txt"), 10); // tracked: someone's work
        file(&p.join("out/a.txt"), 10); // neither: ask
        file(&p.join("node_modules/a/i.js"), 10);
        file(&p.join(".gitignore"), 0);
        fs::write(p.join(".gitignore"), "dist/\nnode_modules/\n").unwrap();
        git(&p, &["init", "-q"]);
        git(&p, &["add", "build/keep.txt", ".gitignore", "package.json"]);

        let scan = scan(&env(&t.0), &p);
        let get = |n: &str| scan.items.iter().find(|i| i.name == n).unwrap().clone();
        assert!(get("dist").selected && get("dist").notes.contains(&"ignored".into()));
        assert_eq!(get("build").blocked.as_deref(), Some("tracked"));
        assert!(!get("build").selected);
        assert!(!get("out").selected && get("out").notes.contains(&"maybe".into()));
        assert!(get("node_modules").selected);
        assert_eq!(still_cleanable(&p.join("build")), Err("tracked".into()));
        assert_eq!(still_cleanable(&p.join("dist")), Ok(()));
        assert_eq!(still_cleanable(&p.join("src")), Err("gone".into()));
    }

    #[test]
    fn venv_without_a_dependency_list_is_not_ticked() {
        let t = tmp("venv");
        file(&t.0.join("p/main.py"), 2);
        file(&t.0.join("p/.venv/pyvenv.cfg"), 2);
        let scan = scan(&env(&t.0), &t.0);
        assert_eq!(scan.items.len(), 1);
        assert!(!scan.items[0].selected && scan.items[0].notes.contains(&"noManifest".into()));
    }

    #[test]
    fn refuses_system_folders_and_skips_tool_installs_at_home() {
        let t = tmp("home");
        let e = env(&t.0);
        file(&t.0.join(".vscode/extensions/x/package.json"), 2);
        file(&t.0.join(".vscode/extensions/x/node_modules/y/i.js"), 10);
        file(&t.0.join("Library/App/package.json"), 2);
        file(&t.0.join("Library/App/node_modules/y/i.js"), 10);
        file(&t.0.join("code/w/package.json"), 2);
        file(&t.0.join("code/w/node_modules/y/i.js"), 10);
        assert_eq!(names(&scan(&e, &t.0), &t.0), ["code/w/node_modules"]);
        assert_eq!(scan(&e, &t.0.join("Library")).refused.as_deref(), Some("library"));
        assert_eq!(scan(&e, t.0.parent().unwrap()).refused.as_deref(), Some("system"));
        assert_eq!(scan(&e, &t.0.join("nope")).refused.as_deref(), Some("missing"));
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_count_once() {
        let t = tmp("links");
        file(&t.0.join("a/big"), 64 * 1024);
        fs::hard_link(t.0.join("a/big"), t.0.join("a/again")).unwrap();
        let one = size_of(&t.0.join("a"));
        assert!((64 * 1024..80 * 1024).contains(&one), "{one}");
    }

    #[test]
    fn busy_means_a_program_other_than_a_shell_runs_inside() {
        let c = |n: &str, d: &str| (n.to_string(), d.to_string());
        let cwds = [c("zsh", "/p"), c("node", "/p/web"), c("node", "/pp"), c("claude", "/q"), c("vite", "/q/site")];
        assert_eq!(busy_in(&cwds, "/p"), ["node"]);
        assert!(busy_in(&cwds, "/x").is_empty());
        assert_eq!(busy_in(&cwds, "/q"), ["vite"], "the agent itself is not in the way");
    }

    #[test]
    fn program_names_from_ps_and_tasklist() {
        if cfg!(windows) {
            assert_eq!(program("Claude.exe\",\"1234\",\"Console\""), "Claude");
        } else {
            assert_eq!(program("/Applications/Claude.app/Contents/MacOS/Claude"), "Claude");
            assert_eq!(program("/usr/local/bin/codex --foo"), "codex");
            assert_eq!(program("/Applications/Claude.app/Contents/Helpers/disclaimer --pgroup -- /x"), "Claude");
            let cli = ["/Users/a/.local/bin/claude --resume", "/Users/a/Library/Application Support/Claude/claude-code/2.1.3/x/claude.app/Contents/MacOS/claude -p"];
            assert!(!running(&cli.map(String::from)).claude, "Claude Code is not the desktop app");
        }
    }

    #[test]
    fn old_versions_keep_the_newest_and_those_in_use() {
        let t = tmp("versions");
        for v in ["2.1.9", "2.1.10", "2.1.309", "2.1.317"] {
            file(&t.0.join(v).join("bin"), 1);
        }
        let procs = vec![format!("{}/claude --x", s(&t.0.join("2.1.10")))];
        let old: Vec<String> = old_versions(&t.0, &procs, Some(&t.0.join("2.1.9"))).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(old, ["2.1.309"]);
    }

    #[test]
    fn leftovers_never_offer_history_and_wait_for_running_apps() {
        let t = tmp("leftovers");
        let e = env(&t.0);
        file(&t.0.join(".codex/sessions/2026/a.jsonl"), 5000);
        file(&t.0.join(".claude/projects/x/a.jsonl"), 5000);
        let vm = e.app_data.join("Claude/vm_bundles/claudevm.bundle");
        file(&vm.join("rootfs.img"), 10);
        file(&vm.join("rootfs.img.zst"), 5000);
        file(&e.app_data.join("Claude/claude-code/2.1.1/x"), 5000);
        file(&e.app_data.join("Claude/claude-code/2.1.2/x"), 5000);
        let l = leftovers(&e);
        let get = |id: &str| l.iter().find(|x| x.id == id).cloned();
        assert_eq!(get("codexSessions").unwrap().blocked.as_deref(), Some("keep"));
        assert_eq!(get("claudeSessions").unwrap().blocked.as_deref(), Some("keep"));
        assert!(!get("claudeVmArchive").unwrap().selected, "costs a download later, so opt-in");
        let old = get("claudeCodeOld").unwrap();
        assert_eq!(old.paths, [s(&e.app_data.join("Claude/claude-code/2.1.1"))]);

        // Cleaning by id never touches history, whatever the UI asks for.
        let moved = Mutex::new(vec![]);
        let fake = |p: &Path| {
            moved.lock().unwrap().push(s(p));
            Ok(None)
        };
        let ids: Vec<String> = ["codexSessions", "claudeSessions", "claudeCodeOld"].map(String::from).to_vec();
        let rec = clean_leftovers(&e, &ids, &fake, &|_, _| {});
        if running(&processes()).claude {
            assert!(rec.done.is_empty());
        } else {
            assert_eq!(*moved.lock().unwrap(), [s(&e.app_data.join("Claude/claude-code/2.1.1"))]);
        }
    }

    #[test]
    fn clean_then_undo_puts_everything_back() {
        let t = tmp("undo");
        let e = env(&t.0);
        let p = t.0.join("w");
        file(&p.join("Cargo.toml"), 2);
        file(&p.join("target/debug/bin"), 100);
        file(&p.join("package.json"), 2);
        file(&p.join("node_modules/a/i.js"), 100);
        let bin = t.0.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let fake = |from: &Path| {
            let to = bin.join(format!("{}-{}", from.file_name().unwrap().to_string_lossy(), now_ms()));
            fs::rename(from, &to).map(|_| Some(to)).map_err(|e| e.to_string())
        };
        let mut items = scan(&e, &p).items;
        // A folder that vanished since the scan is reported, not invented.
        items.push(Item { name: "target".into(), paths: vec![s(&p.join("gone/target"))], ..Default::default() });
        let steps = Mutex::new(vec![]);
        let rec = clean_items(&e, &s(&p), &items, &fake, &|d, n| steps.lock().unwrap().push((d, n)));
        assert_eq!(rec.done.len(), 2);
        assert_eq!(rec.failed.len(), 1);
        assert_eq!(steps.lock().unwrap().last(), Some(&(3, 3)));
        assert!(!p.join("target").exists() && !p.join("node_modules").exists());
        assert_eq!(history(&e)[0].time, rec.time);

        let back = undo(&e, rec.time, &restore_moved).unwrap();
        assert_eq!(back.done.len(), 2);
        assert!(p.join("target/debug/bin").is_file() && p.join("node_modules/a/i.js").is_file());
        assert!(history(&e)[0].undone);
        assert!(undo(&e, rec.time, &restore_moved).is_err(), "once only");
    }

    #[test]
    fn undo_never_overwrites_a_rebuilt_folder() {
        let t = tmp("occupied");
        let e = env(&t.0);
        let p = t.0.join("w");
        file(&p.join("package.json"), 2);
        file(&p.join("node_modules/a/i.js"), 100);
        let bin = t.0.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let fake = |from: &Path| {
            let to = bin.join("nm");
            fs::rename(from, &to).map(|_| Some(to)).map_err(|e| e.to_string())
        };
        let rec = clean_items(&e, "", &scan(&e, &p).items, &fake, &|_, _| {});
        file(&p.join("node_modules/fresh/i.js"), 1); // npm install ran again
        let back = undo(&e, rec.time, &restore_moved).unwrap();
        assert_eq!(back.failed[0].error, "occupied");
        assert!(p.join("node_modules/fresh/i.js").exists() && bin.join("nm/a/i.js").exists());
    }
}
