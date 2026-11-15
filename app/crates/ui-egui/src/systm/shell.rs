//! The SYSTM Cut shell: a Resolve-style page bar at the bottom (Media, Cut, Edit, Fusion, Color,
//! Fairlight, Depth, Deliver), a slim brand bar on top and the agent team as a rail on the right.
//! Every page is a purpose-built arrangement of panels; the page bar is how a person or an agent
//! (through the accessibility tree: every button is labelled) moves between the stages of a film.

use egui::{Color32, Rect, Sense, Stroke, pos2, vec2};

use crate::SystmCutApp;
use crate::state::Mode;
use crate::theme::Tokens;

pub const TOP_H: f32 = 42.0;
pub const DOCK_H: f32 = 54.0;
pub const RAIL_W: f32 = 326.0;
const GAP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Hash)]
pub enum Page {
    Media,
    Cut,
    #[default]
    Edit,
    Fusion,
    Photo,
    Three,
    Color,
    Fairlight,
    Depth,
    Deliver,
}

impl Page {
    pub const ALL: [Page; 10] = [Page::Media, Page::Cut, Page::Edit, Page::Fusion, Page::Photo, Page::Three, Page::Color, Page::Fairlight, Page::Depth, Page::Deliver];
    pub fn label(self) -> &'static str {
        match self {
            Page::Media => "MEDIA",
            Page::Cut => "CUT",
            Page::Edit => "EDIT",
            Page::Fusion => "FUSION",
            Page::Photo => "PHOTO",
            Page::Three => "3D",
            Page::Color => "COLOR",
            Page::Fairlight => "FAIRLIGHT",
            Page::Depth => "DEPTH",
            Page::Deliver => "DELIVER",
        }
    }
    /// The panel arrangement of a page that is made of docked panels.
    pub fn workspace(self) -> Option<&'static str> {
        Some(match self {
            Page::Cut => "Page Cut",
            Page::Edit => "Page Edit",
            Page::Fusion => "Page Fusion",
            Page::Color => "Page Color",
            Page::Fairlight => "Page Fairlight",
            _ => return None,
        })
    }
    pub fn from_name(s: &str) -> Option<Page> {
        Self::ALL.into_iter().find(|p| p.label().eq_ignore_ascii_case(s.trim()))
    }
    /// Icon strokes in a 0..1 box (polylines).
    fn glyph(self) -> &'static [&'static [(f32, f32)]] {
        match self {
            Page::Media => &[&[(0.1, 0.2), (0.9, 0.2), (0.9, 0.8), (0.1, 0.8), (0.1, 0.2)], &[(0.1, 0.4), (0.9, 0.4)]],
            Page::Cut => &[&[(0.2, 0.2), (0.8, 0.8)], &[(0.8, 0.2), (0.2, 0.8)]],
            Page::Edit => &[&[(0.1, 0.25), (0.6, 0.25)], &[(0.1, 0.5), (0.9, 0.5)], &[(0.1, 0.75), (0.45, 0.75)]],
            Page::Fusion => &[&[(0.1, 0.15), (0.4, 0.15), (0.4, 0.45), (0.1, 0.45), (0.1, 0.15)], &[(0.6, 0.55), (0.9, 0.55), (0.9, 0.85), (0.6, 0.85), (0.6, 0.55)], &[(0.4, 0.3), (0.75, 0.3), (0.75, 0.55)]],
            Page::Photo => &[&[(0.1, 0.2), (0.9, 0.2), (0.9, 0.8), (0.1, 0.8), (0.1, 0.2)], &[(0.1, 0.68), (0.36, 0.44), (0.54, 0.6), (0.68, 0.48), (0.9, 0.7)], &[(0.66, 0.34), (0.7, 0.3), (0.74, 0.34), (0.7, 0.38), (0.66, 0.34)]],
            Page::Three => &[&[(0.5, 0.08), (0.9, 0.3), (0.9, 0.72), (0.5, 0.94), (0.1, 0.72), (0.1, 0.3), (0.5, 0.08)], &[(0.1, 0.3), (0.5, 0.52), (0.9, 0.3)], &[(0.5, 0.52), (0.5, 0.94)]],
            Page::Color => &[&[(0.5, 0.1), (0.78, 0.22), (0.9, 0.5), (0.78, 0.78), (0.5, 0.9), (0.22, 0.78), (0.1, 0.5), (0.22, 0.22), (0.5, 0.1)], &[(0.5, 0.1), (0.5, 0.9)]],
            Page::Fairlight => &[&[(0.15, 0.4), (0.15, 0.6)], &[(0.38, 0.25), (0.38, 0.75)], &[(0.62, 0.1), (0.62, 0.9)], &[(0.85, 0.3), (0.85, 0.7)]],
            Page::Depth => &[&[(0.5, 0.1), (0.9, 0.3), (0.5, 0.5), (0.1, 0.3), (0.5, 0.1)], &[(0.1, 0.5), (0.5, 0.7), (0.9, 0.5)], &[(0.1, 0.7), (0.5, 0.9), (0.9, 0.7)]],
            Page::Deliver => &[&[(0.5, 0.1), (0.5, 0.65)], &[(0.25, 0.42), (0.5, 0.67), (0.75, 0.42)], &[(0.15, 0.88), (0.85, 0.88)]],
        }
    }
}

