//! The Edit page, drawn whole after DaVinci Resolve's (research/resolve/edit.png and the `e-*.png`
//! close-ups): Media Pool on the left, one viewer in the middle, the Inspector on the right, and
//! the timeline with its tool strip across the bottom. Flat regions divided by hairlines, a 30 px
//! title strip on each, nothing docked, nothing tabbed.
//!
//! Nothing here comes from the old panel set: the pool, the viewer's frame and transport and the
//! Inspector are this file; the timeline is `systm::timeline`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};
use systmcut_project::{BinEntry, BinId, ItemId, ItemKind};
use systmcut_time::{TICKS_PER_SECOND, Tick, TimeDisplay, format_time};

use super::ico;
use crate::SystmCutApp;
use crate::theme::Tokens;

const BG: Color32 = Color32::from_rgb(0x0c, 0x12, 0x15);
const SURFACE: Color32 = Color32::from_rgb(0x10, 0x18, 0x1c);
const STRIP: Color32 = Color32::from_rgb(0x13, 0x1c, 0x21);
const WELL: Color32 = Color32::from_rgb(0x07, 0x0b, 0x0d);
const LINE: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x31);
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf0);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x9a, 0xa2);
const FAINT: Color32 = Color32::from_rgb(0x52, 0x62, 0x6a);
const STRIP_H: f32 = 30.0;

#[derive(Default)]
pub struct EditState {
    /// The bin the pool shows: the path of bin ids down from the root.
    bin: Vec<BinId>,
    pub(crate) picked: Option<ItemId>,
    search: String,
    searching: bool,
}

fn secs(t: Tick) -> f64 {
    t.0 as f64 / TICKS_PER_SECOND as f64
}

fn run(app: &mut SystmCutApp, command: &str, params: Value) -> bool {
    match app.session.execute(command, params) {
        Ok(_) => true,
        Err(e) => {
            app.status(format!("{e}"));
            false
        }
    }
}

/// A region: its surface, a title strip, hairlines. Returns (strip, body).
fn region(ui: &egui::Ui, r: Rect) -> (Rect, Rect) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, SURFACE);
    let strip = Rect::from_min_size(r.min, vec2(r.width(), STRIP_H));
    p.rect_filled(strip, 0.0, STRIP);
    p.line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, LINE));
    p.rect_stroke(r, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    (strip, Rect::from_min_max(pos2(r.min.x, strip.max.y), r.max))
}

fn icon_button(app: &mut SystmCutApp, ui: &mut egui::Ui, c: Pos2, size: f32, id: &str, tip: &str, icon: char, lit: bool) -> egui::Response {
    let r = Rect::from_center_size(c, vec2(size + 12.0, size + 10.0));
    let resp = ui.interact(r, egui::Id::new(("systm-edit", id)), Sense::click());
    let (t, s) = (tip.to_string(), lit);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, s, t.clone()));
    app.auto.add(&format!("edit.{id}"), r, tip);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, Color32::from_white_alpha(12));
    }
    ico::paint(ui.painter(), c, size, icon, if lit { super::ICE } else if resp.hovered() { INK } else { MUTED });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- Media Pool

enum Entry {
    Bin { id: BinId, name: String, count: usize },
    Item { id: ItemId, name: String, video: bool, sequence: bool, duration: f64 },
}

