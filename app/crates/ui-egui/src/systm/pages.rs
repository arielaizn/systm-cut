//! The page furniture SYSTM Cut does not have: the node graph of the current clip, the strip of
//! numbered clip thumbnails and the stills gallery of the Color page (research/resolve/ANALYSIS.md).

use egui::{Color32, Rect, Sense, Stroke, pos2, vec2};
use systmcut_project::TrackItem;

use crate::SystmCutApp;
use crate::theme::Tokens;

const INK: Color32 = Color32::from_rgb(0xdd, 0xe5, 0xe8);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x97, 0xa0);
const FAINT: Color32 = Color32::from_rgb(0x56, 0x66, 0x6e);
const CARD: Color32 = Color32::from_rgb(0x18, 0x24, 0x2a);
const EDGE: Color32 = Color32::from_rgb(0x2c, 0x39, 0x3f);
const GREEN: Color32 = Color32::from_rgb(0x7c, 0xd9, 0x92);

/// The clips of the first video track that has any, in timeline order.
fn strip_clips(app: &SystmCutApp) -> Vec<TrackItem> {
    let Some(seq) = app.session.active_sequence() else { return Vec::new() };
    let mut v: Vec<TrackItem> = seq.video_tracks.iter().find(|t| !t.items.is_empty()).map(|t| t.items.clone()).unwrap_or_default();
    v.sort_by_key(|i| i.start);
    v
}

/// The clip the page is about: the selected one, else the one under the playhead.
pub fn current_clip(app: &SystmCutApp) -> Option<TrackItem> {
    let seq = app.session.active_sequence()?;
    if let Some(it) = app.session.state.selection.iter().find_map(|c| seq.find_item(*c)).map(|(_, it)| it.clone()) {
        return Some(it);
    }
    let ph = app.session.state.active_sequence.and_then(|id| app.session.state.playheads.get(&id).copied())?;
    seq.video_tracks.iter().rev().flat_map(|t| &t.items).find(|it| it.start <= ph && ph < it.start + it.duration).cloned()
}

fn bezier(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, col: Color32) {
    let dx = ((b.x - a.x) * 0.5).max(24.0);
    let pts: Vec<egui::Pos2> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let (u, c1, c2) = (1.0 - t, pos2(a.x + dx, a.y), pos2(b.x - dx, b.y));
            pos2(u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x, u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y)
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
}