/// The optional panels that are open: the toggles of the top bar. One set for the whole app, the
/// way Resolve remembers that the Inspector is open as you move between pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub media_pool: bool,
    pub effects: bool,
    pub sound: bool,
    pub inspector: bool,
    pub mixer: bool,
    pub source: bool,
    pub scopes: bool,
}

impl Default for Open {
    fn default() -> Self {
        Self { media_pool: true, effects: false, sound: false, inspector: true, mixer: false, source: false, scopes: true }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Toggle {
    MediaPool,
    Effects,
    Sound,
    Source,
    Mixer,
    Inspector,
    Scopes,
}

impl Toggle {
    fn label(self) -> &'static str {
        match self {
            Toggle::MediaPool => "Media Pool",
            Toggle::Effects => "Effects",
            Toggle::Sound => "Sound Library",
            Toggle::Source => "Dual Viewer",
            Toggle::Mixer => "Mixer",
            Toggle::Inspector => "Inspector",
            Toggle::Scopes => "Scopes",
        }
    }
    fn get(self, o: &mut Open) -> &mut bool {
        match self {
            Toggle::MediaPool => &mut o.media_pool,
            Toggle::Effects => &mut o.effects,
            Toggle::Sound => &mut o.sound,
            Toggle::Source => &mut o.source,
            Toggle::Mixer => &mut o.mixer,
            Toggle::Inspector => &mut o.inspector,
            Toggle::Scopes => &mut o.scopes,
        }
    }
}

/// The toggles a page offers: (left of the title, right of it), as in Resolve's UI toolbar.
fn toggles(page: Page) -> (&'static [Toggle], &'static [Toggle]) {
    use Toggle::*;
    match page {
        Page::Cut => (&[MediaPool, Effects, Sound], &[Inspector]),
        Page::Edit => (&[MediaPool, Effects, Sound], &[Source, Mixer, Inspector]),
        Page::Fusion => (&[Effects], &[Source, Inspector]),
        Page::Color => (&[Effects], &[Scopes, Inspector]),
        Page::Fairlight => (&[Effects, Sound], &[Mixer, Inspector]),
        _ => (&[], &[]),
    }
}

/// Lay the page out again from the open panels.
fn relayout(app: &mut SystmCutApp) {
    use crate::dock::PanelKind as P;
    let page = app.systm.page;
    let Some(ws) = page.workspace() else { return };
    let o = &app.systm.open;
    let (lt, _) = toggles(page);
    let left: Vec<P> = lt.iter().filter_map(|t| match t {
        Toggle::MediaPool if o.media_pool => Some(P::Project),
        Toggle::Effects if o.effects => Some(P::Effects),
        Toggle::Sound if o.sound => Some(P::SoundBank),
        _ => None,
    }).collect();
    let opts = crate::dock::PageOpts { left, inspector: o.inspector, mixer: o.mixer, source: o.source, scopes: o.scopes };
    app.ui.dock = crate::dock::page_layout(ws.trim_start_matches("Page "), &opts);
    app.ui.workspace = ws.to_string();
    app.ui.keys.maximized = None;
}

pub fn set_page(app: &mut SystmCutApp, page: Page) {
    app.systm.page = page;
    app.ui.mode = match page {
        Page::Media => Mode::Import,
        Page::Deliver => Mode::Export,
        _ => Mode::Edit,
    };
    relayout(app);
}

const BAR: Color32 = Color32::from_rgb(0x0a, 0x10, 0x13);
const PANEL: Color32 = Color32::from_rgb(0x0f, 0x17, 0x1b);
const LINE: Color32 = Color32::from_rgb(0x1c, 0x28, 0x2e);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x97, 0xa0);
const INK: Color32 = Color32::from_rgb(0xf1, 0xf5, 0xf6);