pub(crate) fn pool(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect) {
    let (strip, body) = region(ui, r);
    // walk to the current bin; a path that no longer exists falls back to the root
    let mut names = vec!["Master".to_string()];
    let entries: Vec<Entry> = {
        let mut bin = &app.session.project.root;
        let mut ok = 0;
        for id in &app.systm.edit.bin {
            let next = bin.children.iter().find_map(|c| match c {
                BinEntry::Bin(b) if b.id == *id => Some(b),
                _ => None,
            });
            match next {
                Some(b) => {
                    bin = b;
                    names.push(b.name.clone());
                    ok += 1;
                }
                None => break,
            }
        }
        let _ = ok;
        bin.children
            .iter()
            .filter_map(|c| match c {
                BinEntry::Bin(b) => Some(Entry::Bin { id: b.id, name: b.name.clone(), count: b.children.len() }),
                BinEntry::Item(id) => app.session.project.item(*id).map(|it| Entry::Item {
                    id: *id,
                    name: it.name.clone(),
                    video: match &it.kind {
                        ItemKind::Media(m) => m.info.video.is_some(),
                        ItemKind::Sequence(_) => true,
                        _ => true,
                    },
                    sequence: matches!(it.kind, ItemKind::Sequence(_)),
                    duration: secs(it.duration()),
                }),
            })
            .collect()
    };
    app.systm.edit.bin.truncate(names.len() - 1);
    // strip: back, the path, search, the count
    let p = ui.painter().clone();
    let cy = strip.center().y;
    let mut x = strip.min.x + 8.0;
    if names.len() > 1 {
        if icon_button(app, ui, pos2(x + 9.0, cy), 13.0, "pool.back", "Up one bin", ico::CHEVRON_LEFT, false).clicked() {
            app.systm.edit.bin.pop();
        }
        x += 24.0;
    }
    p.text(pos2(x + 2.0, cy), egui::Align2::LEFT_CENTER, names.join("  /  "), Tokens::semibold(11.5), INK);
    if icon_button(app, ui, pos2(strip.max.x - 18.0, cy), 13.0, "pool.search", "Search the bin", ico::SEARCH, app.systm.edit.searching).clicked() {
        app.systm.edit.searching = !app.systm.edit.searching;
        if !app.systm.edit.searching {
            app.systm.edit.search.clear();
        }
    }
    if icon_button(app, ui, pos2(strip.max.x - 46.0, cy), 13.0, "pool.import", "Import media…", ico::PLUS, false).clicked() {
        run(app, "file.import", json!({}));
    }
    let mut body = body;
    if app.systm.edit.searching {
        let sr = Rect::from_min_size(body.min + vec2(8.0, 6.0), vec2(body.width() - 16.0, 26.0));
        p.rect(sr, 6.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        ui.put(sr.shrink2(vec2(8.0, 3.0)), egui::TextEdit::singleline(&mut app.systm.edit.search).hint_text("Search").frame(egui::Frame::NONE));
        body.min.y += 38.0;
    }
    let q = app.systm.edit.search.to_lowercase();
    // the grid
    let cols = ((body.width() - 12.0) / 124.0).floor().max(1.0);
    let cw = (body.width() - 12.0) / cols;
    let th = (cw - 14.0) * 9.0 / 16.0;
    let ch = th + 30.0;
    let ph = app.session.playhead();
    let picked = app.systm.edit.picked;
    let mut enter = None;
    let mut pick = None;
    let mut place = None;
    let mut open_seq = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body.shrink(6.0)));
    egui::ScrollArea::vertical().id_salt("systm-pool").auto_shrink([false, false]).show(&mut child, |ui| {
        let shown: Vec<&Entry> = entries
            .iter()
            .filter(|e| {
                q.is_empty()
                    || match e {
                        Entry::Bin { name, .. } | Entry::Item { name, .. } => name.to_lowercase().contains(&q),
                    }
            })
            .collect();
        if shown.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(egui::RichText::new(if q.is_empty() { "No media in this bin" } else { "Nothing matches" }).size(13.0).color(FAINT)));
        }
        let rows = shown.len().div_ceil(cols as usize);
        let (grid, _) = ui.allocate_exact_size(vec2(cols * cw, rows as f32 * ch), Sense::hover());
        for (i, e) in shown.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(grid.min.x + (i % cols as usize) as f32 * cw, grid.min.y + (i / cols as usize) as f32 * ch), vec2(cw, ch));
            if !ui.is_rect_visible(cell) {
                continue;
            }
            let pic = Rect::from_min_size(cell.min + vec2(7.0, 4.0), vec2(cw - 14.0, th));
            let (name, key) = match e {
                Entry::Bin { id, name, .. } => (name.clone(), format!("bin-{}", json!(id))),
                Entry::Item { id, name, .. } => (name.clone(), format!("item-{}", json!(id))),
            };
            let resp = ui.interact(cell, egui::Id::new(("systm-pool", &key)), Sense::click());
            let nm = name.clone();
            resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, nm.clone()));
            app.auto.add(&format!("pool.{key}"), cell, &name);
            let p = ui.painter().clone();
            let mut on = false;
            match e {
                Entry::Bin { id, count, .. } => {
                    p.rect_filled(pic, 6.0, Color32::from_rgb(0x16, 0x21, 0x27));
                    ico::paint(&p, pic.center(), pic.height() * 0.46, ico::FOLDER, super::ICE.gamma_multiply(0.85));
                    p.text(pos2(pic.max.x - 6.0, pic.max.y - 9.0), egui::Align2::RIGHT_CENTER, count.to_string(), Tokens::ui(10.0), MUTED);
                    if resp.double_clicked() {
                        enter = Some(*id);
                    }
                }
                Entry::Item { id, video, sequence, duration, .. } => {
                    on = picked == Some(*id);
                    p.rect_filled(pic, 6.0, WELL);
                    let thumb = if *video { app.thumbnail(ui.ctx(), *id, Tick((duration * 0.2 * TICKS_PER_SECOND as f64) as i64), 256) } else { None };
                    match thumb {
                        Some((tex, size)) => {
                            let fit = crate::panels::monitor::fit(pic, size.x, size.y);
                            ui.painter_at(pic).image(tex, fit, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        None if !*video => {
                            p.rect_filled(pic, 6.0, Color32::from_rgb(0x12, 0x33, 0x29));
                            ico::paint(&p, pic.center(), pic.height() * 0.4, ico::AUDIO_LINES, Color32::from_rgb(0x7c, 0xd9, 0xae));
                        }
                        None => {}
                    }
                    if *sequence {
                        let b = Rect::from_min_size(pic.min + vec2(4.0, 4.0), vec2(20.0, 16.0));
                        p.rect_filled(b, 4.0, Color32::from_black_alpha(170));
                        ico::paint(&p, b.center(), 10.5, ico::ROWS_3, super::ICE);
                    }
                    if resp.clicked() {
                        pick = Some(*id);
                    }
                    if resp.double_clicked() {
                        if *sequence { open_seq = Some(*id) } else { place = Some(*id) }
                    }
                }
            }
            p.rect_stroke(pic, 6.0, Stroke::new(if on { 2.0 } else { 1.0 }, if on { super::ICE } else if resp.hovered() { MUTED } else { LINE }), egui::StrokeKind::Inside);
            let label: String = if name.chars().count() as f32 * 6.4 > cw - 12.0 { name.chars().take(((cw - 24.0) / 6.4) as usize).collect::<String>() + "…" } else { name };
            p.text(pos2(pic.min.x + 1.0, pic.max.y + 12.0), egui::Align2::LEFT_CENTER, label, Tokens::ui(11.0), if on { INK } else { MUTED });
        }
    });
    if let Some(id) = enter {
        app.systm.edit.bin.push(id);
    }
    if let Some(id) = pick {
        app.systm.edit.picked = Some(id);
    }
    if let Some(id) = place {
        // a double-click lays the clip on the timeline at the playhead (Resolve's append is F12; this is the fast path)
        if run(app, "timeline.place", json!({"item": id, "time": ph.0, "insert": false})) {
            app.status("Placed at the playhead");
        }
    }
    if let Some(id) = open_seq {
        run(app, "sequence.open", json!({"sequence": id, "id": id}));
    }
}

