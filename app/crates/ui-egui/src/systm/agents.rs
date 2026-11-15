//! The team chat: the agent rail on the right of the window.
//!
//! Modelled on Archie, the agent chat of the SYSTM CRM: conversations as tabs, a searchable
//! history, Ask (read only) and Act modes, attachments, copy / edit-and-resend / retry on messages,
//! formatted replies, a line for every step an agent takes, and a card for every job that says
//! what happened and lets the user keep it or take all of it back.
//!
//! Each agent is a headless CLI (`claude -p` or `codex exec`) started per message. It reaches the
//! editor through this app's MCP bridge (`systmcut-cli mcp --bridge`) and, with Hands on, through
//! `agent-desktop`: the real window, the accessibility tree, a cursor of its own.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use egui::epaint::text::{LayoutJob, TextDirection, TextFormat};
use egui::{Align, Color32, CornerRadius, RichText, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};

use super::shell::Page;
use super::ui_kit;
use crate::SystmCutApp;
use crate::theme::Tokens;

const INK_TEXT: Color32 = Color32::from_rgb(0xe6, 0xec, 0xee);
const DIM: Color32 = Color32::from_rgb(0x86, 0x97, 0xa0);
const FAINT: Color32 = Color32::from_rgb(0x56, 0x66, 0x6e);
const RAISED: Color32 = Color32::from_rgb(0x18, 0x24, 0x2a);
const EDGE: Color32 = Color32::from_rgb(0x24, 0x32, 0x39);
const WARN: Color32 = Color32::from_rgb(0xf0, 0xb8, 0x6a);
const OK: Color32 = Color32::from_rgb(0x7c, 0xd9, 0x92);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Claude,
    Codex,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Claude, Kind::Codex];
    pub fn id(self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Codex => "codex",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Claude => "Claude",
            Kind::Codex => "Codex",
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            Kind::Claude => Color32::from_rgb(0xe8, 0x8a, 0x5c),
            Kind::Codex => Color32::from_rgb(0x7c, 0xd9, 0x92),
        }
    }
    fn ix(self) -> usize {
        self as usize
    }
    fn from_id(s: &str) -> Option<Kind> {
        Self::ALL.into_iter().find(|k| k.id() == s)
    }
}

/// A specialist of the crew: a worker a lead (Claude or Codex) hands part of a job to. It runs as
/// its own process, with a cursor of its own colour and name, beside the others.
pub struct Spec {
    pub id: &'static str,
    pub name: &'static str,
    pub he: &'static str,
    pub color: Color32,
    /// What it is the expert of (told to the leads, and to itself).
    pub craft: &'static str,
    /// A specialist of the Photo page's team (design and brand) and not of the video crew.
    pub photo: bool,
    /// A specialist of the 3D page's team.
    pub three: bool,
}

impl Spec {
    /// 0 the video crew, 1 the Photo team, 2 the 3D team.
    fn team(&self) -> u8 {
        if self.three { 2 } else if self.photo { 1 } else { 0 }
    }
}

/// The team that works on a page.
fn team_of(page: Page) -> u8 {
    match page {
        Page::Photo => 1,
        Page::Three => 2,
        _ => 0,
    }
}

pub const CREW: &[Spec] = &[
    Spec { id: "planner", name: "Planner", he: "תוכנית עריכה", color: Color32::from_rgb(0xc9, 0xb6, 0xff), craft: "edit plans: reads the footage and the brief and writes the plan of the edit (structure, beats, what goes where and why) before anyone cuts. It does not change the timeline.", photo: false, three: false },
    Spec { id: "cutter", name: "Cutter", he: "קאטים וזומים", color: Color32::from_rgb(0xff, 0x8f, 0x6b), craft: "cuts and zooms: silences and dead air, tightening, pacing, jump cuts, punch-in zooms and reframes on the words that matter.", photo: false, three: false },
    Spec { id: "captions", name: "Captions", he: "כתוביות", color: Color32::from_rgb(0xff, 0xd4, 0x5c), craft: "captions: accurate word-timed subtitles in the speaker's language (Hebrew first), styled and animated so the key word of each line stands out.", photo: false, three: false },
    Spec { id: "motion", name: "Motion", he: "מושן גרפיקס", color: Color32::from_rgb(0x6b, 0xe4, 0xc9), craft: "motion graphics: titles, lower thirds, callouts, counters and animated graphics in the SYSTM style.", photo: false, three: false },
    Spec { id: "vfx", name: "VFX", he: "אפקטים", color: Color32::from_rgb(0xff, 0x6b, 0xb5), craft: "visual effects and compositing: keys, masks, tracking, transitions, speed ramps, stabilisation, SYSTM Depth shots.", photo: false, three: false },
    Spec { id: "color", name: "Colorist", he: "צבע", color: Color32::from_rgb(0x8f, 0xb8, 0xff), craft: "colour: balance and match the shots, then the grade (the SYSTM look unless told otherwise).", photo: false, three: false },
    Spec { id: "sound", name: "Sound", he: "סאונד", color: Color32::from_rgb(0xa6, 0xe8, 0x6b), craft: "sound: clean and level the voice, sound effects from the SYSTM sound bank on the cuts and events, music ducked under the voice, mix to about -14 LUFS.", photo: false, three: false },
    // ---- the Photo page's team: design and brand
    Spec { id: "director", name: "Art Director", he: "ארט דירקטור", color: Color32::from_rgb(0xff, 0xb0, 0x7a), craft: "the concept and the composition of a cover or a design: what the one idea is, what is big and what is small, where the eye goes first.", photo: true, three: false },
    Spec { id: "brand", name: "Brand Guardian", he: "שומר המותג", color: Color32::from_rgb(0xac, 0xf4, 0xf8), craft: "SYSTM's identity: Carbon #080c0e and Ice #acf4f8 as the only accent, the logo and its clear space, SYSTM Display V2, what is and is not on-brand. Reads ~/content-os/brand/brand.md, tokens.css and video-style.md first, and checks every piece before it is shown.", photo: true, three: false },
    Spec { id: "typographer", name: "Typographer", he: "טיפוגרפיה", color: Color32::from_rgb(0xe6, 0xd2, 0xff), craft: "type: SYSTM Display V2 for titles and Heebo for text, hierarchy, sizes, tracking, Hebrew and right-to-left setting, line breaks, legibility at thumbnail size.", photo: true, three: false },
    Spec { id: "layout", name: "Layout Designer", he: "לייאאוט", color: Color32::from_rgb(0x9a, 0xc8, 0xff), craft: "layout: grid, margins, alignment, balance, and the safe areas of each platform (Reels cover, YouTube thumbnail, carousel).", photo: true, three: false },
    Spec { id: "palette", name: "Colour Designer", he: "צבעים", color: Color32::from_rgb(0xff, 0x8f, 0xc8), craft: "colour in the design: palette, contrast, where the accent goes and where it must not, harmony with the brand's colours.", photo: true, three: false },
    Spec { id: "grader", name: "Colour Grader", he: "קולור גריידינג", color: Color32::from_rgb(0xff, 0xd4, 0x5c), craft: "the photograph itself: exposure, contrast, skin, and the cinematic grade the brand uses (a film frame, never bright stock), with adjustment layers.", photo: true, three: false },
    Spec { id: "retoucher", name: "Retoucher", he: "ריטוש", color: Color32::from_rgb(0x8f, 0xe8, 0xb0), craft: "retouching: cut-outs and masks, clean-up, skin, removing objects, sharpening.", photo: true, three: false },
    Spec { id: "imagemaker", name: "Image Maker", he: "יצירת תמונות", color: Color32::from_rgb(0xc0, 0xa0, 0xff), craft: "making or extending imagery when asked (backgrounds, textures, objects) in the brand's art direction, and placing it in the document.", photo: true, three: false },
    Spec { id: "copy", name: "Copy Editor", he: "קופי", color: Color32::from_rgb(0xf0, 0xe0, 0xb0), craft: "the words on the design: short Hebrew hooks with no clichés, spelling, and how the line breaks read.", photo: true, three: false },
    // ---- the 3D page's team
    Spec { id: "scene", name: "Scene Director", he: "במאי סצנה", color: Color32::from_rgb(0xff, 0xb0, 0x7a), craft: "the 3D scene as a whole: what is in it, the staging and the scale, and who builds what.", photo: false, three: true },
    Spec { id: "modeler", name: "Modeler", he: "מידול", color: Color32::from_rgb(0x9a, 0xc8, 0xff), craft: "modelling: meshes from primitives and modifiers (bevel, subdivision, boolean, array, solidify), clean topology, real-world scale, text and curves.", photo: false, three: true },
    Spec { id: "shader", name: "Materials", he: "חומרים", color: Color32::from_rgb(0xff, 0x8f, 0xc8), craft: "materials and shading: Principled BSDF, the SYSTM materials (matte off-white ceramic, frosted glass, brushed graphite, an Ice emission accent), textures and nodes.", photo: false, three: true },
    Spec { id: "lighter", name: "Lighting", he: "תאורה", color: Color32::from_rgb(0xff, 0xd4, 0x5c), craft: "lighting: key, fill and rim, area lights and HDRI, mood, the brand's dark stage with one Ice light.", photo: false, three: true },
    Spec { id: "animator", name: "Animator", he: "אנימציה", color: Color32::from_rgb(0x8f, 0xe8, 0xb0), craft: "animation: keyframes, easing, camera moves, timing to the edit's frame rate.", photo: false, three: true },
    Spec { id: "cameraman", name: "Camera", he: "מצלמה", color: Color32::from_rgb(0xc0, 0xa0, 0xff), craft: "the camera: lens, framing, depth of field, the aspect the film needs (9:16 or 16:9).", photo: false, three: true },
    Spec { id: "render", name: "Render TD", he: "רינדור", color: Color32::from_rgb(0xac, 0xf4, 0xf8), craft: "rendering: engine and samples, resolution, colour management, transparent film when the shot is composited, and getting the frames onto the timeline.", photo: false, three: true },
];

/// How the 3D page's agents work: Blender itself is behind the page.
const THREE_BRIEF: &str = "You work on the 3D page of SYSTM Cut. Behind it is a real Blender running in the background, and you drive it with Blender's own Python (bpy) through the `ui_control` MCP tool: method `three.scene` (every object with type, transform, colour, modifiers; the frame range; the camera), `three.exec` with params {\"code\": \"...python...\"} (runs with `bpy`, `C` = bpy.context, `D` = bpy.data, `Vector`, `math` in scope; set a variable `result` to get a value back; the answer includes the scene), `three.view` (renders an overview of the scene to a PNG and returns its path: READ that file to see your work; params yaw, pitch, dist, target are optional), `three.render` (renders the scene's own camera to a PNG; the scene needs a camera). Work in metres, at real scale. Look at your work with `three.view` before you say it is done. SYSTM's look in 3D: a dark Carbon stage, matte off-white ceramic and frosted glass, one Ice (#acf4f8) light or emission as the accent. Finish with one or two plain sentences in the user's language.";

/// How the Photo page's agents work: its engine is reached through the generic control tool.
const PHOTO_BRIEF: &str = "You work on the PHOTO page of SYSTM Cut: a layered image editor (Photoshop-class: layers, masks, blend modes, adjustment layers, type, brushes, selections, filters). Drive it with the `ui_control` MCP tool, which calls a method of the running app: method `photo.state` (the document and its layers), `photo.commands` with params {\"filter\": \"layer\"} (every command with its parameters; there are about 500, so filter), `photo.execute` with params {\"command\": id, \"params\": {...}} (run one; it is undoable), `photo.view` (writes the flattened picture to a PNG and returns its path: READ that file to see your work, and look before you say it is done), `photo.fromFrame` (the timeline's current frame becomes a document), `photo.toTimeline` (the document goes onto the timeline). New documents: `file.new` {width, height, background}. Text: `type.create` {x, y, text, size, font, color}. The brand lives in ~/content-os/brand/ (brand.md, tokens.css, video-style.md, logos/, fonts/): read brand.md before designing. Carbon #080c0e, Ice #acf4f8 as the single accent, SYSTM Display V2 for titles. Finish with one or two plain sentences in the user's language.";

fn spec_ix(id: &str) -> Option<usize> {
    CREW.iter().position(|c| c.id.eq_ignore_ascii_case(id.trim()))
}

