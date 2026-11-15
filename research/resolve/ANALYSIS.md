# DaVinci Resolve Studio 21: page-by-page analysis

Screenshots are not part of the repository.

## 1. What makes it Resolve and not Premiere

| Resolve | Premiere (what SYSTM Cut 0.4 still does) |
|---|---|
| **Pages.** Seven fixed rooms, one per stage of the work. The layout of a room is designed, not arranged by the user. | Workspaces: the same panels, re-docked. |
| **No tab groups.** A region holds one thing. Its title is plain text in a thin strip. | "Project / Media / Effects / …" tab strips on every region. |
| **Toggles, not tabs.** The top bar ("UI toolbar") has an icon per optional panel: left side opens things on the left (Media Pool, Effects, Index, Sound Library), right side opens things on the right (Mixer, Metadata, Inspector). A panel is either open in its one place or closed. | Panels are tabs you click between, or float. |
| **One viewer by default** on Edit; the Inspector takes the right third. | Source and Program side by side, always. |
| **The page bar** is the bottom edge of the window: product name on the left, seven icons in the centre, Home and Settings on the right. The current page has a red line on top and a lit icon. | No page bar. |
| **Project name in the centre of the top bar**, with "Edited" in red beside it while there are unsaved changes. | Title in the window frame. |
| **Flat, dense, quiet.** One grey family (#28282e panels, #1e1e22 wells), 1 px separators, no rounded cards, almost no colour except the accent (red-orange) on the active thing, the playhead and enabled dots. Clips: video blue, audio green. | |

## 2. The journey (left to right on the page bar is the order of the work)

1. **Media**: bring footage in and organise it. Top-left is the disk browser, bottom is the Media Pool with the bin list on its left, top-centre is a viewer for whatever is selected, right is audio meters over Metadata. Nothing about the timeline is here.
2. **Cut**: assemble fast. Media Pool top-left, one viewer top-right. Below: **two timelines at once**, the whole film as a thin overview strip on top and a zoomed working section under it, with a fixed playhead in the middle. No Inspector by default.
3. **Edit**: refine. Media Pool left, one viewer centre, **Inspector** right (Video / Audio / Effects / Transition / Image / File as icon tabs, then collapsible sections with an enable dot, a keyframe diamond and a reset arrow on every row). A tool strip runs between the viewer and the timeline. The timeline shows the timecode large at the left of the ruler, track headers with the track's **name and clip count**.
4. **Fusion**: effects on the clip under the playhead. Two viewers on top, Inspector right, a strip of tool icons, and the **node graph** across the bottom: `MediaIn → … → MediaOut`.
5. **Color**: grade. Gallery (stills) left, viewer centre, **node graph** right. Under them the **clip strip**: every clip of the timeline as a numbered thumbnail (01, 02, …) with its codec; the selected one is outlined. Then a strip of palette icons, and the bottom third: **Primaries colour wheels** (Lift, Gamma, Gain, Offset, with numeric fields and a master wheel under each) on the left, **Scopes** (Parade) on the right.
6. **Fairlight**: sound. Track meters across the top with the bus meter, Control Room and **Loudness** (Short, Short Max, Range, Integrated) and a small picture of the film. A transport bar with a large timecode. Track list with a header per track (name, volume field, R / S / M) and the **Mixer** on the right (Input, Track FX, Order, Effects, Dynamics, EQ, fader).
7. **Deliver**: export. Render Settings left (preset, file name, location, Video / Audio / File tabs), viewer centre with IN / OUT / DURATION above it, the clip strip and a read-only timeline under it, **Render Queue** right with "Render All".

### What carries over between pages (this is the feeling of Resolve)
- **One timeline, one playhead.** Changing page never loses the place in the film: the viewer shows the same frame on every page.
- **The current clip.** The clip under the playhead on Edit is the clip Fusion opens and the clip Color selects in the strip.
- **The same top bar and page bar.** Only the toggle icons change.
- **Nothing floats.** There is no window management to do, which is why moving between pages is instant and never disorienting.

## 3. What SYSTM Cut takes, and where it goes further

Take exactly:
- seven pages plus ours (Depth) on a full-width page bar; one playhead and one current clip across all of them;
- fixed rooms with single-purpose regions and plain titles, no tab groups;
- a top bar with per-page toggles (left opens left panels, right opens right panels) and the project name in the centre with its saved state;
- one viewer plus Inspector on Edit; viewer, nodes, clip strip, wheels and scopes on Color; meters, loudness and mixer on Fairlight; settings, viewer and queue on Deliver.

Go further (ours):
- the **Team rail** (Archie's panel) is one more right-side toggle on every page, and the agents know the page;
- **agent cursors** move through the same rooms;
- the look: SYSTM Carbon and Ice instead of grey and red, the Ice line on the active page, soft depth between regions instead of flat grey.