fn button(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect, id: &str, label: &str, selected: bool) -> egui::Response {
    let resp = ui.interact(r, egui::Id::new(("systm-shell", id)), Sense::click());
    let (l, s) = (label.to_string(), selected);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, s, l.clone()));
    app.auto.add(id, r, label);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A toggle of the top bar: a small square that fills when its panel is open, and the panel's name.
fn toggle(app: &mut SystmCutApp, ui: &mut egui::Ui, x: f32, cy: f32, t: Toggle, from_right: bool) -> f32 {
    let p = ui.painter().clone();
    let on = *t.get(&mut app.systm.open);
    let g = p.layout_no_wrap(t.label().to_string(), Tokens::ui(11.5), INK);
    let w = g.size().x + 34.0;
    let r = Rect::from_min_size(pos2(if from_right { x - w } else { x }, cy - 13.0), vec2(w, 26.0));
    let resp = button(app, ui, r, &format!("toggle.{}", t.label().to_lowercase().replace(' ', "-")), t.label(), on);
    let col = if on { super::ICE } else if resp.hovered() { INK } else { MUTED };
    if on {
        p.rect(r, 8.0, super::ICE.gamma_multiply(0.1), Stroke::new(1.0, super::ICE.gamma_multiply(0.28)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(r, 8.0, Color32::from_white_alpha(10));
    }
    let ic = Rect::from_center_size(pos2(r.min.x + 15.0, cy), vec2(11.0, 11.0));
    p.rect(ic, 3.0, if on { col.gamma_multiply(0.9) } else { Color32::TRANSPARENT }, Stroke::new(1.4, col), egui::StrokeKind::Inside);
    p.galley(pos2(r.min.x + 26.0, cy - g.size().y / 2.0), g, col);
    if resp.clicked() {
        let v = t.get(&mut app.systm.open);
        *v = !*v;
        relayout(app);
    }
    if from_right { r.min.x - 4.0 } else { r.max.x + 4.0 }
}

fn top_bar(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, BAR);
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, LINE));
    let drag = ui.interact(rect, egui::Id::new("systm-top-drag"), Sense::click_and_drag());
    if drag.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    }
    let cy = rect.center().y;
    let (lt, rt) = toggles(app.systm.page);
    let mut x = rect.min.x + if app.integrated_titlebar { 88.0 } else { 12.0 };
    // Workspace: the menu Resolve keeps its Scripts in
    {
        let g = p.layout_no_wrap("Workspace".to_string(), Tokens::ui(11.5), INK);
        let r = Rect::from_min_size(pos2(x, cy - 13.0), vec2(g.size().x + 30.0, 26.0));
        let open = app.systm.scripts.menu_open;
        let resp = button(app, ui, r, "menu.workspace", "Workspace", open);
        let col = if open { super::ICE } else if resp.hovered() { INK } else { MUTED };
        if open || resp.hovered() {
            p.rect_filled(r, 8.0, Color32::from_white_alpha(10));
        }
        p.galley(pos2(r.min.x + 9.0, cy - g.size().y / 2.0), g, col);
        p.add(egui::Shape::line(vec![pos2(r.max.x - 15.0, cy - 2.0), pos2(r.max.x - 11.0, cy + 2.0), pos2(r.max.x - 7.0, cy - 2.0)], Stroke::new(1.4, col)));
        if resp.clicked() {
            app.systm.scripts.menu_open = !open;
            app.systm.scripts.opened = ui.ctx().input(|i| i.time);
        }
        super::scripts::menu(app, ui.ctx(), pos2(r.min.x, rect.max.y + 4.0));
        x = r.max.x + 8.0;
    }
    for t in lt {
        x = toggle(app, ui, x, cy, *t, false);
    }
    // right: Team (every page), then the page's own
    let mut rx = rect.max.x - 10.0;
    {
        let on = app.systm.rail;
        let pending = app.systm.agents.pending();
        let busy = app.systm.agents.busy();
        let g = p.layout_no_wrap("Team".to_string(), Tokens::semibold(11.5), INK);
        let w = g.size().x + 58.0;
        let r = Rect::from_min_size(pos2(rx - w, cy - 13.0), vec2(w, 26.0));
        let resp = button(app, ui, r, "shell.team", "Team", on);
        p.rect(r, 13.0, if on { super::ICE.gamma_multiply(0.14) } else { PANEL }, Stroke::new(1.0, if on { super::ICE.gamma_multiply(0.45) } else { LINE }), egui::StrokeKind::Inside);
        let t = ui.ctx().input(|i| i.time) as f32;
        for (k, a) in app.systm.agents.agents.iter().enumerate() {
            let c = pos2(r.min.x + 14.0 + k as f32 * 13.0, cy);
            p.circle_filled(c, 5.5, if a.on { a.kind.color() } else { a.kind.color().gamma_multiply(0.3) });
            if a.busy() {
                p.circle_stroke(c, 7.5 + (t * 4.0 + k as f32).sin() * 1.0, Stroke::new(1.2, a.kind.color().gamma_multiply(0.7)));
            }
        }
        p.galley(pos2(r.min.x + 40.0, cy - g.size().y / 2.0), g, if on { super::ICE } else { MUTED });
        if pending > 0 {
            let b = pos2(r.max.x - 3.0, r.min.y + 3.0);
            p.circle_filled(b, 7.5, Color32::from_rgb(0xff, 0xd4, 0x8a));
            p.text(b, egui::Align2::CENTER_CENTER, pending.to_string(), Tokens::semibold(9.5), Color32::from_rgb(0x1a, 0x12, 0x00));
        }
        if busy {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
        }
        if resp.clicked() {
            app.systm.rail = !app.systm.rail;
        }
        rx = r.min.x - 8.0;
    }
    for t in rt.iter().rev() {
        rx = toggle(app, ui, rx, cy, *t, true);
    }
    // centre: the project, and whether it has unsaved changes (Resolve's "Edited")
    let name = app.session.project.name.clone();
    let g = p.layout_no_wrap(name, Tokens::semibold(13.5), INK);
    let edited = app.session.is_dirty();
    let eg = p.layout_no_wrap("Edited".to_string(), Tokens::ui(11.5), super::ICE);
    let total = g.size().x + if edited { eg.size().x + 12.0 } else { 0.0 };
    let left = (rect.center().x - total / 2.0).clamp(x + 8.0, (rx - total - 8.0).max(x + 8.0));
    if left + total < rx {
        let gw = g.size().x;
        p.galley(pos2(left, cy - g.size().y / 2.0), g, INK);
        if edited {
            p.galley(pos2(left + gw + 12.0, cy - eg.size().y / 2.0), eg, super::ICE);
        }
    }
    if let Some(what) = app.systm.depth.running() {
        let g = p.layout_no_wrap(format!("Depth  {what}"), Tokens::ui(11.0), super::ICE);
        let lx = left + total + 18.0;
        if lx + g.size().x < rx {
            p.galley(pos2(lx, cy - g.size().y / 2.0), g, super::ICE);
        }
    }
}