/// The name and colour of whoever sent a command: a lead or a specialist.
pub fn identity(id: &str) -> (String, Color32) {
    if let Some(k) = Kind::from_id(id) {
        (k.name().to_string(), k.color())
    } else if let Some(c) = spec_ix(id).map(|i| &CREW[i]) {
        (c.name.to_string(), c.color)
    } else {
        (id.strip_prefix("script:").unwrap_or(id).to_string(), super::ICE)
    }
}

/// The colour of an agent's cursor and name, by its id.
pub fn color_of(id: &str) -> Color32 {
    identity(id).1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobState {
    Running,
    /// Finished, waiting for the user: keep or undo.
    Review,
    Kept,
    Undone,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Who {
    User,
    Agent(Kind),
    /// A step an agent took (a tool call), shown small.
    Action(Kind),
    Note,
    /// A specialist of the crew (index in `CREW`), and a step it took.
    Crew(usize),
    CrewAction(usize),
    /// The card of one job (everything the team did for one request).
    Job(JobState),
}

fn who_id(w: &Who) -> String {
    match w {
        Who::User => "user".into(),
        Who::Agent(k) => k.id().into(),
        Who::Action(k) => format!("{}.action", k.id()),
        Who::Note => "note".into(),
        Who::Crew(i) => format!("crew.{}", CREW[*i].id),
        Who::CrewAction(i) => format!("crew.{}.action", CREW[*i].id),
        Who::Job(s) => format!("job.{}", format!("{s:?}").to_lowercase()),
    }
}

fn who_from(s: &str) -> Who {
    match s {
        "user" => Who::User,
        "claude" => Who::Agent(Kind::Claude),
        "codex" => Who::Agent(Kind::Codex),
        "claude.action" => Who::Action(Kind::Claude),
        "codex.action" => Who::Action(Kind::Codex),
        "job.kept" => Who::Job(JobState::Kept),
        "job.undone" => Who::Job(JobState::Undone),
        // a job that was running or waiting when the app closed can no longer be taken back
        s if s.starts_with("job.") => Who::Job(JobState::Kept),
        s if s.starts_with("crew.") => {
            let rest = &s[5..];
            match (rest.strip_suffix(".action"), spec_ix(rest.trim_end_matches(".action"))) {
                (Some(_), Some(i)) => Who::CrewAction(i),
                (None, Some(i)) => Who::Crew(i),
                _ => Who::Note,
            }
        }
        _ => Who::Note,
    }
}

pub struct Msg {
    pub who: Who,
    pub text: String,
    pub files: Vec<String>,
}

impl Msg {
    fn new(who: Who, text: impl Into<String>) -> Self {
        Self { who, text: text.into(), files: Vec::new() }
    }
}

/// One conversation.
pub struct Thread {
    pub id: u64,
    pub title: String,
    pub log: Vec<Msg>,
    /// Each CLI's own conversation id (by `Kind::ix`), to continue it.
    sessions: [Option<String>; 2],
    pub updated: u64,
    /// The undo point from before the job in flight or under review.
    mark: Option<systmcut_engine::UndoMark>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Chat,
    History,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Read only: answer, do not change the project.
    Ask,
    Act,
}

enum Line {
    Out(String),
    Err(String),
    End,
}

struct Run {
    child: Child,
    rx: Receiver<Line>,
    thread: u64,
}

pub struct Agent {
    pub kind: Kind,
    pub on: bool,
    run: Option<Run>,
    last_err: String,
}

impl Agent {
    pub fn busy(&self) -> bool {
        self.run.is_some()
    }
}

pub struct Team {
    pub agents: Vec<Agent>,
    pub threads: Vec<Thread>,
    /// Open conversations, in tab order, and the one in front.
    tabs: Vec<u64>,
    active: u64,
    view: View,
    pub mode: Mode,
    pub draft: String,
    attach: Vec<PathBuf>,
    picking: Option<Receiver<Vec<PathBuf>>>,
    /// The message being edited (its index in the active thread) and its text.
    editing: Option<(usize, String)>,
    renaming: Option<(u64, String)>,
    search: String,
    pub wide: bool,
    /// Hands: the agents operate the window like a person, through `agent-desktop`.
    pub hands: bool,
    desk: Option<String>,
    pub needs_permission: bool,
    loaded: bool,
    next_id: u64,
    /// The microphone: recording, then turning the recording into text in the box.
    voice: Voice,
    /// Show every step of every job (off: a job's steps fold to its last two).
    all_steps: bool,
    /// Specialists at work: (index in `CREW`, the lead whose CLI runs it, the process).
    crew: Vec<(usize, Kind, Run)>,
    /// Reports of specialists that finished, waiting to go back to the lead that sent them: (thread, lead, specialist, report).
    reports: Vec<(u64, Kind, usize, String)>,
    /// How many times the leads were called back in the job in flight (a job cannot loop for ever).
    rounds: usize,
    /// The user's request of the job in flight (specialists answer in its language).
    request: String,
    /// What the specialists of the job in flight need to start: the channel, the hands session, the mode.
    ctx: Option<(super::Control, Option<String>, Mode)>,
}

impl Default for Team {
    fn default() -> Self {
        let mut t = Self {
            agents: Kind::ALL.into_iter().map(|kind| Agent { kind, on: true, run: None, last_err: String::new() }).collect(),
            threads: Vec::new(),
            tabs: Vec::new(),
            active: 0,
            view: View::Chat,
            mode: Mode::Act,
            draft: String::new(),
            attach: Vec::new(),
            picking: None,
            editing: None,
            renaming: None,
            search: String::new(),
            wide: false,
            hands: true,
            desk: None,
            needs_permission: false,
            loaded: false,
            voice: Voice::Idle,
            all_steps: false,
            crew: Vec::new(),
            reports: Vec::new(),
            rounds: 0,
            request: String::new(),
            ctx: None,
            next_id: 1,
        };
        t.new_tab();
        t
    }
}

enum Voice {
    Idle,
    /// ffmpeg is recording the microphone to this file since this moment.
    Recording(Child, PathBuf, std::time::Instant),
    Transcribing(Receiver<Result<String, String>>),
}

const TRANSCRIBE: &str = include_str!("transcribe.py");

impl Team {
    fn voice_start(&mut self) {
        let Some(ffmpeg) = super::which("ffmpeg") else {
            self.note("The microphone needs ffmpeg (brew install ffmpeg).");
            return;
        };
        let path = super::data_dir().join("voice.wav");
        let _ = std::fs::create_dir_all(super::data_dir());
        let _ = std::fs::remove_file(&path);
        // the default input device, mono 16 kHz: small, and what speech recognition wants
        match Command::new(ffmpeg).args(["-hide_banner", "-loglevel", "error", "-y", "-f", "avfoundation", "-i", ":default", "-ac", "1", "-ar", "16000", "-t", "180"]).arg(&path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => self.voice = Voice::Recording(child, path, std::time::Instant::now()),
            Err(e) => self.note(&format!("Could not start recording: {e}")),
        }
    }

    /// Stop recording; `keep` sends it to be transcribed.
    fn voice_stop(&mut self, keep: bool) {
        let Voice::Recording(mut child, path, _) = std::mem::replace(&mut self.voice, Voice::Idle) else { return };
        // `q` lets ffmpeg close the file properly
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(b"q");
        }
        let _ = child.wait();
        if !keep {
            return;
        }
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) < 4000 {
            self.note("Nothing was recorded. If macOS asked for the microphone, allow it and try again.");
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let script = super::data_dir().join("transcribe.py");
            let res = std::fs::write(&script, TRANSCRIBE).map_err(|e| e.to_string()).and_then(|_| {
                let py = super::which("python3").ok_or("python3 is not installed")?;
                let out = Command::new(py).arg(&script).arg(&path).env("PATH", super::child_path()).output().map_err(|e| e.to_string())?;
                if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
            });
            let _ = std::fs::remove_file(&path);
            let _ = tx.send(res);
        });
        self.voice = Voice::Transcribing(rx);
    }

    fn note(&mut self, text: &str) {
        let active = self.active;
        if let Some(t) = self.thread_mut(active) {
            t.log.push(Msg::new(Who::Note, text));
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn chat_file() -> PathBuf {
    super::data_dir().join("chat.json")
}

fn desk_cli() -> Option<PathBuf> {
    super::which("agent-desktop")
}

fn desk_json(args: &[&str]) -> Option<Value> {
    let out = Command::new(desk_cli()?).args(args).env("PATH", super::child_path()).stdin(Stdio::null()).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

/// Any string under a key named `key`, anywhere in `v`.
fn find_str(v: &Value, key: &str) -> Option<String> {
    match v {
        Value::Object(m) => m.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| m.values().find_map(|x| find_str(x, key))),
        Value::Array(a) => a.iter().find_map(|x| find_str(x, key)),
        _ => None,
    }
}

const BRIEF: &str = "You are an editor inside SYSTM Cut, a desktop video editor built like DaVinci Resolve: pages for each stage (MEDIA, CUT, EDIT, FUSION, COLOR, FAIRLIGHT, DEPTH, DELIVER) on a bar at the bottom of the window. The `systmcut` MCP tools drive the running app (timeline, project, effects, export). Look at the project state first, work in small visible steps, and finish with one or two plain sentences in the user's language saying what you changed. Do not edit files on disk. The SYSTM sound bank is at ~/content-os/sound (index.json lists every SFX and music file with category, tags and duration); finished SYSTM Depth videos are at ~/content-os/projects/<id>/edit/depth/depth-v*.mp4. Bring any of them in with the media import tool.";

const HANDS: &str = "You also have hands: the `agent-desktop` CLI operates the app's real window through the accessibility tree, and the user sees your own cursor move. Prefer it for anything a person would do with the mouse (changing page on the bottom bar, pressing buttons, dragging). Run `agent-desktop skills get desktop` once, then work in the snapshot/ref loop on the app named \"SYSTM Cut\". Pass these flags on every call:";

const READ_ONLY: &str = "ASK MODE: this request is read-only. Inspect and answer; do not change the project, the timeline or any setting.";

/// One-tap requests, by the page the user is on: (label, the request).
fn chips(page: Page) -> &'static [(&'static str, &'static str)] {
    match page {
        Page::Media => &[
            ("Organise bins", "Organise the project's media into bins by type and shoot, and name the clips clearly."),
            ("Find best takes", "Go over the footage in the project and list the strongest takes, with timecodes."),
            ("Bring Depth video", "Import the newest finished SYSTM Depth video into the project."),
        ],
        Page::Color => &[
            ("Balance shots", "Balance exposure and white balance across the clips of the active sequence so they match."),
            ("SYSTM look", "Grade the active sequence in the SYSTM look: cool deep shadows, clean skin, a soft roll-off in the highlights."),
            ("Match to clip", "Match the selected clip's colour to the clip before it."),
        ],
        Page::Fairlight => &[
            ("Clean the voice", "Clean and level the voice track of the active sequence so it sits at a steady, clear level."),
            ("Sound design", "Add sound effects from the SYSTM sound bank on the cuts and key moments, and a fitting music track ducked under the voice."),
            ("Mix to -14", "Mix the active sequence for social: about -14 LUFS, true peak under -1 dB."),
        ],
        Page::Fusion => &[
            ("Title", "Add an animated title on the selected clip in the SYSTM style: bold, one accent word in Ice."),
            ("Lower third", "Add a clean lower third with the speaker's name on the selected clip."),
        ],
        Page::Photo => &[
            ("קאבר מהפריים", "Make a Reels cover (1080x1920) from the timeline's current frame: bring the frame in, grade it like a film frame, and set a short Hebrew title in SYSTM Display V2 with one word in Ice. Have the brand guardian check it."),
            ("תמונה ממוזערת", "Design a YouTube thumbnail (1920x1080) for this video in the SYSTM brand: one idea, a big readable title, the face or the object large."),
            ("בדיקת מותג", "Check the open document against the SYSTM brand (colours, type, logo, spacing) and tell me exactly what to fix. Do not change anything."),
            ("גרייד קולנועי", "Grade the photograph in the open document like a film frame: deep clean shadows, natural skin, a soft highlight roll-off. Use adjustment layers."),
            ("שיפור טיפוגרפיה", "Improve the typography of the open document: hierarchy, sizes, spacing and line breaks, in Hebrew, keeping the wording."),
        ],
        Page::Three => &[
            ("לוגו בתלת-ממד", "Build a 3D title of the word SYSTM: extruded text in matte off-white ceramic on a dark Carbon stage, one Ice area light from the side, a camera framed for 16:9. Look at it and fix what is off."),
            ("במת מוצר", "Build a product stage: a round plinth, a soft backdrop that curves into the floor, three-point lighting with an Ice rim light, and a camera. Leave the plinth empty."),
            ("תאורה קולנועית", "Light the scene like a film frame: one strong key, a low fill, an Ice rim. Do not move the objects."),
            ("סיבוב מצלמה", "Animate the camera in a slow 120-frame orbit around the scene's centre, eased in and out."),
            ("רינדור לטיימליין", "Render the scene's camera at 1920x1080 and tell me the path of the image."),
        ],
        Page::Depth => &[
            ("Plan this reel", "Plan a SYSTM Depth edit for the selected project and tell me the concept before painting anything."),
            ("To timeline", "Bring the finished SYSTM Depth video of the selected project into the timeline."),
        ],
        Page::Deliver => &[
            ("Export reel", "Export the active sequence as a 9:16 H.264 file for Instagram, at high quality."),
            ("Check before export", "Check the active sequence for gaps, flash frames, audio clipping and missing captions before I export. Do not change anything."),
        ],
        _ => &[
            ("Cut silences", "Cut the silences and dead air out of the active sequence, keeping the speech natural."),
            ("Captions", "Add captions to the active sequence from its speech."),
            ("Sound design", "Add sound effects from the SYSTM sound bank on the cuts and key moments of the active sequence, and a fitting music track ducked under the voice."),
            ("Tighten", "Tighten the active sequence: remove repetitions and weak takes, keep the strongest version of every point."),
            ("Review", "Watch the active sequence and tell me, briefly, the three changes that would improve it most. Do not edit yet."),
        ],
    }
}

fn title_from(text: &str) -> String {
    let t: String = text.replace("@claude", "").replace("@codex", "").split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = t.chars().take(34).collect();
    if t.chars().count() > 34 {
        out.push('…');
    }
    if out.is_empty() { "New chat".into() } else { out }
}

impl Team {
    fn thread_mut(&mut self, id: u64) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }
    fn thread(&self, id: u64) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }
    fn active_thread(&self) -> Option<&Thread> {
        self.thread(self.active)
    }

    fn new_tab(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.threads.push(Thread { id, title: "New chat".into(), log: Vec::new(), sessions: [None, None], updated: now_secs(), mark: None });
        self.tabs.push(id);
        self.active = id;
        self.view = View::Chat;
    }

    fn open(&mut self, id: u64) {
        if !self.tabs.contains(&id) {
            self.tabs.push(id);
        }
        self.active = id;
        self.view = View::Chat;
        self.editing = None;
    }

    fn close_tab(&mut self, id: u64) {
        self.tabs.retain(|t| *t != id);
        // an empty conversation is not kept in the history
        if self.thread(id).is_some_and(|t| t.log.is_empty()) {
            self.threads.retain(|t| t.id != id);
        }
        if self.tabs.is_empty() {
            self.new_tab();
        } else if self.active == id {
            self.active = *self.tabs.last().unwrap_or(&0);
        }
    }

    fn delete(&mut self, id: u64) {
        if self.busy_in(id) {
            return;
        }
        self.tabs.retain(|t| *t != id);
        self.threads.retain(|t| t.id != id);
        if self.tabs.is_empty() {
            self.new_tab();
        } else if self.active == id {
            self.active = *self.tabs.last().unwrap_or(&0);
        }
    }

    pub fn busy(&self) -> bool {
        self.agents.iter().any(Agent::busy) || !self.crew.is_empty() || !self.reports.is_empty()
    }
    /// The rail has something moving (a job, or the microphone).
    pub fn live(&self) -> bool {
        self.busy() || !matches!(self.voice, Voice::Idle)
    }
    /// The specialists at work now (indexes in `CREW`).
    pub fn crew_busy(&self) -> Vec<usize> {
        self.crew.iter().map(|c| c.0).collect()
    }
    fn busy_in(&self, thread: u64) -> bool {
        self.agents.iter().any(|a| a.run.as_ref().is_some_and(|r| r.thread == thread)) || self.crew.iter().any(|c| c.2.thread == thread)
    }
    /// Jobs waiting for the user to keep or undo (the badge on the Team button).
    pub fn pending(&self) -> usize {
        self.threads.iter().flat_map(|t| &t.log).filter(|m| m.who == Who::Job(JobState::Review)).count()
    }

    /// The conversation in front, for `systm.state`.
    pub fn state(&self) -> Value {
        let t = self.active_thread();
        json!({
            "busy": self.busy(),
            "mode": if self.mode == Mode::Ask { "ask" } else { "act" },
            "thread": t.map(|t| json!({"id": t.id, "title": t.title})),
            "agents": self.agents.iter().map(|a| json!({"id": a.kind.id(), "on": a.on, "busy": a.busy(), "error": a.last_err})).collect::<Vec<_>>(),
            "log": t.map(|t| t.log.iter().map(|m| json!({"who": who_id(&m.who), "text": m.text})).collect::<Vec<_>>()).unwrap_or_default(),
        })
    }

    fn save(&self) {
        let threads: Vec<Value> = self
            .threads
            .iter()
            .filter(|t| !t.log.is_empty())
            .rev()
            .take(60)
            .map(|t| {
                json!({
                    "id": t.id, "title": t.title, "updated": t.updated,
                    "sessions": {"claude": t.sessions[0], "codex": t.sessions[1]},
                    "log": t.log.iter().rev().take(400).rev().map(|m| json!({"who": who_id(&m.who), "text": m.text, "files": m.files})).collect::<Vec<_>>(),
                })
            })
            .collect();
        let v = json!({"version": 2, "threads": threads, "tabs": self.tabs, "active": self.active, "mode": if self.mode == Mode::Ask { "ask" } else { "act" }, "hands": self.hands, "wide": self.wide});
        let _ = std::fs::create_dir_all(super::data_dir());
        let _ = std::fs::write(chat_file(), v.to_string());
    }

    fn load(&mut self) {
        self.loaded = true;
        let Some(v) = std::fs::read_to_string(chat_file()).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { return };
        let mut threads = Vec::new();
        for t in v.get("threads").and_then(Value::as_array).into_iter().flatten().rev() {
            let Some(id) = t.get("id").and_then(Value::as_u64) else { continue };
            let log = t
                .get("log")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|m| Msg {
                    who: who_from(m.get("who").and_then(Value::as_str).unwrap_or("")),
                    text: m.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                    files: m.get("files").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
                })
                .collect();
            let sess = |k: &str| t.pointer(&format!("/sessions/{k}")).and_then(Value::as_str).map(str::to_string);
            threads.push(Thread { id, title: t.get("title").and_then(Value::as_str).unwrap_or("Chat").to_string(), log, sessions: [sess("claude"), sess("codex")], updated: t.get("updated").and_then(Value::as_u64).unwrap_or(0), mark: None });
        }
        if threads.is_empty() {
            return;
        }
        self.next_id = threads.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        self.threads = threads;
        self.tabs = v.get("tabs").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).filter(|id| self.threads.iter().any(|t| t.id == *id)).collect()).unwrap_or_default();
        self.active = v.get("active").and_then(Value::as_u64).filter(|id| self.tabs.contains(id)).unwrap_or(0);
        if self.tabs.is_empty() {
            self.new_tab();
        } else if self.active == 0 {
            self.active = self.tabs[0];
        }
        self.mode = if v.get("mode").and_then(Value::as_str) == Some("ask") { Mode::Ask } else { Mode::Act };
        self.hands = v.get("hands").and_then(Value::as_bool).unwrap_or(true);
        self.wide = v.get("wide").and_then(Value::as_bool).unwrap_or(false);
    }

    /// Read what the running CLIs printed. Returns true while any is running.
    pub fn poll(&mut self) -> bool {
        if !self.loaded {
            self.load();
        }
        if let Some(rx) = &self.picking
            && let Ok(files) = rx.try_recv()
        {
            self.attach.extend(files);
            self.picking = None;
        }
        if let Voice::Transcribing(rx) = &self.voice
            && let Ok(res) = rx.try_recv()
        {
            self.voice = Voice::Idle;
            match res {
                Ok(text) => {
                    if !self.draft.is_empty() && !self.draft.ends_with(' ') {
                        self.draft.push(' ');
                    }
                    self.draft.push_str(&text);
                }
                Err(e) => self.note(&format!("Could not transcribe the recording ({e}).")),
            }
        }
        let mut changed = false;
        // (thread, lead, specialist, task): work the leads handed out in this pass
        let mut handed: Vec<(u64, Kind, usize, String)> = Vec::new();
        let mut over: Vec<u64> = Vec::new();
        for i in 0..self.agents.len() {
            let kind = self.agents[i].kind;
            let Some(run) = &mut self.agents[i].run else { continue };
            let tid = run.thread;
            let (out, err, ended) = drain(run);
            if let Some(e) = err {
                self.agents[i].last_err = e;
            }
            let mut fail = None;
            if let Some(t) = self.threads.iter_mut().find(|t| t.id == tid) {
                let from = t.log.len();
                for s in out {
                    let Ok(v) = serde_json::from_str::<Value>(&s) else { continue };
                    let who = (Who::Agent(kind), Who::Action(kind));
                    match kind {
                        Kind::Claude => read_claude(who, &v, &mut t.sessions[kind.ix()], &mut fail, &mut t.log),
                        Kind::Codex => read_codex(who, &v, &mut t.sessions[kind.ix()], &mut fail, &mut t.log),
                    }
                    t.updated = now_secs();
                    changed = true;
                }
                // a lead hands work to specialists with a ```dispatch block at the end of a message
                for m in &mut t.log[from..] {
                    if m.who == Who::Agent(kind)
                        && let Some((rest, tasks)) = take_dispatch(&m.text)
                    {
                        m.text = rest;
                        handed.extend(tasks.into_iter().map(|(spec, task)| (tid, kind, spec, task)));
                    }
                }
                t.log.retain(|m| !(m.who == Who::Agent(kind) && m.text.trim().is_empty()));
            }
            if let Some(f) = fail {
                self.agents[i].last_err = f;
            }
            if ended {
                let code = self.agents[i].run.take().and_then(|mut r| r.child.wait().ok()).and_then(|st| st.code());
                let why = self.agents[i].last_err.clone();
                if code != Some(0)
                    && let Some(t) = self.threads.iter_mut().find(|t| t.id == tid)
                {
                    t.log.push(Msg::new(Who::Note, format!("{} stopped: {}", kind.name(), if why.is_empty() { format!("exit {code:?}") } else { why })));
                }
                over.push(tid);
                changed = true;
            }
        }
        // the specialists
        let mut k = 0;
        while k < self.crew.len() {
            let (spec, engine, tid) = (self.crew[k].0, self.crew[k].1, self.crew[k].2.thread);
            let (out, err, ended) = drain(&mut self.crew[k].2);
            let mut fail = err;
            if let Some(t) = self.threads.iter_mut().find(|t| t.id == tid) {
                let mut session = None;
                for s in out {
                    let Ok(v) = serde_json::from_str::<Value>(&s) else { continue };
                    let who = (Who::Crew(spec), Who::CrewAction(spec));
                    match engine {
                        Kind::Claude => read_claude(who, &v, &mut session, &mut fail, &mut t.log),
                        Kind::Codex => read_codex(who, &v, &mut session, &mut fail, &mut t.log),
                    }
                    t.updated = now_secs();
                    changed = true;
                }
            }
            if ended {
                let (_, lead, mut run) = self.crew.remove(k);
                let code = run.child.wait().ok().and_then(|st| st.code());
                // what it said last is its report to the lead
                let report = self.threads.iter().find(|t| t.id == tid).and_then(|t| t.log.iter().rev().find(|m| m.who == Who::Crew(spec)).map(|m| m.text.clone())).unwrap_or_else(|| "(no report)".into());
                self.reports.push((tid, lead, spec, report));
                if code != Some(0)
                    && let Some(t) = self.threads.iter_mut().find(|t| t.id == tid)
                {
                    t.log.push(Msg::new(Who::Note, format!("{} stopped: {}", CREW[spec].name, fail.unwrap_or_else(|| format!("exit {code:?}")))));
                }
                over.push(tid);
                changed = true;
            } else {
                k += 1;
            }
        }
        // start what was handed out, each specialist once at a time
        for (tid, lead, spec, task) in handed {
            let Some((control, hands, mode)) = self.ctx.clone() else { continue };
            if self.crew.iter().any(|c| c.0 == spec && c.2.thread == tid) {
                continue;
            }
            let c = &CREW[spec];
            let desk = hands.as_ref().map(|s| format!("\n{HANDS} --session {s} --agent-id {} --headed", c.id)).unwrap_or_default();
            let lang = format!("\nThe user's request was: \"{}\". Write your report in the language of that request.", self.request.chars().take(160).collect::<String>());
            let brief = [BRIEF, PHOTO_BRIEF, THREE_BRIEF][c.team() as usize];
            let prompt = format!("{brief}{lang}\nYou are {}, the crew's specialist for {} Other specialists work in the same project at the same time, each on its own part: do only your task, and do not undo their work.{desk}\n\nTask from the lead ({}): {task}", c.name, c.craft, lead.name());
            let started = spawn(lead, c.id, &prompt, None, &control, hands.is_some(), mode, tid);
            if let Some(t) = self.threads.iter_mut().find(|t| t.id == tid) {
                match started {
                    Ok(run) => {
                        t.log.push(Msg::new(Who::CrewAction(spec), format!("קיבל משימה מ-{}: {task}", lead.name())));
                        self.crew.push((spec, lead, run));
                    }
                    Err(e) => t.log.push(Msg::new(Who::Note, format!("{}: {e}", c.name))),
                }
            }
            changed = true;
        }
        // when every specialist a lead sent is back, the lead gets their reports and goes on
        let waiting: Vec<(u64, Kind)> = self.reports.iter().map(|r| (r.0, r.1)).collect();
        for (tid, lead) in waiting {
            if self.crew.iter().any(|c| c.1 == lead && c.2.thread == tid) || self.agents.iter().any(|a| a.kind == lead && a.busy()) || !self.reports.iter().any(|r| r.0 == tid && r.1 == lead) {
                continue;
            }
            let mine: Vec<(usize, String)> = self.reports.iter().filter(|r| r.0 == tid && r.1 == lead).map(|r| (r.2, r.3.clone())).collect();
            self.reports.retain(|r| !(r.0 == tid && r.1 == lead));
            let Some((control, hands, mode)) = self.ctx.clone() else { continue };
            self.rounds += 1;
            let session = self.threads.iter().find(|t| t.id == tid).and_then(|t| t.sessions[lead.ix()].clone());
            if self.rounds > 6 || session.is_none() {
                continue;
            }
            let list: String = mine.iter().map(|(s, r)| format!("\n\n## {} reports\n{r}", CREW[*s].name)).collect();
            let prompt = format!("Your crew is back.{list}\n\nGo on with the job: hand out the next parts with a ```dispatch block (several specialists at once when the parts are separate), or, when the job is done, finish with a short summary for the user in the user's language and no dispatch block.");
            if let Ok(run) = spawn(lead, lead.id(), &prompt, session.as_deref(), &control, hands.is_some(), mode, tid)
                && let Some(a) = self.agents.iter_mut().find(|a| a.kind == lead)
            {
                a.run = Some(run);
                a.last_err.clear();
                over.retain(|t| *t != tid);
            }
            changed = true;
        }
        // a job is over when the last one working on it is done: ask the user to keep or undo
        for tid in over {
            if !self.busy_in(tid)
                && let Some(t) = self.threads.iter_mut().find(|t| t.id == tid)
                && let Some(card) = t.log.iter_mut().rev().find(|m| m.who == Who::Job(JobState::Running))
            {
                card.who = Who::Job(if t.mark.is_some() { JobState::Review } else { JobState::Kept });
            }
        }
        if changed && !self.busy() {
            self.save();
        }
        self.busy()
    }

    pub fn stop(&mut self) {
        let mut stopped: Vec<(u64, &'static str)> = Vec::new();
        for a in &mut self.agents {
            if let Some(mut r) = a.run.take() {
                let _ = r.child.kill();
                let _ = r.child.wait();
                stopped.push((r.thread, a.kind.name()));
            }
        }
        self.reports.clear();
        for (spec, _, mut r) in self.crew.drain(..) {
            let _ = r.child.kill();
            let _ = r.child.wait();
            stopped.push((r.thread, CREW[spec].name));
        }
        for (tid, name) in stopped {
            if let Some(t) = self.threads.iter_mut().find(|t| t.id == tid) {
                t.log.push(Msg::new(Who::Note, format!("{name} stopped")));
                if let Some(card) = t.log.iter_mut().rev().find(|m| m.who == Who::Job(JobState::Running)) {
                    card.who = Who::Job(if t.mark.is_some() { JobState::Review } else { JobState::Kept });
                }
            }
        }
        self.save();
    }

    /// Send `text` (with the attached files) in the active conversation.
    fn send(&mut self, text: &str, files: Vec<String>, page: Page, control: Option<&super::Control>, mark: systmcut_engine::UndoMark) {
        let text = text.trim().to_string();
        if text.is_empty() && files.is_empty() {
            return;
        }
        let tid = self.active;
        if self.busy_in(tid) {
            return;
        }
        let (mode, want_hands) = (self.mode, self.hands);
        let lower = text.to_lowercase();
        let named: Vec<Kind> = Kind::ALL.into_iter().filter(|k| lower.contains(&format!("@{}", k.id()))).collect();
        let team: Vec<Kind> = self.agents.iter().filter(|a| !a.busy() && if named.is_empty() { a.on } else { named.contains(&a.kind) }).map(|a| a.kind).collect();
        // hands: one shared agent-desktop session with a cursor per agent, started on first use
        let mut hands = None;
        if want_hands && mode == Mode::Act && desk_cli().is_some() {
            let granted = desk_json(&["permissions"]).is_some_and(|v| v.pointer("/data/accessibility/state").and_then(Value::as_str) == Some("granted"));
            self.needs_permission = !granted;
            if granted {
                if self.desk.is_none() {
                    self.desk = desk_json(&["session", "start", "--name", "systm-cut", "--cursor", "--multi-agent"]).and_then(|v| find_str(&v, "session_id").or_else(|| find_str(&v, "session")).or_else(|| find_str(&v, "id")));
                }
                hands = self.desk.clone();
            }
        }
        let Some(t) = self.threads.iter_mut().find(|t| t.id == tid) else { return };
        if t.log.is_empty() {
            t.title = title_from(&text);
        }
        t.updated = now_secs();
        // a job still under review is kept once the user moves on
        for m in &mut t.log {
            if m.who == Who::Job(JobState::Review) {
                m.who = Who::Job(JobState::Kept);
            }
        }
        t.log.push(Msg { who: Who::User, text: text.clone(), files: files.clone() });
        let Some(control) = control else {
            t.log.push(Msg::new(Who::Note, "The control channel is not running, so the agents cannot reach the editor. Restart SYSTM Cut."));
            return;
        };
        if team.is_empty() {
            t.log.push(Msg::new(Who::Note, "No agent is free: switch one on above, or wait for the running job."));
            return;
        }
        t.mark = (mode == Mode::Act).then_some(mark);
        if mode == Mode::Act {
            t.log.push(Msg::new(Who::Job(JobState::Running), text.clone()));
        }
        let attached = if files.is_empty() { String::new() } else { format!("\n\nAttached files (read them from disk):\n{}", files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")) };
        let context = format!("\nThe user is on the {} page.{}", page.label(), if mode == Mode::Ask { format!("\n{READ_ONLY}") } else { String::new() });
        self.ctx = Some((control.clone(), hands.clone(), mode));
        self.rounds = 0;
        self.reports.clear();
        self.request = text.clone();
        let team_id = team_of(page);
        let brief = [BRIEF, PHOTO_BRIEF, THREE_BRIEF][team_id as usize];
        let crew = if mode == Mode::Act { crew_brief(team_id) } else { String::new() };
        let mut started = Vec::new();
        for kind in &team {
            let session = t.sessions[kind.ix()].clone();
            let desk = hands.as_ref().map(|s| format!("\n{HANDS} --session {s} --agent-id {} --headed", kind.id())).unwrap_or_default();
            let prompt = if session.is_some() {
                format!("{text}{attached}\n{context}{crew}")
            } else if team.len() > 1 {
                format!("{brief}\nYou work beside another agent in the same project at the same time; you are {}. Split the work sensibly and do not undo what the other one does.{desk}{crew}{context}\n\nRequest: {text}{attached}", kind.name())
            } else {
                format!("{brief}{desk}{crew}{context}\n\nRequest: {text}{attached}")
            };
            match spawn(*kind, kind.id(), &prompt, session.as_deref(), control, hands.is_some(), mode, tid) {
                Ok(run) => started.push((*kind, run)),
                Err(e) => t.log.push(Msg::new(Who::Note, format!("{}: {e}", kind.name()))),
            }
        }
        if started.is_empty()
            && let Some(card) = t.log.iter_mut().rev().find(|m| m.who == Who::Job(JobState::Running))
        {
            card.who = Who::Job(JobState::Kept);
        }
        for (kind, run) in started {
            if let Some(a) = self.agents.iter_mut().find(|a| a.kind == kind) {
                a.run = Some(run);
                a.last_err.clear();
            }
        }
        self.save();
    }
}

/// Everything a run printed since the last look: (stdout lines, the last stderr line, it ended).
fn drain(run: &mut Run) -> (Vec<String>, Option<String>, bool) {
    let (mut out, mut err, mut ended) = (Vec::new(), None, false);
    while let Ok(l) = run.rx.try_recv() {
        match l {
            Line::Out(s) => out.push(s),
            Line::Err(s) => err = Some(s),
            Line::End => ended = true,
        }
    }
    (out, err, ended)
}

/// The ```dispatch block of a lead's message: the message without it, and (specialist, task) pairs.
fn take_dispatch(text: &str) -> Option<(String, Vec<(usize, String)>)> {
    let start = text.find("```dispatch")?;
    let body_from = start + "```dispatch".len();
    let end = text[body_from..].find("```").map(|e| body_from + e)?;
    let v: Value = serde_json::from_str(text[body_from..end].trim()).ok()?;
    let tasks: Vec<(usize, String)> = v
        .as_array()?
        .iter()
        .filter_map(|x| Some((spec_ix(x.get("to")?.as_str()?)?, x.get("task")?.as_str()?.trim().to_string())))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    let rest = format!("{}{}", &text[..start], &text[end + 3..]).trim().to_string();
    (!tasks.is_empty()).then_some((rest, tasks))
}

/// What a lead is told about its crew.
fn crew_brief(team: u8) -> String {
    let list: String = CREW.iter().filter(|c| c.team() == team).map(|c| format!("\n- {} ({}): {}", c.id, c.name, c.craft)).collect();
    format!("\nYou lead a crew of specialists. Each runs as its own agent with its own cursor that the user watches live:{list}\nTo hand work out, END your message with a block exactly like this (JSON array, one entry per specialist, the task written so the specialist can do it with no other context):\n```dispatch\n[{{\"to\": \"<id>\", \"task\": \"...\"}}, {{\"to\": \"<id>\", \"task\": \"...\"}}]\n```\nThe ones you name start at once and work at the same time. Decide like a supervising editor: a small or single-skill request, do it yourself; a job with separate parts, hand each part to its specialist; a whole piece from nothing, start with the one who plans it (the planner for an edit, the art director for a design) alone, hand the parts out in your next message once the plan is in, and before you call it done have it checked (on a design: by the brand guardian). Say in one line who is doing what. This block is the ONLY way to delegate: you have no other sub-agent tool, and work you hand out any other way does not happen.")
}

fn bridge_args(control: &super::Control) -> Result<(String, Vec<String>), String> {
    let cli = super::bridge_cli().ok_or("the bridge (systmcut-cli) is not next to the app")?;
    Ok((cli.display().to_string(), vec!["mcp".into(), "--bridge".into(), format!("127.0.0.1:{}", control.port)]))
}

fn spawn(kind: Kind, who: &str, prompt: &str, session: Option<&str>, control: &super::Control, hands: bool, mode: Mode, thread: u64) -> Result<Run, String> {
    let (cli, args) = bridge_args(control)?;
    let token_file = control.token_file.display().to_string();
    let dir = super::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut cmd = match kind {
        Kind::Claude => {
            let exe = super::which("claude").ok_or("Claude Code (claude) is not installed")?;
            let cfg = dir.join(format!("mcp-{who}.json"));
            let body = json!({"mcpServers": {"systmcut": {"command": cli, "args": args, "env": {"SYSTMCUT_CONTROL_TOKEN_FILE": token_file, "SYSTM_AGENT_ID": who}}}});
            std::fs::write(&cfg, body.to_string()).map_err(|e| e.to_string())?;
            let mut c = Command::new(exe);
            c.args(["-p", prompt, "--output-format", "stream-json", "--verbose", "--strict-mcp-config", "--mcp-config"]).arg(&cfg).args(["--allowedTools", "mcp__systmcut", "Read"]);
            // its own sub-agents would work unseen under its name: the crew is how work is handed out here
            c.args(["--disallowedTools", "Agent", "Task"]);
            if hands {
                c.arg("Bash(agent-desktop:*)");
            }
            if let Some(s) = session {
                c.args(["--resume", s]);
            }
            c
        }
        Kind::Codex => {
            let exe = super::which("codex").ok_or("Codex (codex) is not installed")?;
            let mut c = Command::new(exe);
            c.arg("exec");
            if let Some(s) = session {
                c.args(["resume", s]);
            }
            let q = |s: &str| serde_json::to_string(s).unwrap_or_default();
            // the editor's tools run without asking in Act (the job card can take all of it back); in Ask they
            // are not pre-approved. The shell cannot write outside its folder either way.
            c.args(["--json", "--skip-git-repo-check", "-c", if hands { "sandbox_mode=\"workspace-write\"" } else { "sandbox_mode=\"read-only\"" }]);
            if mode == Mode::Act {
                c.args(["-c", "mcp_servers.systmcut.default_tools_approval_mode=\"approve\""]);
            }
            c.args(["-c", &format!("mcp_servers.systmcut.command={}", q(&cli))])
                .args(["-c", &format!("mcp_servers.systmcut.args=[{}]", args.iter().map(|a| q(a)).collect::<Vec<_>>().join(","))])
                .args(["-c", &format!("mcp_servers.systmcut.env={{SYSTMCUT_CONTROL_TOKEN_FILE={},SYSTM_AGENT_ID={}}}", q(&token_file), q(who))])
                .arg(prompt);
            c
        }
    };
    cmd.current_dir(&dir).env("AGENT_DESKTOP_AGENT_ID", who).env("PATH", super::child_path()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let (tx, rx) = channel();
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(err).lines().map_while(Result::ok) {
                if !l.trim().is_empty() && tx.send(Line::Err(l)).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(Line::Out(l)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Line::End);
        });
    }
    Ok(Run { child, rx, thread })
}

/// A step in plain words: what the agent is doing, not the tool's name.
fn step_label(name: &str, input: &Value) -> String {
    let short = name.rsplit("__").next().unwrap_or(name);
    let arg = input.get("command").or_else(|| input.get("id")).or_else(|| input.get("method")).or_else(|| input.get("query")).and_then(Value::as_str).unwrap_or("");
    let what = match short {
        "project_inspect" => "Reading the project",
        "sequence_inspect" => "Reading the timeline",
        "doc_inspect" => "Reading the document",
        "command_list" => "Looking up commands",
        "command_run" | "command_batch" => "Editing",
        "media_import" => "Importing media",
        "render_frame" => "Looking at a frame",
        "render_preview" => "Watching a preview",
        "ui_click" => "Clicking",
        "ui_drag" => "Dragging",
        "ui_control" => "Using the interface",
        "ToolSearch" => "Finding tools",
        "Bash" | "exec" | "shell" => "Using its hands",
        "Read" => "Reading a file",
        other => other,
    };
    if arg.is_empty() || matches!(short, "ToolSearch" | "command_list") { what.to_string() } else { format!("{what} · {arg}") }
}

fn read_claude(who: (Who, Who), v: &Value, session: &mut Option<String>, fail: &mut Option<String>, log: &mut Vec<Msg>) {
    match v.get("type").and_then(Value::as_str) {
        Some("system") => {
            if let Some(s) = v.get("session_id").and_then(Value::as_str) {
                *session = Some(s.to_string());
            }
        }
        Some("assistant") => {
            for c in v.pointer("/message/content").and_then(Value::as_array).into_iter().flatten() {
                match c.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = c.get("text").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                            log.push(Msg::new(who.0.clone(), t.trim()));
                        }
                    }
                    Some("tool_use") => log.push(Msg::new(who.1.clone(), step_label(c.get("name").and_then(Value::as_str).unwrap_or("tool"), c.get("input").unwrap_or(&Value::Null)))),
                    _ => {}
                }
            }
        }
        Some("result") => {
            if v.get("is_error").and_then(Value::as_bool) == Some(true) {
                *fail = Some(v.get("result").and_then(Value::as_str).unwrap_or("error").to_string());
            }
        }
        _ => {}
    }
}

