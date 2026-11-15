//! SYSTM Cut: the SYSTM layer on top of SYSTM Cut.
//!
//! - `agents`: the side chat and the agent team (Claude and Codex processes that drive this app
//!   through the control channel, `systmcut-cli mcp --bridge`).
//! - `presence`: every agent's cursor, drawn over the UI where its last command landed.
//! - `depth`: the SYSTM Depth engine (Blender, Vision, Codex image_gen) as a panel.
//! - `bank`: the SYSTM sound bank (SFX and music) as a panel.
//! - `studio`: the companion tools (Image, Motion/Fusion, 3D/Blender, Audio) launched beside the app.
//!
//! All SYSTM code lives in this module (plus one line in each SYSTM Cut file that calls it), so
//! SYSTM Cut can be updated underneath with small merges.

pub mod agents;
pub mod bank;
pub mod cut_page;
pub mod depth;
pub mod edit_page;
pub mod pages;
pub mod photo_page;
pub mod fx_library;
pub mod ico;
pub mod inspector;
pub mod presence;
pub mod scripts;
pub mod shell;
pub mod studio;
pub mod three_d;
pub mod timeline;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::SystmCutApp;
use crate::dock::PanelKind;

/// SYSTM brand colours.
pub const ICE: egui::Color32 = egui::Color32::from_rgb(0xac, 0xf4, 0xf8);
pub const INK: egui::Color32 = egui::Color32::from_rgb(0x06, 0x10, 0x14);

/// The control channel this app listens on, for its own agents.
#[derive(Clone, Debug)]
pub struct Control {
    pub port: u16,
    /// Path of the 0600 file that holds the session token (agents' MCP bridges read it; the token
    /// never goes into a command line or a config file).
    pub token_file: PathBuf,
}

#[derive(Default)]
pub struct SystmState {
    pub control: Option<Control>,
    pub agents: agents::Team,
    pub presence: presence::Presence,
    pub depth: depth::DepthState,
    pub bank: bank::BankState,
    pub studio: studio::StudioState,
    /// Where each panel was drawn this frame (for pointing a cursor at a panel).
    pub panel_rects: HashMap<PanelKind, egui::Rect>,
    /// The page of the film's journey that is open (the bar at the bottom).
    pub page: shell::Page,
    /// The team rail on the right is open.
    pub rail: bool,
    pub started: bool,
    /// The optional panels that are open (the toggles of the top bar).
    pub open: shell::Open,
    /// Workspace ▸ Scripts: the installed plugins.
    pub scripts: scripts::ScriptsState,
    /// The timeline's view (zoom, scroll, tool).
    pub timeline: timeline::TimelineState,
    /// The Edit page's own state (the pool's bin, the Inspector's tab).
    pub edit: edit_page::EditState,
    pub inspector: inspector::InspectorState,
    pub library: fx_library::LibraryState,
    /// The Photo page: its documents and tools.
    pub photo: photo_page::PhotoState,
    /// The 3D page: its link to Blender.
    pub three: three_d::ThreeState,
}

/// The app's own data folder: `~/Library/Application Support/SYSTM Cut` on macOS,
/// `%APPDATA%\\SYSTM Cut` on Windows, `~/.local/share/SYSTM Cut` elsewhere.
pub fn data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("SYSTM Cut");
    }
    if cfg!(target_os = "macos") {
        return home().join("Library").join("Application Support").join("SYSTM Cut");
    }
    home().join(".local").join("share").join("SYSTM Cut")
}

/// The user's home directory.
pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `~/content-os`: the SYSTM content system (depth engine, sound bank, projects).
pub fn content_os() -> PathBuf {
    std::env::var_os("SYSTM_CONTENT_OS").map(PathBuf::from).unwrap_or_else(|| home().join("content-os"))
}

