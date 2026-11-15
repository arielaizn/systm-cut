//! The Cut page, after DaVinci Resolve's (research/resolve/cut.png): the Media Pool and one viewer
//! on top, a strip of fast edit actions, then two timelines at once: the whole film as a thin
//! strip, and under it the working timeline with the playhead fixed in the middle while the film
//! moves under it. It edits the same timeline as the Edit page.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};
use systmcut_time::{TICKS_PER_SECOND, Tick};

use super::ico;
use super::shell::Page;
use crate::SystmCutApp;
use crate::theme::Tokens;

const BG: Color32 = Color32::from_rgb(0x0c, 0x12, 0x15);
const STRIP: Color32 = Color32::from_rgb(0x13, 0x1c, 0x21);
const LANE: Color32 = Color32::from_rgb(0x0e, 0x15, 0x19);
const LINE: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x31);
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf0);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x9a, 0xa2);
const VIDEO: Color32 = Color32::from_rgb(0x2c, 0x6c, 0xa8);
const AUDIO: Color32 = Color32::from_rgb(0x2a, 0x8a, 0x66);

fn secs(t: Tick) -> f64 {
    t.0 as f64 / TICKS_PER_SECOND as f64
}

fn run(app: &mut SystmCutApp, command: &str, params: Value) {
    if let Err(e) = app.session.execute(command, params) {
        app.status(format!("{e}"));
    }
}

/// A labelled action of the strip: an icon over its name.
fn action(app: &mut SystmCutApp, ui: &mut egui::Ui, c: Pos2, id: &str, label: &str, tip: &str, icon: char, enabled: bool) -> bool {
    let r = Rect::from_center_size(c, vec2(92.0, 40.0));
    let resp = ui.interact(r, egui::Id::new(("systm-cut", id)), if enabled { Sense::click() } else { Sense::hover() });
    let t = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, t.clone()));
    app.auto.add(&format!("cut.{id}"), r, label);
    let hot = resp.hovered() && enabled;
    if hot {
        ui.painter().rect_filled(r, 8.0, Color32::from_white_alpha(12));
    }
    let col = if !enabled { MUTED.gamma_multiply(0.4) } else if hot { super::ICE } else { MUTED };
    ico::paint(ui.painter(), pos2(c.x, c.y - 7.0), 15.0, icon, col);
    ui.painter().text(pos2(c.x, c.y + 11.0), egui::Align2::CENTER_CENTER, label, Tokens::ui(10.0), col);
    resp.on_hover_text(tip).clicked()
}