fn read_codex(who: (Who, Who), v: &Value, session: &mut Option<String>, fail: &mut Option<String>, log: &mut Vec<Msg>) {
    match v.get("type").and_then(Value::as_str) {
        Some("thread.started") => {
            if let Some(s) = v.get("thread_id").and_then(Value::as_str) {
                *session = Some(s.to_string());
            }
        }
        Some("item.completed") => {
            let Some(item) = v.get("item") else { return };
            match item.get("type").and_then(Value::as_str) {
                Some("agent_message") => {
                    if let Some(t) = item.get("text").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                        log.push(Msg::new(who.0.clone(), t.trim()));
                    }
                }
                Some("mcp_tool_call") => log.push(Msg::new(who.1.clone(), step_label(item.get("tool").and_then(Value::as_str).unwrap_or("tool"), item.get("arguments").unwrap_or(&Value::Null)))),
                Some("command_execution") => log.push(Msg::new(who.1.clone(), "Using its hands")),
                _ => {}
            }
        }
        Some("error") | Some("turn.failed") => {
            *fail = Some(v.get("message").or_else(|| v.pointer("/error/message")).and_then(Value::as_str).unwrap_or("error").to_string());
        }
        _ => {}
    }
}

/// Send `text` to the team in the conversation in front (the panel, the chips, `systm.chat`).
pub fn ask(app: &mut SystmCutApp, text: &str) {
    let files: Vec<String> = std::mem::take(&mut app.systm.agents.attach).iter().map(|p| p.display().to_string()).collect();
    let (mark, page, control) = (app.session.undo_mark(), app.systm.page, app.systm.control.clone());
    app.systm.agents.send(text, files, page, control.as_ref(), mark);
}