/// Find a command-line tool: apps started from the Finder get a bare PATH, so look in the usual
/// install places too.
pub fn which(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let h = home();
    for d in [h.join(".local/bin"), PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin"), PathBuf::from("/usr/bin"), PathBuf::from("/bin")] {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    // Windows programs carry an extension: try the usual ones
    let names: Vec<String> = if cfg!(target_os = "windows") { vec![format!("{name}.exe"), format!("{name}.cmd"), format!("{name}.bat"), name.to_string()] } else { vec![name.to_string()] };
    dirs.into_iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// A PATH for child processes that includes the usual install places.
pub fn child_path() -> String {
    let h = home();
    let mut v: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if !cfg!(target_os = "windows") {
        v.extend([h.join(".local/bin"), "/opt/homebrew/bin".into(), "/usr/local/bin".into(), "/usr/bin".into(), "/bin".into(), "/usr/sbin".into(), "/sbin".into()]);
    }
    std::env::join_paths(v).map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

/// The companion CLI that bridges MCP to this app (`systmcut-cli`), next to our executable in the
/// app bundle, else on PATH.
pub fn bridge_cli() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for n in ["systmcut-cli", "systmcut-cli.exe"] {
        let p = dir.join(n);
        if p.is_file() {
            return Some(p);
        }
    }
    which("systmcut-cli")
}

/// Called once per frame from the app's logic: poll agent processes, depth jobs and launches.
pub fn tick(app: &mut SystmCutApp, ctx: &egui::Context) {
    let was = app.systm.scripts.last.clone();
    let scripts = app.systm.scripts.poll();
    if app.systm.scripts.last != was {
        let m = app.systm.scripts.last.clone();
        app.status(m);
    }
    let busy = scripts | app.systm.agents.poll() | app.systm.depth.poll() | app.systm.presence.animating(ctx.input(|i| i.time));
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

/// Remember where a panel was drawn (called by `panels::show`).
pub fn note_panel(app: &mut SystmCutApp, p: PanelKind, rect: egui::Rect) {
    app.systm.panel_rects.insert(p, rect);
}

/// Draw a SYSTM panel. Returns false when `p` is not one of ours.
pub fn show_panel(app: &mut SystmCutApp, ui: &mut egui::Ui, p: PanelKind, rect: egui::Rect) -> bool {
    match p {
        PanelKind::Agents => agents::show(app, ui, rect),
        PanelKind::Depth => depth::show(app, ui, rect),
        PanelKind::SoundBank => bank::show(app, ui, rect),
        PanelKind::Studio => studio::show(app, ui, rect),
        PanelKind::Timeline => timeline::show(app, ui, rect),
        PanelKind::Properties | PanelKind::EffectControls => {
            let page = app.systm.page;
            inspector::show(app, ui, rect, page)
        }
        PanelKind::Effects => {
            let page = app.systm.page;
            fx_library::show(app, ui, rect, page)
        }
        PanelKind::Nodes => pages::nodes(app, ui, rect),
        PanelKind::ClipStrip => pages::clip_strip(app, ui, rect),
        PanelKind::Gallery => pages::gallery(app, ui, rect),
        _ => return false,
    }
    true
}

/// Small helpers shared by the SYSTM panels.
pub(crate) mod ui_kit {
    use egui::{Color32, CornerRadius, RichText, Stroke};

    pub fn pill(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
        let (fill, ink) = if on { (super::ICE, super::INK) } else { (Color32::from_rgb(0x1b, 0x22, 0x26), Color32::from_rgb(0x9a, 0xa7, 0xad)) };
        ui.add(egui::Button::new(RichText::new(text).color(ink).strong()).fill(fill).corner_radius(CornerRadius::same(12)).stroke(Stroke::NONE))
    }

    pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.add(egui::Button::new(RichText::new(text).color(super::INK).strong()).fill(super::ICE).corner_radius(CornerRadius::same(8)))
    }

    pub fn ghost(ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.add(egui::Button::new(RichText::new(text)).fill(Color32::from_rgb(0x1b, 0x22, 0x26)).corner_radius(CornerRadius::same(8)))
    }

    pub fn section(ui: &mut egui::Ui, text: &str) {
        ui.add_space(6.0);
        ui.label(RichText::new(text).small().strong().color(Color32::from_rgb(0x7d, 0x8a, 0x91)));
        ui.add_space(2.0);
    }
}