fn pretty(effect: &str) -> String {
    let s = effect.rsplit('.').next().unwrap_or(effect).replace(['_', '-'], " ");
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch.is_uppercase() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// The node graph: source, one node per effect on the current clip in processing order, output.
pub fn nodes(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    // the dotted canvas
    let step = 22.0;
    let mut y = rect.min.y + step / 2.0;
    while y < rect.max.y {
        let mut x = rect.min.x + step / 2.0;
        while x < rect.max.x {
            p.circle_filled(pos2(x, y), 0.8, Color32::from_white_alpha(16));
            x += step;
        }
        y += step;
    }
    let Some(clip) = current_clip(app) else {
        crate::dock::placeholder(ui, rect, &app.tokens, "Put the playhead on a clip to see its nodes");
        return;
    };
    let fx: Vec<(String, bool)> = clip.effects.iter().map(|e| (pretty(&e.effect), e.enabled)).collect();
    let n = fx.len().max(1);
    // nodes shrink to fit the room: the whole chain is always in view
    let gap = (rect.width() / (n as f32 * 5.0)).clamp(18.0, 80.0);
    let nw = ((rect.width() - 56.0 - gap * (n as f32 + 1.0)) / n as f32).clamp(54.0, 124.0);
    let nh = (nw * 0.64).clamp(44.0, 80.0);
    let total = nw * n as f32 + gap * (n as f32 + 1.0);
    let x0 = rect.center().x - total / 2.0;
    let cy = rect.center().y;
    let src = pos2((x0 - 6.0).max(rect.min.x + 16.0), cy);
    let out = pos2((x0 + total + 6.0).min(rect.max.x - 16.0), cy);
    let mut prev = src;
    let thumb = app.thumbnail(ui.ctx(), clip.item, clip.source_in, 240);
    for i in 0..n {
        // a gentle zig-zag, so parallel work reads as a graph and not a list
        let r = Rect::from_min_size(pos2(x0 + gap + (nw + gap) * i as f32, cy - nh / 2.0 + if n > 2 { (i as f32 - (n as f32 - 1.0) / 2.0) * 14.0 * if i % 2 == 0 { 1.0 } else { -1.0 } } else { 0.0 }), vec2(nw, nh));
        let (name, on) = fx.get(i).cloned().unwrap_or(("Clip".into(), true));
        let (a, b) = (pos2(r.min.x, r.center().y), pos2(r.max.x, r.center().y));
        bezier(&p, prev, a, super::ICE.gamma_multiply(0.75));
        let sel = i + 1 == n;
        p.rect(r.translate(vec2(0.0, 5.0)).expand(2.0), 12.0, Color32::from_black_alpha(90), Stroke::NONE, egui::StrokeKind::Inside);
        p.rect(r, 10.0, CARD, Stroke::new(if sel { 1.6 } else { 1.0 }, if sel { super::ICE } else { EDGE }), egui::StrokeKind::Inside);
        let pic = Rect::from_min_max(r.min + vec2(4.0, 4.0), pos2(r.max.x - 4.0, r.max.y - 22.0));
        if let Some((tex, _)) = thumb {
            p.image(tex, pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if on { Color32::WHITE } else { Color32::from_white_alpha(70) });
        } else {
            p.rect_filled(pic, 6.0, Color32::from_rgb(0x0a, 0x10, 0x13));
        }
        p.text(pos2(r.min.x + 8.0, r.max.y - 11.0), egui::Align2::LEFT_CENTER, format!("{:02}", i + 1), Tokens::semibold(10.0), if sel { super::ICE } else { MUTED });
        let label: String = name.chars().take(((nw - 34.0) / 6.5).max(0.0) as usize).collect();
        p.text(pos2(r.max.x - 8.0, r.max.y - 11.0), egui::Align2::RIGHT_CENTER, label.to_uppercase(), Tokens::semibold(9.0), if on { if sel { super::ICE } else { MUTED } } else { FAINT });
        for (dot, col) in [(a, GREEN), (b, GREEN)] {
            p.circle_filled(dot, 3.5, col);
        }
        app.auto.add(&format!("nodes.{}", i + 1), r, &name);
        let resp = ui.interact(r, egui::Id::new(("systm-node", i)), Sense::hover());
        let nm = name.clone();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Node {} {}", i + 1, nm)));
        prev = b;
    }
    bezier(&p, prev, out, super::ICE.gamma_multiply(0.75));
    p.circle_filled(src, 5.0, GREEN);
    p.circle_filled(out, 5.0, GREEN);
    p.text(pos2(rect.min.x + 12.0, rect.max.y - 14.0), egui::Align2::LEFT_CENTER, &clip.name, Tokens::ui(11.0), FAINT);
}

/// Every clip of the timeline as a numbered thumbnail; a click makes it the current clip.
pub fn clip_strip(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let clips = strip_clips(app);
    if clips.is_empty() {
        crate::dock::placeholder(ui, rect, &app.tokens, "No clips on the timeline yet");
        return;
    }
    let current = current_clip(app).map(|c| c.id);
    let pad = 8.0_f32;
    let mut pick = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(6.0, 4.0))));
    egui::ScrollArea::horizontal().id_salt("systm-clipstrip").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = pad;
            for (i, it) in clips.iter().enumerate() {
                let h = (rect.height() - 22.0).clamp(60.0, 132.0);
                let cw = ((h - 34.0) * 16.0 / 9.0).max(90.0);
                let (r, resp) = ui.allocate_exact_size(vec2(cw, h), Sense::click());
                let on = current == Some(it.id);
                let name = it.name.clone();
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, format!("Clip {:02} {}", i + 1, name)));
                app.auto.add(&format!("clips.{}", i + 1), r, &it.name);
                let p = ui.painter();
                p.text(pos2(r.min.x + 2.0, r.min.y + 7.0), egui::Align2::LEFT_CENTER, format!("{:02}", i + 1), Tokens::semibold(10.5), if on { super::ICE } else { INK });
                p.text(pos2(r.max.x - 2.0, r.min.y + 7.0), egui::Align2::RIGHT_CENTER, "V1", Tokens::ui(10.0), FAINT);
                let pic = Rect::from_min_max(pos2(r.min.x, r.min.y + 17.0), pos2(r.max.x, r.max.y - 17.0));
                match app.thumbnail(ui.ctx(), it.item, it.source_in, 264) {
                    Some((tex, _)) => {
                        ui.painter().image(tex, pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        ui.painter().rect_filled(pic, 6.0, CARD);
                    }
                }
                let p = ui.painter();
                p.rect_stroke(pic, 6.0, Stroke::new(if on { 2.0 } else { 1.0 }, if on { super::ICE } else if resp.hovered() { MUTED } else { EDGE }), egui::StrokeKind::Inside);
                let label: String = it.name.chars().take((cw / 6.6) as usize).collect();
                p.text(pos2(r.min.x + 2.0, r.max.y - 7.0), egui::Align2::LEFT_CENTER, label, Tokens::ui(10.5), if on { INK } else { MUTED });
                if resp.clicked() {
                    pick = Some((it.id, it.start));
                }
            }
        });
    });
    if let Some((id, start)) = pick {
        app.session.state.selection = vec![id];
        if let Some(seq) = app.session.state.active_sequence {
            app.session.state.playheads.insert(seq, start);
        }
    }
}

/// The stills gallery. Stills are not stored yet: the room is here, and says so.
pub fn gallery(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    p.text(rect.center() - vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, "No stills yet", Tokens::ui(15.0), FAINT);
    p.text(rect.center() + vec2(0.0, 12.0), egui::Align2::CENTER_CENTER, "Grabbing stills comes in a later build", Tokens::ui(10.5), FAINT.gamma_multiply(0.8));
    let _ = app;
}