// ---------- drawing: the rail is Archie's panel from the SYSTM CRM, value for value ----------
// (#0d1417 panel with a 26 px radius; 18 px bubbles: the user's in Ice at 12%, the reply in white at 5%;
// a 22 px input box that lights Ice on focus; round 38 px buttons; a segmented Ask / Act pill)

const A_PANEL: Color32 = Color32::from_rgb(0x0d, 0x14, 0x17);
const A_LINE: Color32 = Color32::from_rgb(0x2c, 0x39, 0x3f);
const A_INK: Color32 = Color32::from_rgb(0xf5, 0xf7, 0xf7);
const A_MUTED: Color32 = Color32::from_rgb(0xa8, 0xb4, 0xb8);
const A_WARN: Color32 = Color32::from_rgb(0xff, 0xd4, 0x8a);
const A_OK: Color32 = Color32::from_rgb(0x7b, 0xe3, 0xa1);
fn soft() -> Color32 {
    Color32::from_white_alpha(23) // rgb(255 255 255 / .09)
}

#[derive(Clone, Copy)]
enum Ic {
    History,
    Wide,
    Close,
    Plus,
    Clip,
    Send,
    Stop,
    Copy,
    Edit,
    Retry,
    Tool,
    Search,
    Trash,
    Mic,
}