/// The page bar: the bottom edge of the window. Product on the left, the pages in the order of the
/// work in the centre, the current one under an Ice line.
fn page_bar(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, BAR);
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, LINE));
    let cy = rect.center().y;
    // product
    let mut x = rect.min.x + 16.0;
    let mark = Rect::from_center_size(pos2(x + 9.0, cy), vec2(18.0, 18.0));
    p.rect(mark, 5.0, super::ICE.gamma_multiply(0.12), Stroke::new(1.0, super::ICE.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    p.line_segment([mark.left_center() + vec2(4.0, 0.0), mark.right_center() - vec2(4.0, 0.0)], Stroke::new(1.6, super::ICE));
    x += 28.0;
    let g = p.layout_no_wrap("SYSTM".to_string(), Tokens::semibold(13.0), INK);
    let w = g.size().x;
    p.galley(pos2(x, cy - g.size().y / 2.0), g, INK);
    let g = p.layout_no_wrap("CUT".to_string(), Tokens::semibold(13.0), super::ICE);
    p.galley(pos2(x + w + 5.0, cy - g.size().y / 2.0), g, super::ICE);
    // pages
    let bw = 84.0_f32.min((rect.width() - 360.0) / Page::ALL.len() as f32).max(54.0);
    let x0 = rect.center().x - bw * Page::ALL.len() as f32 / 2.0;
    let mut go = None;
    for (i, page) in Page::ALL.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(x0 + bw * i as f32, rect.min.y), vec2(bw, rect.height()));
        let on = app.systm.page == page;
        let resp = button(app, ui, r, &format!("page.{}", page.label().to_lowercase()), page.label(), on);
        let col = if on { super::ICE } else if resp.hovered() { INK } else { MUTED };
        if on {
            // the line on top, with its light falling into the cell
            for k in 0..14 {
                let a = (1.0 - k as f32 / 14.0).powi(2) * 0.16;
                p.rect_filled(Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + k as f32 * 2.0), vec2(bw - 12.0, 2.0)), 0.0, super::ICE.gamma_multiply(a));
            }
            p.rect_filled(Rect::from_min_size(pos2(r.min.x + 10.0, r.min.y), vec2(bw - 20.0, 2.0)), 1.0, super::ICE);
        } else if resp.hovered() {
            p.rect_filled(r.shrink2(vec2(4.0, 6.0)), 9.0, Color32::from_white_alpha(8));
        }
        let ic = Rect::from_center_size(pos2(r.center().x, r.min.y + 19.0), vec2(16.0, 16.0));
        for line in page.glyph() {
            let pts: Vec<egui::Pos2> = line.iter().map(|(x, y)| pos2(ic.min.x + x * ic.width(), ic.min.y + y * ic.height())).collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
        }
        let g = p.layout_no_wrap(page.label().into(), Tokens::semibold(9.0), col);
        p.galley(pos2(r.center().x - g.size().x / 2.0, r.max.y - g.size().y - 7.0), g, col);
        if resp.clicked() {
            go = Some(page);
        }
    }
    // right: where the playhead is, the one thing that follows you through every page
    if let Some(seq) = app.session.active_sequence() {
        let n: usize = seq.video_tracks.iter().map(|t| t.items.len()).sum();
        let g = p.layout_no_wrap(format!("{n} clips"), Tokens::ui(11.0), MUTED);
        p.galley(pos2(rect.max.x - 16.0 - g.size().x, cy - g.size().y / 2.0), g, MUTED);
    }
    if let Some(page) = go {
        set_page(app, page);
    }
}