/// The whole film as one strip: every clip a block, the working timeline's window marked on it.
fn overview(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect) {
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, LANE);
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, LINE));
    let Some(seq) = app.session.active_sequence() else { return };
    let total = secs(seq.duration()).max(0.001);
    let inner = r.shrink2(vec2(10.0, 6.0));
    let x_of = |s: f64| inner.min.x + (s / total) as f32 * inner.width();
    let nv = seq.video_tracks.len().max(1);
    let na = seq.audio_tracks.len().max(1);
    let row = (inner.height() / (nv + na) as f32).min(12.0);
    for (i, t) in seq.video_tracks.iter().rev().enumerate() {
        let y = inner.min.y + i as f32 * row;
        for it in &t.items {
            p.rect_filled(Rect::from_min_max(pos2(x_of(secs(it.start)), y + 1.0), pos2(x_of(secs(it.start + it.duration)) - 1.0, y + row - 1.0)), 2.0, VIDEO);
        }
    }
    for (i, t) in seq.audio_tracks.iter().enumerate() {
        let y = inner.min.y + (nv + i) as f32 * row;
        for it in &t.items {
            p.rect_filled(Rect::from_min_max(pos2(x_of(secs(it.start)), y + 1.0), pos2(x_of(secs(it.start + it.duration)) - 1.0, y + row - 1.0)), 2.0, AUDIO);
        }
    }
    // the window the working timeline shows, and the playhead
    let (start, shown) = app.systm.timeline.window();
    let w = Rect::from_min_max(pos2(x_of(start.max(0.0)).max(inner.min.x), r.min.y + 2.0), pos2(x_of((start + shown).min(total)).min(inner.max.x), r.max.y - 2.0));
    p.rect(w, 4.0, Color32::from_white_alpha(14), Stroke::new(1.0, Color32::from_white_alpha(90)), egui::StrokeKind::Inside);
    let ph = secs(app.session.playhead());
    p.line_segment([pos2(x_of(ph), r.min.y), pos2(x_of(ph), r.max.y)], Stroke::new(1.5, super::ICE));
    // click or drag anywhere on the strip to go there
    let resp = ui.interact(r, egui::Id::new("systm-cut-overview"), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Whole timeline"));
    if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let s = ((pt.x - inner.min.x) / inner.width()).clamp(0.0, 1.0) as f64 * total;
        app.session.set_playhead(Tick((s * TICKS_PER_SECOND as f64) as i64));
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, body: Rect) {
    ui.painter().rect_filled(body, 0.0, BG);
    let o = app.systm.open.clone();
    let top_h = (body.height() * 0.5).round();
    let top = Rect::from_min_size(body.min, vec2(body.width(), top_h));
    // top: pool (and what else is open on the left) | viewer | inspector
    let left_n = [o.media_pool, o.sound, o.effects].iter().filter(|x| **x).count() as f32;
    let left_w = if left_n > 0.0 { (body.width() * 0.4).clamp(280.0, 620.0) } else { 0.0 };
    let right_w = if o.inspector { (body.width() * 0.22).clamp(270.0, 340.0) } else { 0.0 };
    if left_n > 0.0 {
        let cw = left_w / left_n;
        let mut x = top.min.x;
        if o.media_pool {
            super::edit_page::pool(app, ui, Rect::from_min_size(pos2(x, top.min.y), vec2(cw, top.height())));
            x += cw;
        }
        if o.effects {
            super::fx_library::show(app, ui, Rect::from_min_size(pos2(x, top.min.y), vec2(cw, top.height())), Page::Cut);
            x += cw;
        }
        if o.sound {
            let r = Rect::from_min_size(pos2(x, top.min.y), vec2(cw, top.height()));
            ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x10, 0x18, 0x1c));
            ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            super::bank::show(app, ui, r);
        }
    }
    super::edit_page::viewer(app, ui, Rect::from_min_max(pos2(top.min.x + left_w, top.min.y), pos2(top.max.x - right_w, top.max.y)));
    if right_w > 0.0 {
        super::inspector::show(app, ui, Rect::from_min_max(pos2(top.max.x - right_w, top.min.y), top.max), Page::Cut);
    }
    // the action strip
    let strip = Rect::from_min_size(pos2(body.min.x, top.max.y), vec2(body.width(), 52.0));
    ui.painter().rect_filled(strip, 0.0, STRIP);
    ui.painter().line_segment([strip.left_top(), strip.right_top()], Stroke::new(1.0, Color32::BLACK));
    ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, LINE));
    let picked = app.systm.edit.picked;
    let has_sel = !app.session.state.selection.is_empty();
    let ph = app.session.playhead();
    let end = app.session.active_sequence().map(|s| s.duration()).unwrap_or(Tick::ZERO);
    let cy = strip.center().y;
    let mut x = strip.center().x - 3.0 * 96.0;
    let item = picked.map(|id| json!(id));
    let need = "Pick a clip in the Media Pool first";
    if action(app, ui, pos2(x, cy), "insert", "Smart Insert", if item.is_some() { "Insert the picked clip at the playhead, pushing what follows" } else { need }, ico::STEP_FORWARD, item.is_some()) {
        run(app, "timeline.place", json!({"item": item, "time": ph.0, "insert": true}));
    }
    x += 96.0;
    if action(app, ui, pos2(x, cy), "append", "Append", if item.is_some() { "Add the picked clip at the end of the timeline" } else { need }, ico::SKIP_FORWARD, item.is_some()) {
        run(app, "timeline.place", json!({"item": item, "time": end.0, "insert": false}));
    }
    x += 96.0;
    if action(app, ui, pos2(x, cy), "ontop", "Place on Top", if item.is_some() { "Put the picked clip on a track above, at the playhead" } else { need }, ico::LAYERS, item.is_some()) {
        run(app, "timeline.place", json!({"item": item, "time": ph.0, "track": "new", "insert": false}));
    }
    x += 96.0;
    if action(app, ui, pos2(x, cy), "split", "Split", "Cut every clip under the playhead", ico::SCISSORS, true) {
        run(app, "sequence.addEditAllTracks", json!({}));
    }
    x += 96.0;
    if action(app, ui, pos2(x, cy), "ripple", "Ripple Delete", "Delete the selected clips and close the gap", ico::TRASH_2, has_sel) {
        run(app, "timeline.delete", json!({"ripple": true}));
    }
    x += 96.0;
    if action(app, ui, pos2(x, cy), "closeup", "Close Up", "Zoom the selected clip to 150% (a punch-in)", ico::ZOOM_IN, has_sel) {
        let clips = app.session.state.selection.clone();
        for c in clips {
            run(app, "effects.setParam", json!({"clip": c, "effect": "motion", "param": "scale", "value": 150.0}));
        }
    }
    x += 96.0;
    let _ = x;
    // the two timelines
    let over = Rect::from_min_size(pos2(body.min.x, strip.max.y), vec2(body.width(), 64.0));
    overview(app, ui, over);
    let tl = Rect::from_min_max(pos2(body.min.x, over.max.y), body.max);
    super::note_panel(app, crate::dock::PanelKind::Timeline, tl);
    app.systm.timeline.centred = true;
    super::timeline::show(app, ui, tl);
    app.systm.timeline.centred = false;
}