/// Line icons drawn in a `r` box (Lucide-like, 1.6 px strokes).
fn icon(p: &egui::Painter, r: egui::Rect, ic: Ic, col: Color32) {
    let s = Stroke::new(1.6, col);
    let at = |x: f32, y: f32| pos2(r.min.x + x * r.width(), r.min.y + y * r.height());
    let line = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::line(pts.iter().map(|(x, y)| at(*x, *y)).collect(), s));
    };
    match ic {
        Ic::History => {
            p.circle_stroke(r.center(), r.width() * 0.42, s);
            line(&[(0.5, 0.26), (0.5, 0.5), (0.68, 0.6)]);
        }
        Ic::Wide => {
            line(&[(0.6, 0.12), (0.88, 0.12), (0.88, 0.4)]);
            line(&[(0.88, 0.12), (0.58, 0.42)]);
            line(&[(0.4, 0.88), (0.12, 0.88), (0.12, 0.6)]);
            line(&[(0.12, 0.88), (0.42, 0.58)]);
        }
        Ic::Close => {
            line(&[(0.2, 0.2), (0.8, 0.8)]);
            line(&[(0.8, 0.2), (0.2, 0.8)]);
        }
        Ic::Plus => {
            line(&[(0.5, 0.15), (0.5, 0.85)]);
            line(&[(0.15, 0.5), (0.85, 0.5)]);
        }
        Ic::Clip => line(&[(0.78, 0.44), (0.44, 0.8), (0.24, 0.8), (0.14, 0.62), (0.2, 0.48), (0.58, 0.12), (0.74, 0.12), (0.82, 0.24), (0.76, 0.36), (0.42, 0.68), (0.34, 0.66), (0.34, 0.56), (0.66, 0.26)]),
        Ic::Send => {
            line(&[(0.5, 0.84), (0.5, 0.16)]);
            line(&[(0.22, 0.44), (0.5, 0.16), (0.78, 0.44)]);
        }
        Ic::Stop => {
            p.rect_filled(r.shrink(r.width() * 0.26), 2.0, col);
        }
        Ic::Copy => {
            p.rect_stroke(egui::Rect::from_min_max(at(0.34, 0.34), at(0.86, 0.86)), 2.0, s, egui::StrokeKind::Inside);
            line(&[(0.66, 0.34), (0.66, 0.14), (0.14, 0.14), (0.14, 0.66), (0.34, 0.66)]);
        }
        Ic::Edit => {
            line(&[(0.16, 0.84), (0.2, 0.64), (0.66, 0.18), (0.82, 0.34), (0.36, 0.8), (0.16, 0.84)]);
        }
        Ic::Retry => {
            line(&[(0.2, 0.5), (0.24, 0.32), (0.4, 0.18), (0.6, 0.18), (0.78, 0.34), (0.82, 0.54), (0.7, 0.74), (0.5, 0.82), (0.32, 0.76)]);
            line(&[(0.14, 0.2), (0.2, 0.5), (0.48, 0.42)]);
        }
        Ic::Tool => {
            line(&[(0.16, 0.84), (0.56, 0.44)]);
            p.circle_stroke(at(0.68, 0.32), r.width() * 0.2, s);
        }
        Ic::Search => {
            p.circle_stroke(at(0.44, 0.44), r.width() * 0.28, s);
            line(&[(0.64, 0.64), (0.86, 0.86)]);
        }
        Ic::Mic => {
            p.rect_stroke(egui::Rect::from_min_max(at(0.36, 0.1), at(0.64, 0.58)), r.width() * 0.14, s, egui::StrokeKind::Inside);
            line(&[(0.2, 0.44), (0.24, 0.62), (0.38, 0.74), (0.62, 0.74), (0.76, 0.62), (0.8, 0.44)]);
            line(&[(0.5, 0.74), (0.5, 0.92)]);
        }
        Ic::Trash => {
            line(&[(0.16, 0.26), (0.84, 0.26)]);
            line(&[(0.26, 0.26), (0.32, 0.86), (0.68, 0.86), (0.74, 0.26)]);
            line(&[(0.4, 0.26), (0.42, 0.12), (0.58, 0.12), (0.6, 0.26)]);
        }
    }
}

/// A square icon button (`.icon-btn.ghost`, `.msg-tools button`): the icon, a soft plate on hover.
fn icon_btn(ui: &mut egui::Ui, size: f32, ic: Ic, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let tip_s = tip.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip_s.clone()));
    if resp.hovered() {
        ui.painter().rect_filled(r, size * 0.28, Color32::from_white_alpha(26));
    }
    icon(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(size * 0.5, size * 0.5)), ic, if resp.hovered() { A_INK } else { A_MUTED });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A round 38 px button (`.send`, `.mic`): filled Ice, or outlined.
