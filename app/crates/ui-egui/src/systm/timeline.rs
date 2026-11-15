//! The SYSTM Cut timeline, drawn from nothing after DaVinci Resolve's Edit page
//! (research/resolve/ANALYSIS.md): a tool strip, the timecode large at the left of the ruler, a
//! header per track with its name and clip count, video clips in blue with a filmstrip, audio in
//! green with a waveform, one playhead.
//!
//! It edits through the engine's own commands (`timeline.select`, `timeline.move`,
//! `timeline.trim`, `timeline.razor`, `timeline.delete`), the same ones the agents use, so a
//! person and an agent can never disagree about what an edit does.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};
use systmcut_project::{ClipId, TrackItem};
use systmcut_time::{TICKS_PER_SECOND, Tick, TimeDisplay, format_time};

use super::ico;
use crate::SystmCutApp;
use crate::theme::Tokens;

const BG: Color32 = Color32::from_rgb(0x0c, 0x12, 0x15);
const LANE: Color32 = Color32::from_rgb(0x10, 0x18, 0x1c);
const LANE_ALT: Color32 = Color32::from_rgb(0x0e, 0x15, 0x19);
const HEAD: Color32 = Color32::from_rgb(0x13, 0x1c, 0x21);
const LINE: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x31);
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf0);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x97, 0xa0);
const FAINT: Color32 = Color32::from_rgb(0x52, 0x62, 0x6a);
// Resolve's clip colours, a shade deeper to sit on Carbon
const VIDEO: Color32 = Color32::from_rgb(0x2c, 0x6c, 0xa8);
const VIDEO_TOP: Color32 = Color32::from_rgb(0x1d, 0x4a, 0x76);
const AUDIO: Color32 = Color32::from_rgb(0x2a, 0x8a, 0x66);
const AUDIO_TOP: Color32 = Color32::from_rgb(0x1c, 0x60, 0x47);

const TOOLBAR_H: f32 = 38.0;
const RULER_H: f32 = 34.0;
const HEAD_W: f32 = 196.0;
const V_H: f32 = 66.0;
const A_H: f32 = 58.0;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Blade,
}

#[derive(Clone)]
enum Drag {
    /// Scrubbing the playhead.
    Scrub,
    /// Moving a clip: the clip, where in it the pointer took hold (s), its lane when taken.
    Move { clip: ClipId, grab: f64, lane: usize, moved: bool },
    /// Trimming an edge: the clip, the edge (false = in, true = out), the edge's time when taken.
    Trim { clip: ClipId, out: bool, origin: f64 },
}

pub struct TimelineState {
    /// Pixels per second, and the time at the left edge of the lanes.
    pps: f32,
    scroll: f64,
    fitted: bool,
    pub tool: Tool,
    pub snap: bool,
    drag: Option<Drag>,
    shown: f64,
    /// The Cut page's way: the playhead stays in the middle and the film moves under it.
    pub centred: bool,
}

impl TimelineState {
    /// The visible stretch of time: (start, seconds shown), as last drawn.
    pub fn window(&self) -> (f64, f64) {
        (self.scroll, self.shown)
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self { pps: 60.0, scroll: 0.0, fitted: false, tool: Tool::Select, snap: true, drag: None, centred: false, shown: 0.0 }
    }
}

struct Lane {
    id: Value,
    tag: String,
    name: String,
    video: bool,
    locked: bool,
    muted: bool,
    items: Vec<TrackItem>,
    rect: Rect,
}

fn secs(t: Tick) -> f64 {
    t.0 as f64 / TICKS_PER_SECOND as f64
}
fn ticks(s: f64) -> i64 {
    (s.max(0.0) * TICKS_PER_SECOND as f64).round() as i64
}

fn run(app: &mut SystmCutApp, command: &str, params: Value) {
    if let Err(e) = app.session.execute(command, params) {
        app.status(format!("{e}"));
    }
}

