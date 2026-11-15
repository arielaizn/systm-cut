# For Claude, in this repository

- If the person here has just cloned the repository and wants their own edition of the app, follow `PERSONALIZE.md`: interview them, apply the answers, build, package, verify with a screenshot.
- The app's own code is in `app/crates/ui-egui/src/systm/` (one file per page or system; `mod.rs` lists them). The video engine (`app/crates/*`) and the photo engine (`photo-engine/crates/*`) are libraries: change them only when a page needs something they lack.
- Build: `cd app && cargo build --release -p systmcut -p systmcut-cli`. Package: `packaging/package.sh --skip-build` (macOS), `packaging/windows/package.ps1` (Windows).
- Drive the running app from a shell with `systmcut-cli` (`page`, `team`, `photo`, `scripts`, `app <method>`); it is also an MCP server (`systmcut-cli mcp --live`).
- Every page draws its own interface with egui's painter; nothing uses stock widgets for layout. New controls must register with `app.auto.add(id, rect, label)` and `widget_info`, so agents and assistive tools can find them.