fn round_btn(ui: &mut egui::Ui, ic: Ic, filled: bool, enabled: bool, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), if enabled { Sense::click() } else { Sense::hover() });
    let tip_s = tip.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip_s.clone()));
    let p = ui.painter();
    let c = r.center();
    let rad = if resp.is_pointer_button_down_on() { 17.5 } else { 19.0 };
    if filled {
        p.circle_filled(c, rad, if enabled { super::ICE } else { super::ICE.gamma_multiply(0.35) });
        icon(p, egui::Rect::from_center_size(c, vec2(17.0, 17.0)), ic, Color32::from_rgb(0x06, 0x18, 0x1b));
    } else {
        let hot = resp.hovered() && enabled;
        p.circle_filled(c, rad, Color32::from_white_alpha(13));
        p.circle_stroke(c, rad, Stroke::new(1.0, if hot { super::ICE } else { A_LINE }));
        icon(p, egui::Rect::from_center_size(c, vec2(17.0, 17.0)), ic, if hot { super::ICE } else { A_INK });
    }
    resp.on_hover_text(tip)
}

/// A chip (`.chip`): a pill with a line border; muted, ink on hover.
fn chip(ui: &mut egui::Ui, label: &str, tip: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(13.0), A_MUTED);
    let (r, resp) = ui.allocate_exact_size(g.size() + vec2(28.0, 14.0), Sense::click());
    let l = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
    let hot = resp.hovered();
    ui.painter().rect_stroke(r, r.height() / 2.0, Stroke::new(1.0, if hot { A_MUTED } else { A_LINE }), egui::StrokeKind::Inside);
    ui.painter().galley(r.center() - g.size() / 2.0, g, if hot { A_INK } else { A_MUTED });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn file_name(p: &str) -> String {
    std::path::Path::new(p).file_name().map_or_else(|| p.to_string(), |n| n.to_string_lossy().to_string())
}

/// What the user did in the panel this frame (applied after drawing, when nothing is borrowed).
enum Do {
    Send(String),
    Stop,
    NewTab,
    Open(u64),
    Close(u64),
    Delete(u64),
    Rename(u64, String),
    Keep(usize),
    Undo(usize),
    Resend(usize, String),
    Pick,
    Detach(usize),
    AllowHands,
    HideRail,
    Mic,
    MicCancel,
}

fn pick_files() -> Receiver<Vec<PathBuf>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        // the system's own file chooser, without blocking the editor
        let script = "set fs to choose file with prompt \"Attach to the chat\" with multiple selections allowed\nset out to \"\"\nrepeat with f in fs\nset out to out & POSIX path of f & linefeed\nend repeat\nreturn out";
        let files = Command::new("osascript").args(["-e", script]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.trim().is_empty()).map(|l| PathBuf::from(l.trim())).collect()).unwrap_or_default();
        let _ = tx.send(files);
    });
    rx
}

/// The team's mark (where Archie has his portrait): a rounded tile with the two agents' colours.
fn emblem(ui: &mut egui::Ui, size: f32, glow: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    if glow {
        for k in 1..=5 {
            p.rect_filled(r.expand(k as f32 * 5.0), size * 0.27 + k as f32 * 5.0, super::ICE.gamma_multiply(0.022));
        }
    }
    p.rect(r, size * 0.3, Color32::from_rgb(0x08, 0x10, 0x14), Stroke::new(1.0, super::ICE.gamma_multiply(0.45)), egui::StrokeKind::Inside);
    let d = size * 0.15;
    p.circle_filled(r.center() - vec2(d, 0.0), size * 0.13, Kind::Claude.color());
    p.circle_filled(r.center() + vec2(d, 0.0), size * 0.13, Kind::Codex.color());
}

fn rule(ui: &mut egui::Ui) {
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([pos2(r.min.x - 14.0, r.min.y), pos2(r.max.x + 14.0, r.min.y)], Stroke::new(1.0, soft()));
}

fn header(team: &mut Team, ui: &mut egui::Ui, page: Page, act: &mut Vec<Do>) {
    // `.archie-head`: portrait, name, then history / wide / close
    ui.horizontal(|ui| {
        ui.set_height(38.0);
        emblem(ui, 30.0, false);
        ui.add_space(2.0);
        ui.label(RichText::new(match page { Page::Photo => "צוות העיצוב", Page::Three => "צוות התלת-ממד", _ => "הצוות" }).font(Tokens::semibold(18.0)).color(A_INK));
        ui.add_space(6.0);
        for a in &mut team.agents {
            let on = a.on;
            let name = if a.busy() { format!("{} ···", a.kind.name()) } else { a.kind.name().to_string() };
            let g = ui.painter().layout_no_wrap(name.clone(), Tokens::semibold(11.5), Color32::WHITE);
            let (r, resp) = ui.allocate_exact_size(g.size() + vec2(18.0, 9.0), Sense::click());
            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, on, name.clone()));
            let col = a.kind.color();
            ui.painter().rect(r, r.height() / 2.0, if on { col.gamma_multiply(0.16) } else { Color32::TRANSPARENT }, Stroke::new(1.0, if on { col.gamma_multiply(0.55) } else { A_LINE }), egui::StrokeKind::Inside);
            ui.painter().galley(r.center() - g.size() / 2.0, g, if on { col } else { A_MUTED.gamma_multiply(0.6) });
            if resp.on_hover_text(if on { "פעיל: מקבל את הבקשות שלך. לחיצה מכבה" } else { "כבוי. לחיצה מפעילה" }).clicked() {
                a.on = !a.on;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if icon_btn(ui, 34.0, Ic::Close, "סגירה").clicked() {
                act.push(Do::HideRail);
            }
            if icon_btn(ui, 34.0, Ic::Wide, if team.wide { "הקטנה" } else { "הרחבה" }).clicked() {
                team.wide = !team.wide;
            }
            if icon_btn(ui, 34.0, Ic::History, "היסטוריית שיחות").clicked() {
                team.view = if team.view == View::History { View::Chat } else { View::History };
            }
        });
    });
    // the crew: the specialists the leads hand work to; one lights up while it works
    let working = team.crew_busy();
    ui.add_space(4.0);
    egui::ScrollArea::horizontal().id_salt("systm-crew").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let tm = ui.ctx().input(|i| i.time) as f32;
            for (i, c) in CREW.iter().enumerate().filter(|(_, c)| c.team() == team_of(page)) {
                let on = working.contains(&i);
                let g = ui.painter().layout_no_wrap(c.he.to_string(), egui::FontId::proportional(11.0), Color32::WHITE);
                let (r, resp) = ui.allocate_exact_size(g.size() + vec2(24.0, 8.0), Sense::hover());
                ui.painter().rect(r, r.height() / 2.0, if on { c.color.gamma_multiply(0.16) } else { Color32::TRANSPARENT }, Stroke::new(1.0, if on { c.color.gamma_multiply(0.6) } else { soft() }), egui::StrokeKind::Inside);
                let dot = pos2(r.max.x - 10.0, r.center().y);
                ui.painter().circle_filled(dot, 3.0, if on { c.color } else { c.color.gamma_multiply(0.45) });
                if on {
                    ui.painter().circle_stroke(dot, 4.5 + (tm * 5.0 + i as f32).sin(), Stroke::new(1.0, c.color.gamma_multiply(0.7)));
                }
                ui.painter().galley(pos2(r.min.x + 9.0, r.center().y - g.size().y / 2.0), g, if on { c.color } else { A_MUTED });
                resp.on_hover_text(format!("{}: {}", c.name, c.craft));
            }
        });
    });
    ui.add_space(6.0);
    rule(ui);
    ui.add_space(8.0);
    // `.archie-tabs`: the open conversations, then "+"
    ui.horizontal(|ui| {
        ui.set_height(32.0);
        egui::ScrollArea::horizontal().id_salt("systm-tabs").max_width(ui.available_width() - 40.0).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            for id in team.tabs.clone() {
                let on = id == team.active && team.view == View::Chat;
                let Some(title) = team.thread(id).map(|t| t.title.clone()) else { continue };
                let busy = team.busy_in(id);
                egui::Frame::new().fill(if on { Color32::from_white_alpha(20) } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0, if on { A_LINE } else { Color32::TRANSPARENT })).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin { left: 10, right: 6, top: 5, bottom: 5 }).show(ui, |ui| {
                    if let Some((rid, val)) = &mut team.renaming
                        && *rid == id
                    {
                        let e = ui.add(egui::TextEdit::singleline(val).desired_width(130.0));
                        e.request_focus();
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            act.push(Do::Rename(id, val.clone()));
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            act.push(Do::Rename(id, String::new()));
                        }
                    } else {
                        let short: String = title.chars().take(20).collect();
                        let text = if busy { format!("{short} ···") } else { short };
                        let r = ui.add(egui::Label::new(RichText::new(text).size(13.0).color(if on { A_INK } else { A_MUTED })).sense(Sense::click())).on_hover_text("לחיצה כפולה לשינוי שם");
                        if r.double_clicked() {
                            team.renaming = Some((id, title.clone()));
                        } else if r.clicked() {
                            act.push(Do::Open(id));
                        }
                    }
                    if icon_btn(ui, 18.0, Ic::Close, "סגירת לשונית").clicked() {
                        act.push(Do::Close(id));
                    }
                });
            }
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if icon_btn(ui, 30.0, Ic::Plus, "שיחה חדשה").clicked() {
                act.push(Do::NewTab);
            }
        });
    });
    ui.add_space(8.0);
    rule(ui);
}

fn history(team: &mut Team, ui: &mut egui::Ui, act: &mut Vec<Do>) {
    ui.add_space(14.0);
    egui::Frame::new().fill(Color32::from_rgb(0x08, 0x0c, 0x0e)).stroke(Stroke::new(1.0, A_LINE)).corner_radius(CornerRadius::same(14)).inner_margin(egui::Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icon(ui.painter(), r, Ic::Search, A_MUTED);
            ui.add(egui::TextEdit::singleline(&mut team.search).hint_text("חיפוש בשיחות").desired_width(f32::INFINITY).frame(egui::Frame::NONE));
        });
    });
    ui.add_space(12.0);
    let q = team.search.to_lowercase();
    let mut rows: Vec<(u64, String, u64, usize)> = team.threads.iter().filter(|t| !t.log.is_empty() && (q.is_empty() || t.title.to_lowercase().contains(&q) || t.log.iter().any(|m| m.text.to_lowercase().contains(&q)))).map(|t| (t.id, t.title.clone(), t.updated, t.log.iter().filter(|m| m.who == Who::User).count())).collect();
    rows.sort_by(|a, b| b.2.cmp(&a.2));
    egui::ScrollArea::vertical().id_salt("systm-history").auto_shrink([false, false]).show(ui, |ui| {
        if rows.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| ui.label(RichText::new(if q.is_empty() { "עדיין אין שיחות." } else { "לא נמצאו שיחות." }).size(14.0).color(A_MUTED)));
        }
        let now = now_secs();
        for (id, title, updated, n) in rows {
            egui::Frame::new().fill(Color32::from_white_alpha(10)).stroke(Stroke::new(1.0, soft())).corner_radius(CornerRadius::same(16)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let lines = crate::rtl::Lines::new(ui.painter(), &title, Tokens::semibold(14.5), A_INK, ui.available_width());
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), lines.size.y), Sense::click());
                lines.paint(ui.painter(), r, A_INK);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    act.push(Do::Open(id));
                }
                ui.horizontal(|ui| {
                    let age = now.saturating_sub(updated);
                    let when = if age < 3600 { format!("לפני {} דק׳", age / 60) } else if age < 86400 { format!("לפני {} שע׳", age / 3600) } else { format!("לפני {} ימים", age / 86400) };
                    ui.label(RichText::new(format!("{n} בקשות · {when}")).size(12.0).color(A_MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if icon_btn(ui, 26.0, Ic::Trash, "מחיקת שיחה").clicked() {
                            act.push(Do::Delete(id));
                        }
                        if icon_btn(ui, 26.0, Ic::Edit, "שינוי שם").clicked() {
                            team.renaming = Some((id, title.clone()));
                            act.push(Do::Open(id));
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
    });
}

/// A paragraph laid out for a bubble: `**bold**`, `` `code` ``, wrapped at `wrap`, in reading order.
fn layout(ui: &egui::Ui, text: &str, size: f32, color: Color32, all_bold: bool, wrap: f32) -> (std::sync::Arc<egui::Galley>, bool) {
    let rtl = crate::rtl::has_rtl(text);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    job.direction = TextDirection::Auto;
    job.halign = if rtl { Align::RIGHT } else { Align::LEFT };
    job.halign_min_is_start = true;
    let (plain, bold, mono) = (egui::FontId::proportional(size), Tokens::semibold(size), egui::FontId::monospace(size - 1.5));
    let (mut b, mut c) = (all_bold, false);
    let mut rest = text;
    while !rest.is_empty() {
        let next = [rest.find("**"), rest.find('`')].into_iter().flatten().min().unwrap_or(rest.len());
        if next > 0 {
            let font = if c { mono.clone() } else if b { bold.clone() } else { plain.clone() };
            let mut f = TextFormat::simple(font, if c { super::ICE } else if b { Color32::WHITE } else { color });
            f.line_height = Some(size * 1.6);
            job.append(&rest[..next], 0.0, f);
        }
        if next >= rest.len() {
            break;
        }
        if rest[next..].starts_with("**") {
            b = !b;
            rest = &rest[next + 2..];
        } else {
            c = !c;
            rest = &rest[next + 1..];
        }
    }
    (ui.painter().layout_job(job), rtl)
}

/// Paint a laid-out paragraph with its ink inside `r` (to the right when it reads right to left).
fn put(ui: &egui::Ui, g: std::sync::Arc<egui::Galley>, rtl: bool, r: egui::Rect) {
    let ink = g.rect;
    let left = if rtl { r.max.x - ink.width() } else { r.min.x };
    ui.painter().galley(pos2(left - ink.min.x, r.min.y - ink.min.y), g, Color32::WHITE);
}

fn para(ui: &mut egui::Ui, text: &str, size: f32, color: Color32, all_bold: bool) {
    let w = ui.available_width().max(40.0);
    let (g, rtl) = layout(ui, text, size, color, all_bold, w);
    let (r, _) = ui.allocate_exact_size(vec2(w, g.rect.height()), Sense::hover());
    put(ui, g, rtl, r);
}

/// A reply with headings, lists, code blocks and bold.
fn markdown(ui: &mut egui::Ui, text: &str) {
    let mut code = false;
    for line in text.lines() {
        let t = line.trim_end();
        if t.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if code {
            egui::Frame::new().fill(Color32::from_black_alpha(90)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(10, 3)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t).monospace().size(12.5).color(super::ICE));
            });
            continue;
        }
        let s = t.trim_start();
        if s.is_empty() {
            ui.add_space(6.0);
        } else if let Some(h) = s.strip_prefix("### ").or_else(|| s.strip_prefix("## ")).or_else(|| s.strip_prefix("# ")) {
            ui.add_space(4.0);
            para(ui, h, 16.0, Color32::WHITE, true);
        } else if let Some(item) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")) {
            para(ui, &format!("•  {item}"), 15.0, A_INK, false);
        } else {
            para(ui, s, 15.0, A_INK, false);
        }
    }
}