fn tool_button(app: &mut SystmCutApp, ui: &mut egui::Ui, c: Pos2, id: &str, tip: &str, icon: char, on: bool) -> bool {
    let r = Rect::from_center_size(c, vec2(30.0, 26.0));
    let resp = ui.interact(r, egui::Id::new(("systm-tl-tool", id)), Sense::click());
    let (t, s) = (tip.to_string(), on);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, s, t.clone()));
    app.auto.add(&format!("timeline.tool.{id}"), r, tip);
    let p = ui.painter();
    if on {
        p.rect(r, 7.0, super::ICE.gamma_multiply(0.14), Stroke::new(1.0, super::ICE.gamma_multiply(0.4)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(r, 7.0, Color32::from_white_alpha(12));
    }
    ico::paint(p, c, 15.0, icon, if on { super::ICE } else if resp.hovered() { INK } else { MUTED });
    resp.on_hover_text(tip).clicked()
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, BG);
    let Some(seq) = app.session.active_sequence() else {
        crate::dock::placeholder(ui, rect, &app.tokens, "Open or create a timeline");
        return;
    };
    let (rate, drop) = (seq.settings.frame_rate, seq.settings.drop_frame);
    // lanes, top to bottom: the highest video track first, then the audio tracks
    let mut lanes: Vec<Lane> = Vec::new();
    let nv = seq.video_tracks.len();
    for (i, t) in seq.video_tracks.iter().enumerate().rev() {
        lanes.push(Lane { id: json!(t.id), tag: format!("V{}", i + 1), name: if t.name.is_empty() { format!("Video {}", i + 1) } else { t.name.clone() }, video: true, locked: t.locked, muted: !t.enabled, items: t.items.clone(), rect: Rect::NOTHING });
    }
    for (i, t) in seq.audio_tracks.iter().enumerate() {
        lanes.push(Lane { id: json!(t.id), tag: format!("A{}", i + 1), name: if t.name.is_empty() { format!("Audio {}", i + 1) } else { t.name.clone() }, video: false, locked: t.locked, muted: t.muted, items: t.items.clone(), rect: Rect::NOTHING });
    }
    let end = lanes.iter().flat_map(|l| &l.items).map(|i| secs(i.start + i.duration)).fold(0.0_f64, f64::max);
    let ph = secs(app.session.playhead());
    let selection: Vec<ClipId> = app.session.state.selection.clone();
    let playing = app.playback.playing;
    let _ = nv;

    // ---- geometry
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TOOLBAR_H));
    let ruler = Rect::from_min_max(pos2(rect.min.x + HEAD_W, bar.max.y), pos2(rect.max.x, bar.max.y + RULER_H));
    let area = Rect::from_min_max(pos2(ruler.min.x, ruler.max.y), rect.max);
    let st = &mut app.systm.timeline;
    if !st.fitted && end > 0.0 && area.width() > 50.0 {
        st.pps = (area.width() / (end as f32 * 1.12)).clamp(4.0, 400.0);
        st.fitted = true;
    }
    let (pps, mut scroll) = (st.pps, st.scroll);
    st.shown = area.width() as f64 / pps as f64;
    if st.centred {
        scroll = ph - st.shown / 2.0;
        st.scroll = scroll;
    } else if scroll < 0.0 {
        // back from the Cut page's centred view
        scroll = 0.0;
        st.scroll = 0.0;
    } else if playing {
        // follow the playhead: page forward when it leaves the view
        let view = area.width() as f64 / pps as f64;
        if ph > scroll + view * 0.96 || ph < scroll {
            scroll = (ph - view * 0.1).max(0.0);
            st.scroll = scroll;
        }
    }
    let x_of = |s: f64| area.min.x + ((s - scroll) * pps as f64) as f32;
    let t_of = |x: f32| scroll + ((x - area.min.x) / pps) as f64;
    let mut y = area.min.y;
    let mut split_y = None;
    for l in &mut lanes {
        if !l.video && split_y.is_none() {
            split_y = Some(y);
            y += 3.0;
        }
        let h = if l.video { V_H } else { A_H };
        l.rect = Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), h));
        y += h;
    }

    // ---- tool strip
    p.rect_filled(bar, 0.0, HEAD);
    p.line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, LINE));
    let cy = bar.center().y;
    let mut x = bar.center().x - 120.0;
    let (tool, snap) = (app.systm.timeline.tool, app.systm.timeline.snap);
    if tool_button(app, ui, pos2(x, cy), "select", "Selection mode (A)", ico::MOUSE_POINTER_2, tool == Tool::Select) {
        app.systm.timeline.tool = Tool::Select;
    }
    x += 36.0;
    if tool_button(app, ui, pos2(x, cy), "blade", "Blade: click a clip to cut it (B)", ico::SCISSORS, tool == Tool::Blade) {
        app.systm.timeline.tool = Tool::Blade;
    }
    x += 50.0;
    if tool_button(app, ui, pos2(x, cy), "snap", "Snapping (N)", ico::MAGNET, snap) {
        app.systm.timeline.snap = !snap;
    }
    x += 36.0;
    if tool_button(app, ui, pos2(x, cy), "link", "Linked selection", ico::LINK, app.session.state.linked_selection) {
        run(app, "sequence.linkedSelection", json!({}));
    }
    x += 50.0;
    if tool_button(app, ui, pos2(x, cy), "delete", "Delete the selected clips (Backspace)", ico::TRASH_2, false) && !selection.is_empty() {
        run(app, "timeline.delete", json!({}));
    }
    x += 36.0;
    if tool_button(app, ui, pos2(x, cy), "ripple", "Ripple delete: delete and close the gap (Shift+Backspace)", ico::ALIGN_HORIZONTAL_SPACE_AROUND, false) && !selection.is_empty() {
        run(app, "timeline.delete", json!({"ripple": true}));
    }
    // zoom, on the right
    let zx = bar.max.x - 150.0;
    if tool_button(app, ui, pos2(zx, cy), "zoom-out", "Zoom out", ico::ZOOM_OUT, false) {
        app.systm.timeline.pps = (pps / 1.3).max(2.0);
    }
    let track = Rect::from_min_max(pos2(zx + 22.0, cy - 2.0), pos2(zx + 102.0, cy + 2.0));
    p.rect_filled(track, 2.0, LINE);
    let k = ((pps.ln() - 2.0_f32.ln()) / (600.0_f32.ln() - 2.0_f32.ln())).clamp(0.0, 1.0);
    let zr = ui.interact(track.expand2(vec2(4.0, 9.0)), egui::Id::new("systm-tl-zoom"), Sense::click_and_drag());
    p.circle_filled(pos2(track.min.x + track.width() * k, cy), 5.5, if zr.hovered() || zr.dragged() { INK } else { MUTED });
    if let Some(pt) = zr.interact_pointer_pos().filter(|_| zr.dragged() || zr.clicked()) {
        let k = ((pt.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        app.systm.timeline.pps = (2.0_f32.ln() + k * (600.0_f32.ln() - 2.0_f32.ln())).exp();
    }
    if tool_button(app, ui, pos2(zx + 124.0, cy), "zoom-in", "Zoom in", ico::ZOOM_IN, false) {
        app.systm.timeline.pps = (pps * 1.3).min(600.0);
    }
    if tool_button(app, ui, pos2(bar.min.x + 24.0, cy), "fit", "Fit the whole timeline", ico::MAXIMIZE_2, false) {
        app.systm.timeline.fitted = false;
        app.systm.timeline.scroll = 0.0;
    }

    // ---- the timecode, large, at the left of the ruler
    let corner = Rect::from_min_max(pos2(rect.min.x, bar.max.y), pos2(area.min.x, ruler.max.y));
    p.rect_filled(corner, 0.0, HEAD);
    let tc = format_time(Tick(ticks(ph)), rate, drop, TimeDisplay::Timecode, 48000);
    p.text(corner.center(), egui::Align2::CENTER_CENTER, tc, egui::FontId::monospace(19.0), INK);

    // ---- ruler
    let pr = ui.painter_at(Rect::from_min_max(ruler.min, rect.max));
    pr.rect_filled(ruler, 0.0, HEAD);
    let step = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0].into_iter().find(|s| s * pps as f64 >= 120.0).unwrap_or(1200.0);
    let mut t = (scroll / step).floor() * step;
    while x_of(t) < ruler.max.x {
        let tx = x_of(t);
        if tx >= ruler.min.x - 1.0 && t >= -1e-6 {
            pr.line_segment([pos2(tx, ruler.max.y - 12.0), pos2(tx, ruler.max.y)], Stroke::new(1.0, FAINT));
            pr.text(pos2(tx + 5.0, ruler.min.y + 11.0), egui::Align2::LEFT_CENTER, format_time(Tick(ticks(t)), rate, drop, TimeDisplay::Timecode, 48000), egui::FontId::monospace(10.5), MUTED);
        }
        for m in 1..5 {
            let mx = x_of(t + step * m as f64 / 5.0);
            if mx >= ruler.min.x && mx < ruler.max.x {
                pr.line_segment([pos2(mx, ruler.max.y - 5.0), pos2(mx, ruler.max.y)], Stroke::new(1.0, LINE));
            }
        }
        t += step;
    }
    pr.line_segment([pos2(rect.min.x, ruler.max.y), pos2(rect.max.x, ruler.max.y)], Stroke::new(1.0, LINE));

    // ---- lanes and headers
    let pa = ui.painter_at(area);
    let hover = ui.input(|i| i.pointer.hover_pos());
    let mut hot: Option<(ClipId, usize, Rect, u8)> = None; // clip, lane, rect, zone (0 body, 1 in edge, 2 out edge)
    for (li, l) in lanes.iter().enumerate() {
        pa.rect_filled(l.rect, 0.0, if li % 2 == 0 { LANE } else { LANE_ALT });
        pa.line_segment([l.rect.left_bottom(), l.rect.right_bottom()], Stroke::new(1.0, LINE));
        // header: tag, state, name and how many clips, as Resolve writes it
        let h = Rect::from_min_max(pos2(rect.min.x, l.rect.min.y), pos2(area.min.x, l.rect.max.y));
        p.rect_filled(h, 0.0, HEAD);
        p.line_segment([h.left_bottom(), h.right_bottom()], Stroke::new(1.0, LINE));
        let accent = if l.video { VIDEO } else { AUDIO };
        p.rect_filled(Rect::from_min_size(h.min, vec2(3.0, h.height())), 0.0, accent);
        let tag = Rect::from_min_size(pos2(h.min.x + 12.0, h.min.y + 9.0), vec2(30.0, 19.0));
        p.rect(tag, 4.0, accent.gamma_multiply(0.28), Stroke::new(1.0, accent.gamma_multiply(0.9)), egui::StrokeKind::Inside);
        p.text(tag.center(), egui::Align2::CENTER_CENTER, &l.tag, Tokens::semibold(10.5), INK);
        ico::paint(&p, pos2(tag.max.x + 18.0, tag.center().y), 13.0, if l.locked { ico::LOCK } else { ico::LOCK_OPEN }, if l.locked { super::ICE } else { FAINT });
        let state = if l.video { if l.muted { ico::EYE_OFF } else { ico::EYE } } else if l.muted { ico::VOLUME_X } else { ico::VOLUME_2 };
        ico::paint(&p, pos2(tag.max.x + 42.0, tag.center().y), 13.0, state, if l.muted { super::ICE } else { FAINT });
        let name: String = l.name.chars().take(22).collect();
        p.text(pos2(h.min.x + 12.0, h.min.y + 40.0), egui::Align2::LEFT_CENTER, name, Tokens::semibold(11.5), INK);
        p.text(pos2(h.max.x - 10.0, h.min.y + 40.0), egui::Align2::RIGHT_CENTER, format!("{} Clip{}", l.items.len(), if l.items.len() == 1 { "" } else { "s" }), Tokens::ui(10.5), FAINT);

        for it in &l.items {
            let (a, b) = (x_of(secs(it.start)), x_of(secs(it.start + it.duration)));
            if b < area.min.x - 2.0 || a > area.max.x + 2.0 {
                continue;
            }
            let r = Rect::from_min_max(pos2(a, l.rect.min.y + 3.0), pos2(b.max(a + 2.0), l.rect.max.y - 3.0));
            let sel = selection.contains(&it.id);
            let (fill, top) = if l.video { (VIDEO, VIDEO_TOP) } else { (AUDIO, AUDIO_TOP) };
            let dim = if it.enabled { 1.0 } else { 0.4 };
            pa.rect_filled(r, 5.0, fill.gamma_multiply(dim));
            let body = Rect::from_min_max(pos2(r.min.x, r.min.y + 17.0), r.max);
            if l.video && r.width() > 26.0 {
                // filmstrip: frames of the clip, each where it plays
                let tw = body.height() * 16.0 / 9.0;
                let n = ((body.width() / tw).ceil() as usize).clamp(1, 14);
                let clip = Rect::from_min_max(pos2(body.min.x.max(area.min.x), body.min.y), pos2(body.max.x.min(area.max.x), body.max.y - 1.0));
                let pc = ui.painter_at(clip);
                for k in 0..n {
                    let tx = body.min.x + k as f32 * tw;
                    if tx > area.max.x || tx + tw < area.min.x {
                        continue;
                    }
                    let at = Tick(it.source_in.0 + ticks((k as f32 * tw / pps) as f64));
                    if let Some((tex, _)) = app.thumbnail(ui.ctx(), it.item, at, 192) {
                        pc.image(tex, Rect::from_min_size(pos2(tx, body.min.y), vec2(tw, body.height() - 1.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha((255.0 * dim) as u8));
                    }
                }
            } else if !l.video && r.width() > 6.0 {
                crate::panels::timeline::waveforms::draw_waveform(app, &pa, body.shrink2(vec2(0.0, 2.0)), it, Color32::from_rgb(0x9c, 0xf0, 0xcc).gamma_multiply(0.75));
            }
            // the name strip
            let strip = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 17.0));
            pa.rect(strip, egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, top.gamma_multiply(dim), Stroke::NONE, egui::StrokeKind::Inside);
            if r.width() > 34.0 {
                let pn = ui.painter_at(strip.intersect(area).shrink2(vec2(5.0, 0.0)));
                pn.text(pos2(strip.min.x.max(area.min.x) + 6.0, strip.center().y), egui::Align2::LEFT_CENTER, &it.name, Tokens::ui(10.5), INK);
                if !it.effects.is_empty() && r.width() > 90.0 {
                    ico::paint(&pa, pos2(strip.max.x - 10.0, strip.center().y), 10.0, ico::SPARKLES, Color32::from_rgb(0xff, 0xd4, 0x8a));
                }
            }
            pa.rect_stroke(r, 5.0, Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { super::ICE } else { Color32::from_black_alpha(120) }), egui::StrokeKind::Inside);
            app.auto.add(&format!("timeline.clip.{}", json!(it.id)), r.intersect(area), &it.name);
            if let Some(hp) = hover
                && r.contains(hp)
                && area.contains(hp)
            {
                let zone = if hp.x - r.min.x < 6.0 && r.width() > 18.0 { 1 } else if r.max.x - hp.x < 6.0 && r.width() > 18.0 { 2 } else { 0 };
                hot = Some((it.id, li, r, zone));
            }
        }
    }
    if let Some(sy) = split_y {
        pa.rect_filled(Rect::from_min_size(pos2(rect.min.x, sy), vec2(rect.width(), 3.0)), 0.0, Color32::from_black_alpha(200));
    }

    // ---- input
    let resp = ui.interact(Rect::from_min_max(ruler.min, rect.max), egui::Id::new("systm-timeline"), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Group, true, "Timeline"));
    let pointer = resp.interact_pointer_pos().or(hover);
    let blade = app.systm.timeline.tool == Tool::Blade;
    // snap a time to the playhead and to every clip edge within 8 px
    let snap_to = |s: f64, skip: Option<ClipId>| -> f64 {
        if !snap {
            return s;
        }
        let mut best = (8.0 / pps as f64, s);
        let mut try_at = |c: f64| {
            let d = (c - s).abs();
            if d < best.0 {
                best = (d, c);
            }
        };
        try_at(ph);
        try_at(0.0);
        for it in lanes.iter().flat_map(|l| &l.items).filter(|i| Some(i.id) != skip) {
            try_at(secs(it.start));
            try_at(secs(it.start + it.duration));
        }
        best.1
    };
    if resp.hovered() {
        let (scroll_delta, zoom, mods) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
        if (zoom - 1.0).abs() > 0.001 || (mods.command && scroll_delta.y != 0.0) {
            // zoom about the pointer
            let f = if (zoom - 1.0).abs() > 0.001 { zoom } else { (scroll_delta.y * 0.01).exp() };
            let anchor = pointer.map_or(ph, |pt| t_of(pt.x));
            let npps = (pps * f).clamp(2.0, 600.0);
            let px = ((anchor - scroll) * pps as f64) as f32;
            app.systm.timeline.pps = npps;
            app.systm.timeline.scroll = (anchor - (px / npps) as f64).max(0.0);
        } else if scroll_delta.x != 0.0 || scroll_delta.y != 0.0 {
            let d = if scroll_delta.x != 0.0 { scroll_delta.x } else { scroll_delta.y };
            if app.systm.timeline.centred {
                // the film moves under the fixed playhead: scrolling is scrubbing
                app.session.set_playhead(Tick(ticks(ph - (d / pps) as f64)));
            } else {
                app.systm.timeline.scroll = (scroll - (d / pps) as f64).max(0.0);
            }
        }
        if let Some((_, _, _, zone)) = hot {
            ui.ctx().set_cursor_icon(if blade { egui::CursorIcon::Crosshair } else if zone > 0 { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::Grab });
        }
        let (del, shift, key_a, key_b, key_n) = ui.input(|i| (i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete), i.modifiers.shift, i.key_pressed(egui::Key::A), i.key_pressed(egui::Key::B), i.key_pressed(egui::Key::N)));
        if del && !selection.is_empty() {
            run(app, "timeline.delete", json!({"ripple": shift}));
        }
        if key_a {
            app.systm.timeline.tool = Tool::Select;
        }
        if key_b {
            app.systm.timeline.tool = Tool::Blade;
        }
        if key_n {
            app.systm.timeline.snap = !snap;
        }
    }
    if resp.drag_started() || resp.clicked() {
        if let Some(pt) = pointer {
            let in_ruler = pt.y < ruler.max.y;
            let shift = ui.input(|i| i.modifiers.shift);
            match hot {
                Some((clip, _, _, _)) if !in_ruler && blade => {
                    run(app, "timeline.razor", json!({"time": ticks(snap_to(t_of(pt.x), None)), "clip": clip}));
                }
                Some((clip, lane, r, zone)) if !in_ruler => {
                    run(app, "timeline.select", json!({"clips": [clip], "add": shift, "toggle": shift}));
                    if resp.drag_started() {
                        app.systm.timeline.drag = Some(match zone {
                            1 => Drag::Trim { clip, out: false, origin: t_of(r.min.x) },
                            2 => Drag::Trim { clip, out: true, origin: t_of(r.max.x) },
                            _ => Drag::Move { clip, grab: t_of(pt.x) - t_of(r.min.x), lane, moved: false },
                        });
                    }
                }
                _ => {
                    if !in_ruler && !shift {
                        run(app, "timeline.select", json!({"clips": []}));
                    }
                    app.session.set_playhead(Tick(ticks(t_of(pt.x))));
                    if resp.drag_started() {
                        app.systm.timeline.drag = Some(Drag::Scrub);
                    }
                }
            }
        }
    }
    let drag = app.systm.timeline.drag.clone();
    if let (Some(d), Some(pt)) = (&drag, pointer) {
        let over_lane = lanes.iter().position(|l| pt.y >= l.rect.min.y && pt.y < l.rect.max.y);
        match d {
            Drag::Scrub => {
                app.session.set_playhead(Tick(ticks(t_of(pt.x.clamp(area.min.x, area.max.x)))));
            }
            Drag::Move { clip, grab, lane, .. } => {
                let start = snap_to(t_of(pt.x) - grab, Some(*clip)).max(0.0);
                let Some(src) = lanes.iter().flat_map(|l| &l.items).find(|i| i.id == *clip) else { return };
                // a clip stays among its own kind
                let target = over_lane.filter(|&t| lanes[t].video == lanes[*lane].video).unwrap_or(*lane);
                let ghost = Rect::from_min_max(pos2(x_of(start), lanes[target].rect.min.y + 3.0), pos2(x_of(start + secs(src.duration)), lanes[target].rect.max.y - 3.0));
                pa.rect(ghost, 5.0, super::ICE.gamma_multiply(0.16), Stroke::new(1.5, super::ICE), egui::StrokeKind::Inside);
                if resp.drag_stopped() {
                    if (start - secs(src.start)).abs() * pps as f64 > 1.5 || target != *lane {
                        run(app, "timeline.move", json!({"moves": [{"clip": clip, "track": lanes[target].id, "time": ticks(start)}], "insert": false}));
                    }
                } else {
                    app.systm.timeline.drag = Some(Drag::Move { clip: *clip, grab: *grab, lane: *lane, moved: true });
                }
            }
            Drag::Trim { clip, out, origin } => {
                let to = snap_to(t_of(pt.x), Some(*clip));
                pa.line_segment([pos2(x_of(to), area.min.y), pos2(x_of(to), area.max.y)], Stroke::new(1.5, Color32::from_rgb(0xff, 0xd4, 0x8a)));
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                if resp.drag_stopped() && (to - origin).abs() * pps as f64 > 1.0 {
                    run(app, "timeline.trim", json!({"clip": clip, "edge": if *out { "out" } else { "in" }, "mode": "regular", "delta": ticks(to.max(0.0)) - ticks(*origin)}));
                }
            }
        }
    }
    if resp.drag_stopped() || (!resp.dragged() && !ui.input(|i| i.pointer.any_down())) {
        app.systm.timeline.drag = None;
    }

    // ---- the playhead, over everything
    let px = x_of(ph);
    if px >= area.min.x - 1.0 && px <= area.max.x {
        let pp = ui.painter_at(Rect::from_min_max(ruler.min, rect.max));
        pp.line_segment([pos2(px, ruler.min.y + 14.0), pos2(px, rect.max.y)], Stroke::new(1.5, super::ICE));
        pp.add(egui::Shape::convex_polygon(vec![pos2(px - 6.5, ruler.min.y + 6.0), pos2(px + 6.5, ruler.min.y + 6.0), pos2(px + 6.5, ruler.min.y + 14.0), pos2(px, ruler.min.y + 21.0), pos2(px - 6.5, ruler.min.y + 14.0)], super::ICE, Stroke::NONE));
    }
    if playing {
        ui.ctx().request_repaint();
    }
}