// ---------------------------------------------------------------- Viewer

pub(crate) fn viewer(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect) {
    let (strip, body) = region(ui, r);
    let p = ui.painter().clone();
    let Some(seq) = app.session.active_sequence() else {
        crate::dock::placeholder(ui, body, &app.tokens, "No timeline is open");
        return;
    };
    let (rate, drop, dur) = (seq.settings.frame_rate, seq.settings.drop_frame, seq.duration());
    let name = app.session.state.active_sequence.and_then(|id| app.session.project.item(id)).map(|i| i.name.clone()).unwrap_or_default();
    let ph = app.session.playhead();
    let tc = |t: Tick| format_time(t, rate, drop, TimeDisplay::Timecode, 48000);
    // strip: duration on the left, the timeline's name in the centre, where we are on the right (Resolve's order)
    let cy = strip.center().y;
    p.text(pos2(strip.min.x + 12.0, cy), egui::Align2::LEFT_CENTER, tc(dur), egui::FontId::monospace(11.5), MUTED);
    p.text(pos2(strip.center().x, cy), egui::Align2::CENTER_CENTER, name, Tokens::semibold(12.0), INK);
    p.text(pos2(strip.max.x - 12.0, cy), egui::Align2::RIGHT_CENTER, tc(ph), egui::FontId::monospace(11.5), super::ICE);
    // the frame, above a scrub line and the transport
    let transport = Rect::from_min_max(pos2(body.min.x, body.max.y - 42.0), body.max);
    let scrub = Rect::from_min_max(pos2(body.min.x + 14.0, transport.min.y - 16.0), pos2(body.max.x - 14.0, transport.min.y - 4.0));
    let pic_area = Rect::from_min_max(body.min, pos2(body.max.x, scrub.min.y - 4.0));
    p.rect_filled(pic_area, 0.0, WELL);
    crate::panels::monitor::picture_only(app, ui, pic_area.shrink(1.0));
    // scrub
    let total = secs(dur).max(0.001);
    let k = (secs(ph) / total).clamp(0.0, 1.0) as f32;
    let line_y = scrub.center().y;
    p.line_segment([pos2(scrub.min.x, line_y), pos2(scrub.max.x, line_y)], Stroke::new(2.0, LINE));
    p.line_segment([pos2(scrub.min.x, line_y), pos2(scrub.min.x + scrub.width() * k, line_y)], Stroke::new(2.0, super::ICE.gamma_multiply(0.55)));
    let sr = ui.interact(scrub.expand2(vec2(6.0, 5.0)), egui::Id::new("systm-viewer-scrub"), Sense::click_and_drag());
    sr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Viewer position"));
    p.circle_filled(pos2(scrub.min.x + scrub.width() * k, line_y), if sr.hovered() || sr.dragged() { 6.0 } else { 4.5 }, super::ICE);
    if let Some(pt) = sr.interact_pointer_pos().filter(|_| sr.dragged() || sr.clicked()) {
        let k = ((pt.x - scrub.min.x) / scrub.width()).clamp(0.0, 1.0) as f64;
        app.session.set_playhead(Tick((k * total * TICKS_PER_SECOND as f64) as i64));
    }
    // transport, centred as Resolve lays it: |<  <  stop  >  >|  loop
    let cx = transport.center().x;
    let ty = transport.center().y;
    let playing = app.playback.playing;
    if icon_button(app, ui, pos2(cx - 104.0, ty), 15.0, "viewer.start", "Go to the first frame", ico::SKIP_BACK, false).clicked() {
        run(app, "playhead.start", json!({}));
    }
    if icon_button(app, ui, pos2(cx - 62.0, ty), 15.0, "viewer.back", "One frame back", ico::STEP_BACK, false).clicked() {
        run(app, "playhead.stepBack", json!({}));
    }
    if icon_button(app, ui, pos2(cx - 22.0, ty), 15.0, "viewer.stop", "Stop", ico::SQUARE, false).clicked() && playing {
        app.toggle_play(1.0);
    }
    if icon_button(app, ui, pos2(cx + 22.0, ty), 17.0, "viewer.play", if playing { "Pause (Space)" } else { "Play (Space)" }, if playing { ico::PAUSE } else { ico::PLAY }, playing).clicked() {
        app.toggle_play(1.0);
    }
    if icon_button(app, ui, pos2(cx + 62.0, ty), 15.0, "viewer.forward", "One frame forward", ico::STEP_FORWARD, false).clicked() {
        run(app, "playhead.stepForward", json!({}));
    }
    if icon_button(app, ui, pos2(cx + 104.0, ty), 15.0, "viewer.end", "Go to the last frame", ico::SKIP_FORWARD, false).clicked() {
        run(app, "playhead.end", json!({}));
    }
    let looping = app.playback.looping;
    if icon_button(app, ui, pos2(cx + 150.0, ty), 14.0, "viewer.loop", "Loop", ico::REPEAT, looping).clicked() {
        app.playback.looping = !looping;
    }
    if playing {
        ui.ctx().request_repaint();
    }
}