/// `.msg.me`: the user's bubble, hugging its text on the right, at most 88% wide.
fn me_bubble(ui: &mut egui::Ui, m: &Msg) {
    let avail = ui.available_width();
    let (pad_x, pad_y) = (14.0, 10.0);
    let wrap = avail * 0.88 - pad_x * 2.0;
    let (g, rtl) = layout(ui, &m.text, 15.0, A_INK, false, wrap);
    let files: Vec<std::sync::Arc<egui::Galley>> = m.files.iter().map(|f| ui.painter().layout_no_wrap(file_name(f), egui::FontId::proportional(13.0), A_INK)).collect();
    let files_h: f32 = files.iter().map(|f| f.size().y + 14.0).sum();
    let text_h = if m.text.is_empty() { 0.0 } else { g.rect.height() };
    let inner_w = files.iter().map(|f| f.size().x + 22.0).fold(if m.text.is_empty() { 0.0 } else { g.rect.width() }, f32::max).min(wrap).max(12.0);
    let size = vec2(inner_w + pad_x * 2.0, text_h + files_h + pad_y * 2.0);
    let (row, _) = ui.allocate_exact_size(vec2(avail, size.y), Sense::hover());
    let r = egui::Rect::from_min_size(pos2(row.max.x - size.x, row.min.y), size);
    ui.painter().rect(r, 18.0, super::ICE.gamma_multiply(0.12), Stroke::new(1.0, super::ICE.gamma_multiply(0.3)), egui::StrokeKind::Inside);
    let mut y = r.min.y + pad_y;
    for f in files {
        let fr = egui::Rect::from_min_size(pos2(r.min.x + pad_x, y), vec2(f.size().x + 20.0, f.size().y + 8.0));
        ui.painter().rect_filled(fr, 10.0, Color32::from_white_alpha(20));
        ui.painter().galley(pos2(fr.min.x + 10.0, fr.min.y + 4.0), f, A_INK);
        y += fr.height() + 6.0;
    }
    if !m.text.is_empty() {
        put(ui, g, rtl, egui::Rect::from_min_size(pos2(r.min.x + pad_x, y), vec2(inner_w, text_h)));
    }
}

fn bot_frame() -> egui::Frame {
    egui::Frame::new().fill(Color32::from_white_alpha(13)).stroke(Stroke::new(1.0, soft())).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::symmetric(14, 10))
}

fn messages(team: &mut Team, ui: &mut egui::Ui, page: Page, act: &mut Vec<Do>) {
    let busy = team.busy_in(team.active);
    let Some(t) = team.threads.iter().find(|t| t.id == team.active) else { return };
    if t.log.is_empty() {
        // `.archie-hello`
        ui.add_space((ui.available_height() * 0.5 - 170.0).max(18.0));
        ui.vertical_centered(|ui| {
            emblem(ui, 112.0, true);
            ui.add_space(14.0);
            ui.label(RichText::new("היי, אנחנו הצוות שלך").font(Tokens::semibold(22.0)).color(A_INK));
            ui.add_space(4.0);
            ui.scope(|ui| {
                ui.set_max_width(340.0);
                let text = "Claude ו-Codex עורכים איתך את הפרויקט: חיתוך, צבע, סאונד, כתוביות ועומק. כל עבודה נגמרת בכרטיס: משאירים, או מבטלים הכול בלחיצה.";
                let (g, _) = layout(ui, text, 14.5, A_MUTED, false, 340.0);
                let (r, _) = ui.allocate_exact_size(vec2(340.0, g.rect.height()), Sense::hover());
                // centred: the hello paragraph is the one centred text in the panel
                ui.painter().galley(pos2(r.center().x - g.rect.width() / 2.0 - g.rect.min.x, r.min.y), g, A_MUTED);
            });
        });
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (label, prompt) in chips(page) {
                if chip(ui, label, prompt).clicked() {
                    act.push(Do::Send((*prompt).to_string()));
                }
            }
        });
        return;
    }
    ui.spacing_mut().item_spacing.y = 10.0;
    let last_user = t.log.iter().rposition(|m| m.who == Who::User);
    let is_step = |m: &Msg| matches!(m.who, Who::Action(_) | Who::CrewAction(_));
    let all_steps = team.all_steps;
    let mut toggle_steps = false;
    for (i, m) in t.log.iter().enumerate() {
        if !all_steps && is_step(m) {
            // how many steps follow this one in the same run
            let after = t.log[i + 1..].iter().take_while(|x| is_step(x)).count();
            let before = t.log[..i].iter().rev().take_while(|x| is_step(x)).count();
            if after >= 2 {
                if before == 0 {
                    let hidden = after - 1;
                    if ui.add(egui::Label::new(RichText::new(format!("＋ {hidden} צעדים קודמים")).size(12.0).color(A_MUTED)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        toggle_steps = true;
                    }
                }
                continue;
            }
        }
        match &m.who {
            Who::User => {
                if let Some((ei, text)) = &mut team.editing
                    && *ei == i
                {
                    egui::Frame::new().fill(super::ICE.gamma_multiply(0.12)).stroke(Stroke::new(1.0, super::ICE.gamma_multiply(0.3))).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::TextEdit::multiline(text).desired_rows(3).desired_width(f32::INFINITY).font(egui::FontId::proportional(15.0)));
                        ui.label(RichText::new("שימו לב: פעולות שכבר בוצעו בהמשך השיחה לא יבוטלו.").size(12.0).color(A_MUTED));
                        ui.horizontal(|ui| {
                            if ui_kit::primary(ui, "שמירה ושליחה מחדש").clicked() {
                                act.push(Do::Resend(i, text.clone()));
                            }
                            if ui_kit::ghost(ui, "ביטול").clicked() {
                                act.push(Do::Resend(usize::MAX, String::new()));
                            }
                        });
                    });
                    continue;
                }
                me_bubble(ui, m);
                // `.msg-tools`
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if icon_btn(ui, 26.0, Ic::Copy, "העתקה").clicked() {
                        ui.ctx().copy_text(m.text.clone());
                    }
                    if !busy && icon_btn(ui, 26.0, Ic::Edit, "עריכה ושליחה מחדש").clicked() {
                        team.editing = Some((i, m.text.clone()));
                    }
                    if !busy && Some(i) == last_user && icon_btn(ui, 26.0, Ic::Retry, "ניסיון חוזר").clicked() {
                        act.push(Do::Resend(i, m.text.clone()));
                    }
                });
            }
            Who::Agent(_) | Who::Crew(_) if m.text.trim().is_empty() => {}
            Who::Agent(k) => {
                // `.msg.bot`, with the agent's name above it (Archie is one agent; this is a team)
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, k.color());
                    ui.label(RichText::new(k.name()).font(Tokens::semibold(12.0)).color(k.color()));
                });
                bot_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0;
                    markdown(ui, &m.text);
                });
                if icon_btn(ui, 26.0, Ic::Copy, "העתקה").clicked() {
                    ui.ctx().copy_text(m.text.clone());
                }
            }
            Who::Action(k) => {
                // `.msg.tool`
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    icon(ui.painter(), r, Ic::Tool, k.color().gamma_multiply(0.9));
                    ui.label(RichText::new(&m.text).size(12.5).color(A_MUTED));
                });
            }
            Who::Crew(ix) => {
                let c = &CREW[*ix];
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, c.color);
                    ui.label(RichText::new(format!("{} · {}", c.name, c.he)).font(Tokens::semibold(12.0)).color(c.color));
                });
                bot_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0;
                    markdown(ui, &m.text);
                });
            }
            Who::CrewAction(ix) => {
                let c = &CREW[*ix];
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    icon(ui.painter(), r, Ic::Tool, c.color.gamma_multiply(0.9));
                    let w = ui.available_width().max(60.0);
                    let short: String = m.text.chars().take(120).collect();
                    let (g, rtl) = layout(ui, &format!("{}  {}{}", c.name, short, if m.text.chars().count() > 120 { "…" } else { "" }), 12.5, A_MUTED, false, w);
                    let (r, _) = ui.allocate_exact_size(vec2(w, g.rect.height()), Sense::hover());
                    put(ui, g, rtl, r);
                });
            }
            Who::Note => {
                // `.msg.note`
                ui.vertical_centered(|ui| ui.label(RichText::new(&m.text).size(13.0).color(A_WARN)));
            }
            Who::Job(state) => {
                // `.msg.action`: amber while it waits for the user, green once done
                let (col, head) = match state {
                    JobState::Running => (super::ICE, "הצוות עובד"),
                    JobState::Review => (A_WARN, "ממתין לאישור שלך"),
                    JobState::Kept => (A_OK, "בוצע"),
                    JobState::Undone => (A_MUTED, "בוטל"),
                };
                egui::Frame::new().fill(col.gamma_multiply(0.07)).stroke(Stroke::new(1.0, col.gamma_multiply(0.45))).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.set_width((ui.available_width()).min(380.0));
                    ui.horizontal(|ui| {
                        if *state == JobState::Running {
                            ui.spinner();
                        }
                        ui.label(RichText::new(head).font(Tokens::semibold(15.0)).color(A_INK));
                    });
                    let steps = t.log[i + 1..].iter().take_while(|x| !matches!(x.who, Who::User | Who::Job(_))).filter(|x| matches!(x.who, Who::Action(_) | Who::CrewAction(_))).count();
                    let summary = match state {
                        JobState::Running => format!("{steps} צעדים עד עכשיו"),
                        JobState::Review => format!("העבודה הסתיימה ב-{steps} צעדים. להשאיר את השינויים בפרויקט?"),
                        JobState::Kept => format!("{steps} צעדים"),
                        JobState::Undone => "הפרויקט חזר למצב שלפני העבודה".to_string(),
                    };
                    para(ui, &summary, 13.5, A_MUTED, false);
                    if *state == JobState::Review {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui_kit::primary(ui, "אישור והשארה").clicked() {
                                act.push(Do::Keep(i));
                            }
                            if ui_kit::ghost(ui, "ביטול הכול").on_hover_text("מחזיר את הפרויקט למצב שלפני העבודה").clicked() {
                                act.push(Do::Undo(i));
                            }
                        });
                    }
                });
            }
        }
    }
    if all_steps && t.log.iter().any(is_step) && ui.add(egui::Label::new(RichText::new("קיפול הצעדים").size(12.0).color(A_MUTED)).sense(Sense::click())).clicked() {
        toggle_steps = true;
    }
    if toggle_steps {
        team.all_steps = !team.all_steps;
    }
    if busy {
        // `.msg.typing`
        let (r, _) = ui.allocate_exact_size(vec2(58.0, 30.0), Sense::hover());
        ui.painter().rect_filled(r, 16.0, Color32::from_white_alpha(13));
        let tm = ui.ctx().input(|i| i.time) as f32;
        for d in 0..3 {
            let a = if ((tm / 1.2 - d as f32 * 0.167).fract() + 1.0).fract() < 0.5 { 1.0 } else { 0.4 };
            ui.painter().circle_filled(pos2(r.min.x + 17.0 + d as f32 * 12.0, r.center().y), 3.5, super::ICE.gamma_multiply(a));
        }
    }
}