/// Draw the whole window body: top bar, the page, the team rail and the page bar.
pub fn frame(app: &mut SystmCutApp, ui: &mut egui::Ui, full: Rect, status_h: f32) {
    if !app.systm.started {
        app.systm.started = true;
        app.systm.rail = true;
        set_page(app, Page::Edit);
    }
    // Color and Fusion are about one clip: with nothing selected, it is the clip under the playhead
    if matches!(app.systm.page, Page::Color | Page::Fusion)
        && app.session.state.selection.is_empty()
        && let Some(c) = super::pages::current_clip(app)
    {
        app.session.state.selection = vec![c.id];
    }
    let top = Rect::from_min_size(full.min, vec2(full.width(), TOP_H));
    top_bar(app, ui, top);
    let bottom = full.max.y - status_h;
    let dock = Rect::from_min_max(pos2(full.min.x, bottom - DOCK_H), pos2(full.max.x, bottom));
    let rw = if app.systm.agents.wide { 680.0 } else { 440.0 };
    let rail_w = if app.systm.rail { rw + GAP * 2.0 } else { 0.0 };
    let body = Rect::from_min_max(pos2(full.min.x + 2.0, top.max.y + 2.0), pos2(full.max.x - 2.0 - rail_w, dock.min.y - 2.0));
    match app.systm.page {
        Page::Media => crate::panels::import_mode::show(app, ui, body),
        Page::Deliver => crate::panels::export_mode::show(app, ui, body),
        Page::Depth => {
            ui.painter().rect(body.shrink(4.0), 10.0, PANEL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            super::depth::show(app, ui, body.shrink(8.0));
        }
        // the Edit page is drawn whole by us: no docked panels
        Page::Edit => super::edit_page::show(app, ui, body),
        Page::Cut => super::cut_page::show(app, ui, body),
        Page::Photo => super::photo_page::show(app, ui, body),
        Page::Three => super::three_d::show(app, ui, body),
        _ => app.dock_area(ui, body),
    }
    if app.systm.rail {
        let rail = Rect::from_min_max(pos2(full.max.x - GAP - rw, top.max.y + GAP), pos2(full.max.x - GAP, dock.min.y - GAP));
        // Archie's panel: #0d1417, a 26 px radius, a deep shadow
        for k in 1..=6 {
            ui.painter().rect_filled(rail.expand(k as f32 * 3.0).translate(vec2(0.0, 6.0)), 26.0 + k as f32 * 3.0, Color32::from_black_alpha(14));
        }
        ui.painter().rect(rail, 26.0, Color32::from_rgb(0x0d, 0x14, 0x17), Stroke::new(1.0, Color32::from_rgb(0x2c, 0x39, 0x3f)), egui::StrokeKind::Inside);
        super::note_panel(app, crate::dock::PanelKind::Agents, rail);
        super::agents::show(app, ui, rail);
    }
    page_bar(app, ui, dock);
}
