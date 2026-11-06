# Contributing

Thanks for helping. A few things keep Shed small and trustworthy.

## Setup

You need [Rust](https://rustup.rs), Node.js 18+, and on macOS the Xcode Command Line Tools.

```bash
npm install
npm run dev                 # run the app
cd src-tauri && cargo test  # run the tests
```

The UI is plain HTML, CSS and JS in `ui/` with no build step. Open `ui/index.html` in any browser to work on it: `ui/preview-mock.js` fakes the backend with sample data when the page runs outside the app.

## Where things live

| File | What it does |
|---|---|
| `src-tauri/src/core.rs` | The project rules, scanning, AI leftovers, cleaning, history, undo, and the tests |
| `src-tauri/src/trash.rs` | The system Trash (macOS) and Recycle Bin (Windows) |
| `src-tauri/src/menu.rs` | The right-click entry: a Finder service, an Explorer verb |
| `src-tauri/src/app.rs` | Settings and the commands the UI calls |
| `src-tauri/src/main.rs` | The `shed` command line |

## Adding a project type

Add a `Rule` to `RULES` in `core.rs`: the file that proves the project (`Cargo.toml`, `*.csproj`) and the folders its tools rebuild. If a folder name could also be someone's real work, add it to `MAYBE` so it is only ticked when git ignores it. Add a case to the tests.

## Adding an AI leftover

Add it to `leftover_list` in `core.rs` only if you can say exactly what it is, that the tool recreates it or never needs it again, and what cleaning it costs. Write that in the UI text (`L_<id>` in `ui/app.js`). If the tool keeps it open while running, block it in `assess` until the tool is closed. Conversation history is never a leftover.

## Ground rules

- **No network code.** Not for updates, analytics or crash reports.
- **Never delete.** Everything goes to the Trash and every clean can be undone.
- **Prefer less code.** Use the standard library and the OS before adding a dependency. A new dependency needs a reason in the pull request.
- Keep pull requests focused, and run `cargo test` before opening one.
