//! The Effects library, on every page, after Resolve's: categories on the left, the effects of the
//! one that is open on the right, a search. Each page sees what it works with (Fairlight the audio
//! effects, Color and Fusion the video ones, Cut and Edit everything, transitions included).
//! A double-click applies the effect to the selected clips.

use egui::{Color32, Rect, Sense, Stroke, pos2, vec2};
use serde_json::json;
use systmcut_project::{EffectKind, effect_defs};

use super::ico;
use super::shell::Page;
use crate::SystmCutApp;
use crate::theme::Tokens;

const SURFACE: Color32 = Color32::from_rgb(0x10, 0x18, 0x1c);
const STRIP: Color32 = Color32::from_rgb(0x13, 0x1c, 0x21);
const WELL: Color32 = Color32::from_rgb(0x07, 0x0b, 0x0d);
const LINE: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x31);
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf0);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x9a, 0xa2);
const FAINT: Color32 = Color32::from_rgb(0x52, 0x62, 0x6a);

#[derive(Default)]
pub struct LibraryState {
    search: String,
    /// The open category, by its label.
    open: String,
}

fn group_of(kind: EffectKind, category: &[&str]) -> String {
    match kind {
        EffectKind::VideoTransition => "Video Transitions".into(),
        EffectKind::AudioTransition => "Audio Transitions".into(),
        EffectKind::Audio => category.last().map_or("Audio FX".to_string(), |c| (*c).to_string()),
        EffectKind::Video => category.last().map_or("Video FX".to_string(), |c| (*c).to_string()),
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect, page: Page) {
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, SURFACE);
    let strip = Rect::from_min_size(r.min, vec2(r.width(), 30.0));
    p.rect_filled(strip, 0.0, STRIP);
    p.line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, LINE));
    p.rect_stroke(r, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    p.text(pos2(strip.min.x + 12.0, strip.center().y), egui::Align2::LEFT_CENTER, "Effects", Tokens::semibold(11.5), INK);
    // search, in the strip
    let sr = Rect::from_min_max(pos2(strip.min.x + 74.0, strip.min.y + 4.0), pos2(strip.max.x - 8.0, strip.max.y - 4.0));
    p.rect(sr, 6.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    ico::paint(&p, pos2(sr.min.x + 11.0, sr.center().y), 11.0, ico::SEARCH, FAINT);
    ui.put(Rect::from_min_max(pos2(sr.min.x + 22.0, sr.min.y + 1.0), sr.max - vec2(4.0, 1.0)), egui::TextEdit::singleline(&mut app.systm.library.search).hint_text("Search").frame(egui::Frame::NONE));
    let q = app.systm.library.search.to_lowercase();
    // what this page works with
    let shows = |k: EffectKind| match page {
        Page::Fairlight => matches!(k, EffectKind::Audio | EffectKind::AudioTransition),
        Page::Color | Page::Fusion => k == EffectKind::Video,
        _ => true,
    };
    let all: Vec<(String, &'static str, &'static str, EffectKind, bool)> = effect_defs().iter().filter(|d| !d.intrinsic && shows(d.kind)).map(|d| (group_of(d.kind, d.category), d.id, d.name, d.kind, d.accelerated)).collect();
    let mut groups: Vec<(String, usize)> = Vec::new();
    for (g, ..) in &all {
        match groups.iter_mut().find(|x| &x.0 == g) {
            Some(x) => x.1 += 1,
            None => groups.push((g.clone(), 1)),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    if !groups.iter().any(|g| g.0 == app.systm.library.open) {
        app.systm.library.open = groups.first().map(|g| g.0.clone()).unwrap_or_default();
    }
    let body = Rect::from_min_max(pos2(r.min.x, strip.max.y), r.max);
    let split = if q.is_empty() { (body.width() * 0.42).clamp(110.0, 190.0) } else { 0.0 };
    // categories
    if split > 0.0 {
        let left = Rect::from_min_size(body.min, vec2(split, body.height()));
        p.line_segment([left.right_top(), left.right_bottom()], Stroke::new(1.0, LINE));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left.shrink2(vec2(0.0, 4.0))));
        egui::ScrollArea::vertical().id_salt(("systm-fx-cats", page as usize)).auto_shrink([false, false]).show(&mut child, |ui| {
            for (g, n) in &groups {
                let (row, resp) = ui.allocate_exact_size(vec2(split, 26.0), Sense::click());
                let on = *g == app.systm.library.open;
                let name = g.clone();
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, name.clone()));
                if on {
                    ui.painter().rect_filled(row.shrink2(vec2(4.0, 1.0)), 6.0, super::ICE.gamma_multiply(0.1));
                } else if resp.hovered() {
                    ui.painter().rect_filled(row.shrink2(vec2(4.0, 1.0)), 6.0, Color32::from_white_alpha(8));
                }
                let label: String = g.chars().take(((split - 46.0) / 6.4) as usize).collect();
                ui.painter().text(pos2(row.min.x + 14.0, row.center().y), egui::Align2::LEFT_CENTER, label, Tokens::ui(11.5), if on { super::ICE } else { MUTED });
                ui.painter().text(pos2(row.max.x - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, n.to_string(), Tokens::ui(10.0), FAINT);
                if resp.clicked() {
                    app.systm.library.open = g.clone();
                }
            }
        });
    }
    // the effects
    let right = Rect::from_min_max(pos2(body.min.x + split, body.min.y), body.max);
    let open = app.systm.library.open.clone();
    let mut apply = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink2(vec2(0.0, 4.0))));
    egui::ScrollArea::vertical().id_salt(("systm-fx-list", page as usize)).auto_shrink([false, false]).show(&mut child, |ui| {
        let w = right.width();
        let mut any = false;
        for (g, id, name, kind, gpu) in &all {
            if if q.is_empty() { *g != open } else { !name.to_lowercase().contains(&q) } {
                continue;
            }
            any = true;
            let (row, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
            let nm = *name;
            resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, nm));
            app.auto.add(&format!("effects.{id}"), row, name);
            if resp.hovered() {
                ui.painter().rect_filled(row.shrink2(vec2(4.0, 1.0)), 6.0, Color32::from_white_alpha(10));
            }
            let icon = match kind {
                EffectKind::Audio | EffectKind::AudioTransition => ico::AUDIO_LINES,
                EffectKind::VideoTransition => ico::BLEND,
                EffectKind::Video => ico::SPARKLES,
            };
            ico::paint(ui.painter(), pos2(row.min.x + 18.0, row.center().y), 13.0, icon, if resp.hovered() { super::ICE } else { MUTED });
            let label: String = name.chars().take(((w - 70.0) / 6.6) as usize).collect();
            ui.painter().text(pos2(row.min.x + 34.0, row.center().y), egui::Align2::LEFT_CENTER, label, Tokens::ui(12.0), INK);
            if *gpu && w > 170.0 {
                ui.painter().text(pos2(row.max.x - 12.0, row.center().y), egui::Align2::RIGHT_CENTER, "GPU", Tokens::semibold(8.5), FAINT);
            }
            if resp.double_clicked() {
                apply = Some((*id, *kind, *name));
            }
            resp.on_hover_text("Double-click to apply to the selected clips");
        }
        if !any {
            ui.add_space(26.0);
            ui.vertical_centered(|ui| ui.label(egui::RichText::new("Nothing matches").size(12.5).color(FAINT)));
        }
    });
    if let Some((id, kind, name)) = apply {
        let (command, params) = match kind {
            EffectKind::VideoTransition => ("sequence.applyVideoTransition", json!({"effect": id})),
            EffectKind::AudioTransition => ("sequence.applyAudioTransition", json!({"effect": id})),
            _ => ("effects.apply", json!({"effect": id})),
        };
        match app.session.execute(command, params) {
            Ok(_) => app.status(format!("Applied {name}")),
            Err(e) => app.status(format!("{e}")),
        }
    }
}