// ---------------------------------------------------------------- the page

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, body: Rect) {
    ui.painter().rect_filled(body, 0.0, BG);
    let o = app.systm.open.clone();
    let top_h = (body.height() * 0.54).round();
    let top = Rect::from_min_size(body.min, vec2(body.width(), top_h));
    let bottom = Rect::from_min_max(pos2(body.min.x, top.max.y + 1.0), body.max);
    // left: the pool, and under it the sound library when it is open
    let left_w = if o.media_pool || o.sound || o.effects { (body.width() * 0.27).clamp(250.0, 420.0) } else { 0.0 };
    let right_w = if o.inspector { (body.width() * 0.25).clamp(280.0, 360.0) } else { 0.0 };
    if left_w > 0.0 {
        let left = Rect::from_min_size(top.min, vec2(left_w, top.height()));
        let n = [o.media_pool, o.sound, o.effects].iter().filter(|x| **x).count() as f32;
        let h = left.height() / n;
        let mut y = left.min.y;
        if o.media_pool {
            pool(app, ui, Rect::from_min_size(pos2(left.min.x, y), vec2(left_w, h)));
            y += h;
        }
        if o.sound {
            let r = Rect::from_min_size(pos2(left.min.x, y), vec2(left_w, h));
            let (strip, inner) = region(ui, r);
            ui.painter().text(pos2(strip.min.x + 12.0, strip.center().y), egui::Align2::LEFT_CENTER, "Sound Library", Tokens::semibold(11.5), INK);
            super::bank::show(app, ui, inner);
            y += h;
        }
        if o.effects {
            super::fx_library::show(app, ui, Rect::from_min_size(pos2(left.min.x, y), vec2(left_w, h)), super::shell::Page::Edit);
        }
    }
    viewer(app, ui, Rect::from_min_max(pos2(top.min.x + left_w, top.min.y), pos2(top.max.x - right_w, top.max.y)));
    if right_w > 0.0 {
        super::inspector::show(app, ui, Rect::from_min_max(pos2(top.max.x - right_w, top.min.y), top.max), super::shell::Page::Edit);
    }
    // the timeline runs the full width under everything, with the mixer beside it when it is open
    let mix_w = if o.mixer { (body.width() * 0.26).clamp(260.0, 400.0) } else { 0.0 };
    let tl = Rect::from_min_max(bottom.min, pos2(bottom.max.x - mix_w, bottom.max.y));
    super::note_panel(app, crate::dock::PanelKind::Timeline, tl);
    super::timeline::show(app, ui, tl);
    if mix_w > 0.0 {
        let r = Rect::from_min_max(pos2(tl.max.x, bottom.min.y), bottom.max);
        let (strip, inner) = region(ui, r);
        ui.painter().text(pos2(strip.min.x + 12.0, strip.center().y), egui::Align2::LEFT_CENTER, "Mixer", Tokens::semibold(11.5), INK);
        crate::panels::mixer::track_mixer(app, ui, inner);
    }
    ui.painter().line_segment([top.left_bottom(), top.right_bottom()], Stroke::new(1.0, Color32::BLACK));
}
