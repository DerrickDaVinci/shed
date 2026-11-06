# Security

Shed moves folders to the Trash, so anything that could make it take the wrong folder gets priority.

## Reporting a problem

Please **do not open a public issue** for a vulnerability. Use GitHub's private reporting instead:
**Security → Report a vulnerability** on this repository.

Helpful reports include the Shed version, the operating system, the folder you scanned (its layout is enough) and what was listed or moved that should not have been.

## What counts

- A folder offered or moved that is not rebuildable output: source code, something git tracks, someone's work.
- Anything outside the folder you picked being touched.
- Conversation history (Codex sessions, Claude Code transcripts) being offered or moved.
- Undo not restoring a folder that is still in the Trash, or overwriting one.
- Shed sending anything over the network. It must never do this.

## Design guarantees

- No network access, no telemetry, no update checks.
- Nothing moves without a confirmation (`--apply` on the command line).
- Folders go to the Trash or Recycle Bin, never deleted directly.
- Each folder is checked again right before it moves: a real folder (links are never followed), next to its project file, nothing in it tracked by git.
- Clean requests from the window refer to items of the last scan by position; the window cannot name arbitrary paths.
