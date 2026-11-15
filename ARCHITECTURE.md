# How it is put together

```
app/                         the editor (Rust workspace)
  apps/systmcut              the desktop app (window, control server)
  apps/systmcut-cli          the CLI and MCP server
  crates/engine, project, …  the video engine: project model, commands, render, codecs
  crates/automation          MCP tools and the bridge to a running app
  crates/ui-egui             the interface
    src/systm/               everything SYSTM Cut adds; start here
photo-engine/                the photo engine (documents, layers, paint, type, PSD); a library
plugins/                     plugin SDK, an example, the installer builder
packaging/                   macOS and Windows packaging
research/resolve/            the analysis of DaVinci Resolve the pages were built from
```

## `app/crates/ui-egui/src/systm/`
| File | What it is |
|---|---|
| `shell.rs` | the window: top bar with per-page toggles, the page bar, the team rail; `Page` |
| `edit_page.rs`, `cut_page.rs` | those pages, drawn whole; the Media Pool and the viewer live in `edit_page.rs` |
| `timeline.rs` | the timeline (both pages use it) |
| `inspector.rs`, `fx_library.rs` | the Inspector and the Effects library, on every page |
| `photo_page.rs` | the Photo page over the photo engine's commands |
| `three_d.rs`, `blender_server.py` | the 3D page and the script Blender runs for it |
| `pages.rs` | Color page furniture: nodes, clip strip, gallery |
| `agents.rs` | the chat rail, the leads, the crews, dispatch, the microphone |
| `presence.rs` | the agents' cursors |
| `scripts.rs` | Workspace > Scripts (plugins) |
| `depth.rs`, `bank.rs`, `studio.rs` | SYSTM Depth, the sound library, companion tools |
| `ico.rs` | Lucide icon glyphs |

## Rules the code keeps
- **Everything is a command.** A click, an agent and a script all change the project through the same engine commands (`session.execute(id, params)`), so every edit is undoable and nobody disagrees about what an edit does.
- **One control channel.** The app listens on a local port with a per-launch token (`control.token`, `control.port` in the app's data folder). The CLI, MCP clients, plugins and the agents' bridges all use it. Methods added by this project start with `systm.`, `photo.` and `three.`.
- **Agents are processes.** A lead or a specialist is a CLI run per message with the app's MCP bridge; `SYSTM_AGENT_ID` tags its requests so its cursor can be drawn. A lead delegates by ending a message with a ```dispatch block; the app starts the specialists and feeds their reports back.
- **Heavy engines stay asleep.** Blender starts the first time the 3D page is opened; the photo engine holds nothing until a document exists.
