# SYSTM Cut plugins (Workspace > Scripts)

A plugin is a script the editor lists under **Workspace > Scripts** and runs as its own process. It drives the app through the control channel: everything a person or an agent can do.

## Where plugins live
- `~/Library/Application Support/SYSTM Cut/Scripts/<Category>/` (yours)
- `/Library/Application Support/SYSTM Cut/Scripts/<Category>/` (the whole Mac; where a `.pkg` installs)

## Two forms
1. **A single file**: `My_Script.py`, `.sh`, `.js`, `.lua` or any executable. The file name is the menu name.
2. **A folder with `plugin.json`**:
   ```json
   { "name": "Page Tour", "category": "Utility", "description": "…", "command": ["python3", "tour.py"] }
   ```

## What a plugin gets
Environment: `SYSTM_CUT_PORT`, `SYSTM_CUT_TOKEN_FILE`, `SYSTM_CUT_CLI`. The last line it prints is shown in the status bar.
Python: copy `sdk/systm_cut.py` beside your script (see `examples/page-tour`).

## Shipping one
`plugins/make-plugin-pkg.sh <plugin folder> [version]` builds a macOS installer in `dist/plugins/`.
Agents can list and run plugins too: control methods `systm.scripts` and `systm.script.run {name}`.
