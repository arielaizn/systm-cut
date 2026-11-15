# Make this your own: instructions for Claude

You are Claude (Claude Code or another coding agent), and someone has just cloned this repository because they want **their own edition** of this editor: their name on it, their brand's colours and fonts, their own team of agents, built for their machine. Your job is to interview them, apply their answers to the source, build it and hand them an installer. Do the work; do not hand them a to-do list.

Read `README.md` and `ARCHITECTURE.md` first. Then run the interview.

## 1. The interview

Ask in the user's language. Ask one group at a time (use your question tool if you have one), offer the default in brackets, and accept "default" for anything. Do not ask what you can find out yourself (their OS, whether Blender or ffmpeg is installed, which agent CLIs are on PATH: check).

1. **Name.** What is the app called? [SYSTM Cut] What should the window title and the installer say?
2. **Brand colours.** The background colour (dark) [#080c0e], the one accent colour [#acf4f8], and the ink (text) colour [#f5f7f7]. If they give a website or a logo, read the colours from it and confirm.
3. **Fonts.** A display font for the brand's own words (a .ttf or .otf file they have the right to ship) [none: Inter Semibold is used], and whether the interface language is left-to-right or right-to-left.
4. **Icon.** A 1024 px PNG for the app icon [generate a simple monogram from the name and the accent colour].
5. **Pages.** Which pages they want on the bar: MEDIA, CUT, EDIT, FUSION, PHOTO, 3D, COLOR, FAIRLIGHT, DEPTH, DELIVER [all but DEPTH: it needs the SYSTM Depth engine, which is not in this repository].
6. **The agents.**
   - Which leads they have installed and want: Claude Code (`claude`), Codex (`codex`) [whichever is on PATH].
   - The language the team answers in [the language of each request].
   - The video crew [Planner, Cutter, Captions, Motion, VFX, Colorist, Sound], the Photo team [Art Director, Brand Guardian, Typographer, Layout, Colour, Grader, Retoucher, Image Maker, Copy] and the 3D team [Scene Director, Modeler, Materials, Lighting, Animator, Camera, Render]: keep, rename, remove or add specialists. For a new one ask: its name, what it is the expert of (one sentence), a colour.
   - Their brand rules for the Brand Guardian: a brand file or a few sentences (colours, fonts, what is never done).
   - The one-tap requests (chips) they would use most on the Edit page, in their own words [the defaults].
7. **Voice.** Do they want the microphone in the chat? It needs an ElevenLabs or a Gemini API key in `~/.env.local` (`ELEVENLABS_API_KEY`, `GEMINI_API_KEY`). Never ask them to paste a key into the chat: tell them the file and the variable name.
8. **Sound library.** A folder of their own sound effects and music with an `index.json` [none: the Sound Library panel stays empty].
9. **Signing.** macOS: an Apple Developer ID to sign and notarise with [no: ad-hoc signing, the app opens with right-click > Open]. Windows: a code-signing certificate [no].

Repeat their answers back in a short list and get a yes before changing anything.

## 2. Where each answer goes

| Answer | Where |
|---|---|
| App name | `app/apps/systmcut/src/main.rs` (`with_title`), `app/apps/systmcut/src/native_menu.rs`, `app/crates/engine/src/demo.rs` (demo project name), `packaging/package.sh` and `packaging/windows/package.ps1` (bundle name, identifier), the page bar's wordmark in `app/crates/ui-egui/src/systm/shell.rs` (`page_bar`) |
| Colours | The accent and ink constants `ICE` and `INK` in `app/crates/ui-egui/src/systm/mod.rs`; the dark theme block in `app/crates/ui-egui/src/theme.rs` (`let dark = Tokens { … }`); the surface constants at the top of each file in `app/crates/ui-egui/src/systm/` (`BG`, `SURFACE`, `STRIP`, `WELL`, `LINE`, `INK`, `MUTED`, `FAINT`): derive them from the background (lighter steps of the same hue) so the whole app shifts together |
| Display font | Put the file at `app/assets/fonts/display.ttf` and set `DISPLAY_FONT` in `app/crates/ui-egui/src/theme.rs` (see the comment there); with no file the app falls back to Inter |
| Icon | `packaging/icon.png` (1024 px); the packaging scripts build the platform icons from it |
| Pages | `Page::ALL` in `app/crates/ui-egui/src/systm/shell.rs` (remove a page from the array to take it off the bar) |
| Leads and crews | `CREW` in `app/crates/ui-egui/src/systm/agents.rs`: one `Spec` per specialist (id, name, native-language name, colour, craft, team flags). `BRIEF`, `PHOTO_BRIEF`, `THREE_BRIEF` are what every agent of a team is told; put the brand rules into the Brand Guardian's `craft` and into `PHOTO_BRIEF` |
| Chips | `chips()` in the same file: `(label, request)` pairs per page |
| Rail texts | The Hebrew strings in `agents.rs` (`header`, `messages`, `composer`): translate them to the user's language |
| Sound library | `content_os()` in `app/crates/ui-egui/src/systm/mod.rs` and `systm/bank.rs` (the folder that holds `sound/index.json`) |

Search for the old value after each change (`grep -rn`), so nothing keeps the previous name or colour.

## 3. Build and hand over

1. Check the tools: `rustc --version` (1.85 or newer; install with rustup if missing), on macOS the Xcode command-line tools, on Windows the MSVC build tools.
2. Build: `cd app && cargo build --release -p systmcut -p systmcut-cli`. The first build takes 15 to 30 minutes. Fix what the compiler reports: your edits are the likely cause.
3. Package:
   - macOS: `packaging/package.sh --skip-build` makes `dist/<Name>-<version>.dmg`.
   - Windows: `powershell -File packaging/windows/package.ps1` makes `dist\<Name>-<version>-windows.zip`.
4. Run it and look: open the app, go through every page on the bar, send the team one request, and take a screenshot (`systmcut-cli app ui.screenshot '{"path":"shot.png"}'` while it runs). Check the screenshot yourself before you say it is done: the name, the colours, the roster.
5. Tell the user where the installer is, how to open it the first time (unsigned apps: right-click > Open on macOS, "More info > Run anyway" on Windows), and what they need installed for each part: Claude Code or Codex for the team, ffmpeg for the microphone, Blender for the 3D page, `agent-desktop` for the agents' own cursors on screen.

## Rules
- Their brand files, keys and personal paths never go into a commit unless they say so.
- Keep `LICENSE-*`, `NOTICE` and `ATTRIBUTION.md` in `app/` and `photo-engine/`: the engines are MIT / Apache-2.0 and require them.
- One change at a time when the build breaks: revert the last edit before trying another.