fn composer(team: &mut Team, ui: &mut egui::Ui, page: Page, act: &mut Vec<Do>) {
    let busy = team.busy_in(team.active);
    if team.needs_permission && team.hands {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("macOS עוד לא אישר שליטה במסך, אז הסוכנים עובדים בלי ידיים.").size(12.0).color(A_WARN));
            if ui.add(egui::Label::new(RichText::new("לאישור").size(12.0).color(super::ICE)).sense(Sense::click())).clicked() {
                act.push(Do::AllowHands);
            }
        });
        ui.add_space(4.0);
    }
    // `.archie-box`: Ice border and a soft ring while the text has focus
    let focus_id = egui::Id::unique("systm-chat-input");
    let focused = ui.memory(|m| m.has_focus(focus_id));
    let frame = egui::Frame::new().fill(Color32::from_rgba_unmultiplied(4, 9, 11, 153)).stroke(Stroke::new(1.0, if focused { super::ICE } else { A_LINE })).corner_radius(CornerRadius::same(22)).inner_margin(egui::Margin::symmetric(12, 10));
    let out = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        if !team.attach.is_empty() {
            // `.archie-atts`
            ui.horizontal_wrapped(|ui| {
                for (i, f) in team.attach.iter().enumerate() {
                    egui::Frame::new().fill(Color32::from_white_alpha(13)).stroke(Stroke::new(1.0, A_LINE)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        icon(ui.painter(), r, Ic::Clip, super::ICE);
                        let name: String = file_name(&f.display().to_string()).chars().take(22).collect();
                        ui.label(RichText::new(name).size(12.5).color(A_INK));
                        if icon_btn(ui, 20.0, Ic::Close, "הסרה").clicked() {
                            act.push(Do::Detach(i));
                        }
                    });
                }
            });
            ui.add_space(4.0);
        }
        let hint = if team.mode == Mode::Ask { "שאל משהו על הפרויקט…" } else { "מה לעשות? למשל: תהדקו את הפתיחה ל-3 שניות" };
        let edit = ui.add(egui::TextEdit::multiline(&mut team.draft).id(focus_id).hint_text(hint).font(egui::FontId::proportional(16.0)).desired_rows(1).desired_width(f32::INFINITY).frame(egui::Frame::NONE));
        let enter = edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        if enter {
            team.draft = team.draft.trim_end_matches('\n').to_string();
        }
        ui.add_space(4.0);
        // `.archie-box-row`: attach, the Ask / Act pill, where we are, send
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if round_btn(ui, Ic::Clip, false, true, "צירוף תמונה, וידאו, סאונד או מסמך").clicked() {
                act.push(Do::Pick);
            }
            // `.seg`
            egui::Frame::new().fill(Color32::from_white_alpha(10)).stroke(Stroke::new(1.0, A_LINE)).corner_radius(CornerRadius::same(99)).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (m, label, tip) in [(Mode::Ask, "שאל", "קריאה בלבד"), (Mode::Act, "פעל", "יכול לבצע פעולות")] {
                    let on = team.mode == m;
                    let g = ui.painter().layout_no_wrap(label.to_string(), Tokens::semibold(13.0), Color32::WHITE);
                    let (r, resp) = ui.allocate_exact_size(g.size() + vec2(28.0, 8.0), Sense::click());
                    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label));
                    if on {
                        ui.painter().rect_filled(r, r.height() / 2.0, super::ICE);
                    }
                    ui.painter().galley(r.center() - g.size() / 2.0, g, if on { Color32::from_rgb(0x06, 0x18, 0x1b) } else { A_MUTED });
                    if resp.on_hover_text(tip).clicked() {
                        team.mode = m;
                    }
                }
            });
            {
                // hands: the one control Archie does not have
                let on = team.hands;
                let g = ui.painter().layout_no_wrap("ידיים".to_string(), Tokens::semibold(13.0), Color32::WHITE);
                let (r, resp) = ui.allocate_exact_size(g.size() + vec2(24.0, 14.0), Sense::click());
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, on, "ידיים"));
                ui.painter().rect(r, r.height() / 2.0, if on { super::ICE.gamma_multiply(0.14) } else { Color32::TRANSPARENT }, Stroke::new(1.0, if on { super::ICE.gamma_multiply(0.5) } else { A_LINE }), egui::StrokeKind::Inside);
                ui.painter().galley(r.center() - g.size() / 2.0, g, if on { super::ICE } else { A_MUTED });
                if resp.on_hover_text("הסוכנים מפעילים את החלון כמו בן אדם, כל אחד עם סמן משלו").clicked() {
                    team.hands = !team.hands;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                match &team.voice {
                    Voice::Recording(_, _, since) => {
                        // `.mic.rec`: red and pulsing, the time beside it, a small cancel
                        let (r, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "סיום הקלטה"));
                        let tm = ui.ctx().input(|i| i.time) as f32;
                        let red = Color32::from_rgb(0xff, 0x5c, 0x5c);
                        ui.painter().circle_stroke(r.center(), 19.0 + (tm * 4.5).sin().abs() * 5.0, Stroke::new(2.0, red.gamma_multiply(0.35)));
                        ui.painter().circle_filled(r.center(), 19.0, red);
                        icon(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(17.0, 17.0)), Ic::Mic, Color32::WHITE);
                        if resp.on_hover_text("סיום הקלטה").clicked() {
                            act.push(Do::Mic);
                        }
                        let s = since.elapsed().as_secs();
                        ui.label(RichText::new(format!("{}:{:02}", s / 60, s % 60)).size(13.0).color(Color32::from_rgb(0xff, 0x9c, 0x8a)));
                        if icon_btn(ui, 28.0, Ic::Close, "ביטול הקלטה").clicked() {
                            act.push(Do::MicCancel);
                        }
                        ui.ctx().request_repaint();
                        return;
                    }
                    Voice::Transcribing(_) => {
                        ui.add_sized([38.0, 38.0], egui::Spinner::new());
                        ui.label(RichText::new("מתמלל…").size(12.0).color(A_MUTED));
                        ui.ctx().request_repaint();
                        return;
                    }
                    Voice::Idle => {}
                }
                if busy {
                    if round_btn(ui, Ic::Stop, true, true, "עצירה").clicked() {
                        act.push(Do::Stop);
                    }
                } else {
                    let ready = !team.draft.trim().is_empty() || !team.attach.is_empty();
                    if (round_btn(ui, Ic::Send, true, ready, "שליחה").clicked() || enter) && ready {
                        act.push(Do::Send(std::mem::take(&mut team.draft)));
                    }
                }
                if round_btn(ui, Ic::Mic, false, true, "הקלטה קולית").clicked() {
                    act.push(Do::Mic);
                }
                // `.archie-ctx`
                ui.label(RichText::new(format!("עמוד {}", page.label())).size(12.0).color(A_MUTED)).on_hover_text("הצוות יודע באיזה עמוד אתה נמצא");
            });
        });
    });
    if focused {
        ui.painter().rect_stroke(out.response.rect.expand(3.0), 25.0, Stroke::new(4.0, super::ICE.gamma_multiply(0.12)), egui::StrokeKind::Outside);
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: egui::Rect) {
    let page = app.systm.page;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(rect.min + vec2(14.0, 12.0), rect.max - vec2(14.0, 12.0))));
    let ui = &mut child;
    let mut act = Vec::new();
    // a finished job that changed nothing in the project needs no decision
    let now_mark = app.session.undo_mark();
    let team = &mut app.systm.agents;
    for t in &mut team.threads {
        if t.mark == Some(now_mark)
            && let Some(card) = t.log.iter().rposition(|m| m.who == Who::Job(JobState::Review))
        {
            // nothing to keep or undo: an answer needs no card at all
            t.log.remove(card);
            t.mark = None;
        }
    }
    header(team, ui, page, &mut act);
    if team.view == View::History {
        history(team, ui, &mut act);
    } else {
        let composer_h = 118.0 + if team.attach.is_empty() { 0.0 } else { 34.0 } + if team.needs_permission && team.hands { 40.0 } else { 0.0 };
        let log_h = (ui.available_height() - composer_h).max(60.0);
        egui::ScrollArea::vertical().id_salt(("systm-chat", team.active)).max_height(log_h).min_scrolled_height(log_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            ui.add_space(14.0);
            messages(team, ui, page, &mut act);
            ui.add_space(8.0);
        });
        ui.add_space(8.0);
        composer(team, ui, page, &mut act);
    }
    for a in act {
        apply(app, a);
    }
}

fn finish_job(app: &mut SystmCutApp, i: usize, undo: bool) {
    let team = &mut app.systm.agents;
    let active = team.active;
    let mark = team.thread_mut(active).and_then(|t| {
        if let Some(m) = t.log.get_mut(i) {
            m.who = Who::Job(if undo { JobState::Undone } else { JobState::Kept });
        }
        t.mark.take()
    });
    team.save();
    if undo && let Some(mark) = mark {
        let n = app.session.revert_to(mark);
        app.status(format!("Took back the team's job ({n} steps)"));
    }
}

fn apply(app: &mut SystmCutApp, a: Do) {
    let team = &mut app.systm.agents;
    match a {
        Do::Send(text) => ask(app, &text),
        Do::Stop => team.stop(),
        Do::NewTab => team.new_tab(),
        Do::Open(id) => team.open(id),
        Do::Close(id) => {
            team.close_tab(id);
            team.save();
        }
        Do::Delete(id) => {
            team.delete(id);
            team.save();
        }
        Do::Rename(id, title) => {
            team.renaming = None;
            if !title.trim().is_empty()
                && let Some(t) = team.thread_mut(id)
            {
                t.title = title.trim().chars().take(80).collect();
            }
            team.save();
        }
        Do::Keep(i) => finish_job(app, i, false),
        Do::Undo(i) => finish_job(app, i, true),
        Do::Resend(i, text) => {
            team.editing = None;
            if i == usize::MAX {
                return;
            }
            let active = team.active;
            let files = team.thread_mut(active).map(|t| {
                let files = t.log.get(i).map(|m| m.files.clone()).unwrap_or_default();
                t.log.truncate(i);
                // the CLIs' own histories still hold the old turn: start them fresh
                t.sessions = [None, None];
                files
            });
            team.attach = files.unwrap_or_default().into_iter().map(PathBuf::from).collect();
            ask(app, &text);
        }
        Do::Pick => {
            if team.picking.is_none() {
                team.picking = Some(pick_files());
            }
        }
        Do::Detach(i) => {
            if i < team.attach.len() {
                team.attach.remove(i);
            }
        }
        Do::HideRail => app.systm.rail = false,
        Do::Mic => {
            if matches!(team.voice, Voice::Recording(..)) { team.voice_stop(true) } else if matches!(team.voice, Voice::Idle) { team.voice_start() }
        }
        Do::MicCancel => team.voice_stop(false),
        Do::AllowHands => {
            if let Some(cli) = desk_cli() {
                let _ = Command::new(cli).args(["permissions", "--guide"]).spawn();
            }
        }
    }
}
