# SYSTM Cut

A desktop video editor built like DaVinci Resolve (a page for each stage of the work) with a team of AI agents inside it. You talk to the team in a chat rail; they edit the same timeline you do, each with a cursor of its own, and every job ends with a card: keep it, or take all of it back.

> Personal project, early. It runs on macOS (Apple silicon). A Windows build is produced by CI and is not yet verified on a real machine. See **Status** for what is real and what is not.

## Make your own edition
Clone the repository, open it in Claude Code and say: **"make this mine"**. Claude follows [`PERSONALIZE.md`](PERSONALIZE.md): it asks for your name, colours, fonts, pages and agents, applies them to the source, builds the app and hands you an installer.

## The pages
| Page | What it is |
|---|---|
| MEDIA | bring footage in and organise it |
| CUT | fast assembly: the whole film as a strip, a working timeline with the playhead fixed in the middle, one-tap edit actions |
| EDIT | Media Pool, one viewer, the Inspector, the timeline with its tool strip |
| FUSION | effects on the current clip, with its node chain |
| PHOTO | a layered image editor (layers, masks, blend modes, adjustment layers, type, brushes, selections, PSD) that shares the film: a frame opens as a document, a document goes onto the timeline |
| 3D | Blender's own engine behind a page: viewport, outliner, properties, materials, keyframes, render to the timeline |
| COLOR | viewer, nodes, the clip strip, primaries, scopes |
| FAIRLIGHT | meters, timeline, mixer |
| DELIVER | export |

The Inspector and the Effects library are on every page and show what that page works with.

## The team
- **Leads:** Claude Code and Codex, run as local CLIs. They decide who does what.
- **Video crew:** Planner, Cutter, Captions, Motion, VFX, Colorist, Sound.
- **Photo team:** Art Director, Brand Guardian, Typographer, Layout, Colour, Grader, Retoucher, Image Maker, Copy.
- **3D team:** Scene Director, Modeler, Materials, Lighting, Animator, Camera, Render.

A lead hands work out; the specialists run at the same time, each as its own process with its own cursor and colour, and report back. The rail shows the team of the page you are on. It has conversation tabs, history with search, Ask (read only) and Act modes, attachments, a microphone, and a keep / undo card per job.

## From the outside
- **CLI:** `systmcut-cli page COLOR`, `systmcut-cli team ask "tighten the opening"`, `systmcut-cli photo type.create x=80 y=300 text=Hello size=160`, `systmcut-cli app <method> '<json>'`. It finds the running app by itself.
- **MCP:** `systmcut-cli mcp --live` is a local MCP server for the running app (timeline, pages, team, Photo, 3D, scripts).
- **Plugins:** scripts under *Workspace > Scripts*, as files or packaged installers ([`plugins/README.md`](plugins/README.md)).

## Build
```bash
cd app
cargo build --release -p systmcut -p systmcut-cli     # first build: 15-30 minutes
cd .. && packaging/package.sh --skip-build              # macOS: dist/SYSTM-Cut-<version>.dmg
```
Rust 1.85+. Optional at run time: Claude Code and/or Codex (the team), ffmpeg (the microphone), Blender (the 3D page), `agent-desktop` (agents operating the window with their own on-screen cursors).

## Status
Works and was exercised: the page shell; the Edit page (pool, viewer, Inspector, timeline: select, move, trim, blade); the team with dispatch to specialists and reports back; the Photo page (documents, type, layers) driven by commands; the CLI; plugins; the bridge to Blender.
Written, lightly tested: the Cut page actions, Photo's brush / selection / adjustments by hand, the microphone inside the app, the 3D page's interface.
Not there yet: Media, Fusion, Color, Fairlight and Deliver still use panels of the underlying engine inside the new shell; the Color nodes are read-only; no Windows verification; no code signing.

## What it stands on
- The video engine in `app/crates/` and the photo engine in `photo-engine/` come from the open-source Craft apps by the ArtCraft team (FilmCraft and PhotoCraft, MIT or Apache-2.0). Their licences and notices are kept in those folders. The interface, the pages, the agent team and everything in `app/crates/ui-egui/src/systm/` are this project's.
- Icons: Lucide (ISC).
- Blender is not part of this repository or of the app: the 3D page starts the Blender installed on your machine as a separate program. The small script that Blender runs (`blender_server.py`) is GPL-2.0-or-later, as Blender's API requires.

## Licence
MIT or Apache-2.0, at your option, for this project's own code; see the notices above for the parts that come from elsewhere.
