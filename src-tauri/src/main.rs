// Hide the console window on Windows release builds.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod core;
mod menu;
mod trash;

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan" | "clean" | "leftovers" | "undo" | "help" | "--help" | "-h") => std::process::exit(cli(&args)),
        _ => app::run(),
    }
}

const HELP: &str = "Shed: clean the build output and AI-tool leftovers you never see.

  shed scan <folder>             list what can be cleaned in a folder
  shed clean <folder> --apply    move the ticked items to the Trash
  shed leftovers                 leftovers of AI coding tools, caches, history sizes
  shed undo                      put the last clean back

Nothing moves without --apply, and everything goes to the Trash.
Run with no arguments to open the app.";

fn cli(args: &[String]) -> i32 {
    let Some(env) = core::Env::real() else {
        eprintln!("cannot find the home folder");
        return 1;
    };
    let folder = || args.get(1).filter(|a| !a.starts_with("--")).map(Path::new);
    match args[0].as_str() {
        "scan" | "clean" => {
            let Some(root) = folder() else {
                eprintln!("{HELP}");
                return 2;
            };
            let scan = core::scan(&env, root);
            if let Some(why) = scan.refused {
                eprintln!("not scanned ({why}): {}", scan.root);
                return 1;
            }
            for i in &scan.items {
                let mark = if i.blocked.is_some() { "x" } else if i.selected { "*" } else { " " };
                let mut notes = i.blocked.iter().chain(&i.notes).cloned().collect::<Vec<_>>().join(", ");
                if !i.busy.is_empty() {
                    notes += &format!(" ({})", i.busy.join(", "));
                }
                let more = if i.paths.len() > 1 { format!(" (+{})", i.paths.len() - 1) } else { String::new() };
                println!("[{mark}] {:>9}  {:<10} {}{more}  {notes}", human(i.bytes), i.kind, i.paths[0]);
            }
            let ticked: Vec<core::Item> = scan.items.iter().filter(|i| i.selected).cloned().collect();
            let total: u64 = ticked.iter().map(|i| i.bytes).sum();
            if args[0] == "clean" && args.iter().any(|a| a == "--apply") {
                let rec = core::clean_items(&env, &scan.root, &ticked, &trash::to_trash, &|_, _| {});
                rec.failed.iter().for_each(|f| eprintln!("not moved ({}): {}", f.error, f.path));
                println!("moved {} folders to the Trash, {} (empty the Trash to free the space)", rec.done.len(), human(rec.bytes()));
                i32::from(!rec.failed.is_empty())
            } else {
                println!("{} ticked [*], {}. Nothing changed: run `shed clean <folder> --apply` to move them to the Trash.", ticked.len(), human(total));
                0
            }
        }
        "leftovers" => {
            for l in core::leftovers(&env) {
                let state = l.blocked.as_deref().unwrap_or(if l.selected { "ticked" } else { "" });
                println!("{:<8} {:>9}  {:<18} {:<11} {}", l.group, human(l.bytes), l.id, state, l.paths.join(", "));
            }
            0
        }
        "undo" => match core::history(&env).into_iter().find(|r| !r.undone) {
            Some(r) => match core::undo(&env, r.time, &trash::restore) {
                Ok(back) => {
                    back.failed.iter().for_each(|f| eprintln!("not put back ({}): {}", f.error, f.path));
                    println!("put back {} folders", back.done.len());
                    i32::from(!back.failed.is_empty())
                }
                Err(e) => {
                    eprintln!("not undone: {e}");
                    1
                }
            },
            None => {
                eprintln!("nothing to undo");
                1
            }
        },
        _ => {
            println!("{HELP}");
            0
        }
    }
}

fn human(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}
