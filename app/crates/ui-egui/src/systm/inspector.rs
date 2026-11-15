//! The Inspector, on every page, after Resolve's: the selected clip's name, icon tabs (Video,
//! Audio, Effects, Transition, Image, File), sections with an enable switch and a reset, rows of
//! sliders and number wells. It opens on the tab the page is about (Audio on Fairlight, Effects on
//! Fusion, Video elsewhere).
//!
//! Every effect on the clip is editable here from its own definition: a slider for a number, a
//! switch for a flag, two wells for a point, a stepper for a choice. All edits are engine commands.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};
use systmcut_project::{EffectInstance, EffectKind, ParamKind, TrackItem};
use systmcut_time::{TICKS_PER_SECOND, Tick};

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

const TABS: [(&str, char); 6] = [("Video", ico::FILM), ("Audio", ico::MUSIC), ("Effects", ico::WAND_SPARKLES), ("Transition", ico::BLEND), ("Image", ico::IMAGE), ("File", ico::INFO)];

#[derive(Default)]
pub struct InspectorState {
    /// The tab chosen on each page (by `Page as usize`); none = the page's own default.
    tab: [Option<usize>; 16],
}

fn run(app: &mut SystmCutApp, command: &str, params: Value) {
    if let Err(e) = app.session.execute(command, params) {
        app.status(format!("{e}"));
    }
}

fn icon_button(app: &mut SystmCutApp, ui: &mut egui::Ui, c: Pos2, size: f32, id: &str, tip: &str, icon: char) -> bool {
    let r = Rect::from_center_size(c, vec2(size + 12.0, size + 10.0));
    let resp = ui.interact(r, egui::Id::new(("systm-insp", id)), Sense::click());
    let t = tip.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t.clone()));
    app.auto.add(&format!("inspector.{id}"), r, tip);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, Color32::from_white_alpha(12));
    }
    ico::paint(ui.painter(), c, size, icon, if resp.hovered() { INK } else { MUTED });
    resp.on_hover_text(tip).clicked()
}

struct Rows<'a> {
    ui: &'a mut egui::Ui,
    y: f32,
    x0: f32,
    x1: f32,
}

impl Rows<'_> {
    fn row(&mut self, h: f32) -> Rect {
        let r = Rect::from_min_max(pos2(self.x0, self.y), pos2(self.x1, self.y + h));
        self.y += h;
        r
    }
    /// A section title: the enable switch, the name, the reset. Returns (toggled, reset).
    fn section(&mut self, app: &mut SystmCutApp, id: &str, name: &str, on: bool) -> (bool, bool) {
        let r = self.row(34.0);
        let p = self.ui.painter().clone();
        p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, LINE));
        let sw = Rect::from_min_size(pos2(r.min.x + 12.0, r.center().y - 7.0), vec2(26.0, 14.0));
        let resp = self.ui.interact(sw.expand(4.0), egui::Id::new(("systm-insp-sw", id)), Sense::click());
        let (n, o) = (name.to_string(), on);
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, o, n.clone()));
        app.auto.add(&format!("inspector.{id}.enabled"), sw, name);
        p.rect_filled(sw, 7.0, Color32::from_rgb(0x06, 0x09, 0x0b));
        p.circle_filled(pos2(if on { sw.max.x - 7.0 } else { sw.min.x + 7.0 }, sw.center().y), 4.5, if on { super::ICE } else { FAINT });
        p.text(pos2(sw.max.x + 12.0, r.center().y), egui::Align2::LEFT_CENTER, name, Tokens::semibold(12.5), if on { INK } else { MUTED });
        let reset = icon_button(app, self.ui, pos2(r.max.x - 18.0, r.center().y), 12.5, &format!("{id}.reset"), "Reset", ico::ROTATE_CCW);
        (resp.clicked(), reset)
    }
    /// A labelled row; returns where its controls go.
    fn labelled(&mut self, label: &str) -> Rect {
        let r = self.row(28.0);
        let lw = ((self.x1 - self.x0) * 0.36).clamp(84.0, 128.0);
        let text: String = label.chars().take((lw / 6.3) as usize).collect();
        self.ui.painter().text(pos2(r.min.x + lw, r.center().y), egui::Align2::RIGHT_CENTER, text, Tokens::ui(11.5), MUTED);
        Rect::from_min_max(pos2(r.min.x + lw + 10.0, r.min.y + 3.0), pos2(r.max.x - 14.0, r.max.y - 3.0))
    }
    /// A number well you drag or type in. True when it changed.
    fn number(&mut self, at: Rect, v: &mut f64, speed: f64, decimals: usize, suffix: &str) -> bool {
        self.ui.painter().rect(at, 5.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        let mut child = self.ui.new_child(egui::UiBuilder::new().max_rect(at.shrink2(vec2(4.0, 1.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        child.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        child.style_mut().visuals.override_text_color = Some(INK);
        child.add(egui::DragValue::new(v).speed(speed).fixed_decimals(decimals).suffix(suffix)).changed()
    }
    /// A slider and its number, as Resolve pairs them. True when it changed.
    fn slider(&mut self, id: &str, label: &str, v: &mut f64, lo: f64, hi: f64, decimals: usize, suffix: &str) -> bool {
        let at = self.labelled(label);
        let nw = 62.0_f32.min(at.width() * 0.4);
        let track = Rect::from_min_max(pos2(at.min.x + 2.0, at.center().y - 1.5), pos2(at.max.x - nw - 12.0, at.center().y + 1.5));
        let mut changed = false;
        if track.width() > 24.0 {
            self.ui.painter().rect_filled(track, 1.5, LINE);
            let sr = self.ui.interact(track.expand2(vec2(0.0, 9.0)), egui::Id::new(("systm-insp-sl", id)), Sense::click_and_drag());
            let l = label.to_string();
            sr.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, l.clone()));
            let k = (((*v - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0)) as f32;
            self.ui.painter().rect_filled(Rect::from_min_max(track.min, pos2(track.min.x + track.width() * k, track.max.y)), 1.5, super::ICE.gamma_multiply(0.5));
            self.ui.painter().circle_filled(pos2(track.min.x + track.width() * k, track.center().y), if sr.hovered() || sr.dragged() { 6.0 } else { 5.0 }, INK);
            if let Some(pt) = sr.interact_pointer_pos().filter(|_| sr.dragged() || sr.clicked()) {
                *v = lo + ((pt.x - track.min.x) / track.width()).clamp(0.0, 1.0) as f64 * (hi - lo);
                changed = true;
            }
        }
        let speed = ((hi - lo) / 400.0).max(0.001);
        changed | self.number(Rect::from_min_size(pos2(at.max.x - nw, at.min.y), vec2(nw, at.height())), v, speed, decimals, suffix)
    }
}

/// Every parameter of one effect, each drawn by its kind.
fn effect_rows(app: &mut SystmCutApp, rows: &mut Rows, clip: &Value, index: usize, e: &EffectInstance, local: Tick, frame: (f64, f64)) {
    let Some(def) = e.def() else { return };
    let set = |app: &mut SystmCutApp, param: &str, value: Value| run(app, "effects.setParam", json!({"clip": clip, "effect": index, "param": param, "value": value, "merge": true}));
    let mut group = None;
    for pd in &def.params {
        if pd.group != group {
            group = pd.group;
            if let Some(g) = group {
                let r = rows.row(22.0);
                rows.ui.painter().text(pos2(r.min.x + 14.0, r.center().y + 2.0), egui::Align2::LEFT_CENTER, g.to_uppercase(), Tokens::semibold(9.5), FAINT);
            }
        }
        let key = format!("{index}.{}", pd.id);
        match &pd.kind {
            ParamKind::Float { soft_min, soft_max, unit, decimals, .. } => {
                let mut v = e.f64_at(pd.id, local);
                if rows.slider(&key, pd.label, &mut v, *soft_min, *soft_max, *decimals as usize, unit) {
                    set(app, pd.id, json!(v));
                }
            }
            ParamKind::Angle => {
                let mut v = e.f64_at(pd.id, local);
                if rows.slider(&key, pd.label, &mut v, -180.0, 180.0, 1, "°") {
                    set(app, pd.id, json!(v));
                }
            }
            ParamKind::Bool => {
                let at = rows.labelled(pd.label);
                let on = e.f64_at(pd.id, local) > 0.5;
                let sw = Rect::from_min_size(pos2(at.min.x, at.center().y - 7.0), vec2(26.0, 14.0));
                let resp = rows.ui.interact(sw.expand(4.0), egui::Id::new(("systm-insp-b", &key)), Sense::click());
                let l = pd.label;
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, on, l));
                rows.ui.painter().rect_filled(sw, 7.0, Color32::from_rgb(0x06, 0x09, 0x0b));
                rows.ui.painter().circle_filled(pos2(if on { sw.max.x - 7.0 } else { sw.min.x + 7.0 }, sw.center().y), 4.5, if on { super::ICE } else { FAINT });
                if resp.clicked() {
                    set(app, pd.id, json!(!on));
                }
            }
            ParamKind::Point => {
                let at = rows.labelled(pd.label);
                let v = e.vec2_at(pd.id, local);
                let (mut x, mut y) = (if v.x.is_nan() { frame.0 / 2.0 } else { v.x }, if v.y.is_nan() { frame.1 / 2.0 } else { v.y });
                let half = (at.width() - 8.0) / 2.0;
                let a = rows.number(Rect::from_min_size(at.min, vec2(half, at.height())), &mut x, 1.0, 1, "");
                let b = rows.number(Rect::from_min_size(pos2(at.max.x - half, at.min.y), vec2(half, at.height())), &mut y, 1.0, 1, "");
                if a || b {
                    set(app, pd.id, json!([x, y]));
                }
            }
            ParamKind::Choice(options) => {
                let at = rows.labelled(pd.label);
                let cur = (e.f64_at(pd.id, local).max(0.0) as usize).min(options.len().saturating_sub(1));
                rows.ui.painter().rect(at, 5.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
                let name: String = options.get(cur).copied().unwrap_or("").chars().take(((at.width() - 44.0) / 6.6) as usize).collect();
                rows.ui.painter().text(pos2(at.min.x + 8.0, at.center().y), egui::Align2::LEFT_CENTER, name, Tokens::ui(11.5), INK);
                ico::paint(rows.ui.painter(), pos2(at.max.x - 12.0, at.center().y), 12.0, ico::CHEVRON_DOWN, MUTED);
                // a click steps to the next option (Shift: the one before)
                let resp = rows.ui.interact(at, egui::Id::new(("systm-insp-c", &key)), Sense::click());
                let l = pd.label;
                resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, l));
                if resp.clicked() && !options.is_empty() {
                    let back = rows.ui.input(|i| i.modifiers.shift);
                    let next = if back { (cur + options.len() - 1) % options.len() } else { (cur + 1) % options.len() };
                    set(app, pd.id, json!(next));
                }
            }
            ParamKind::Color => {
                let at = rows.labelled(pd.label);
                let c = e.params.get(pd.id).and_then(|p| p.value_at(local).as_color()).or_else(|| pd.default.as_color()).unwrap_or([1.0; 4]);
                let sw = Rect::from_min_size(at.min, vec2(46.0, at.height()));
                rows.ui.painter().rect(sw, 5.0, Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8), Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            }
            // curves, wheels, mask paths and text are edited on their own pages (Color, the viewer, Fusion)
            ParamKind::Curve { .. } | ParamKind::Wheel | ParamKind::Path | ParamKind::Text => {}
        }
    }
    rows.row(6.0);
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect, page: Page) {
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, SURFACE);
    let strip = Rect::from_min_size(r.min, vec2(r.width(), 30.0));
    p.rect_filled(strip, 0.0, STRIP);
    p.line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, LINE));
    p.rect_stroke(r, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    let body = Rect::from_min_max(pos2(r.min.x, strip.max.y), r.max);
    // the clip: the selected one, the way Resolve's Inspector follows the selection
    let clip: Option<TrackItem> = app.session.active_sequence().and_then(|seq| app.session.state.selection.iter().find_map(|c| seq.find_item(*c)).map(|(_, it)| it.clone()));
    let Some(clip) = clip else {
        p.text(pos2(strip.min.x + 12.0, strip.center().y), egui::Align2::LEFT_CENTER, "Inspector", Tokens::semibold(11.5), MUTED);
        p.text(body.center(), egui::Align2::CENTER_CENTER, "Nothing to inspect", Tokens::ui(17.0), FAINT);
        return;
    };
    ico::paint(&p, pos2(strip.min.x + 16.0, strip.center().y), 13.0, ico::FILM, MUTED);
    let title: String = clip.name.chars().take(((strip.width() - 50.0) / 7.2) as usize).collect();
    p.text(pos2(strip.min.x + 32.0, strip.center().y), egui::Align2::LEFT_CENTER, title, Tokens::semibold(12.0), INK);
    let default_tab = match page {
        Page::Fairlight => 1,
        Page::Fusion => 2,
        _ => 0,
    };
    let slot = (page as usize).min(15);
    let tab = app.systm.inspector.tab[slot].unwrap_or(default_tab);
    // tabs: an icon over its name, the current one over an Ice line
    let tabs = Rect::from_min_size(body.min, vec2(body.width(), 54.0));
    let tw = tabs.width() / TABS.len() as f32;
    for (i, (name, icon)) in TABS.iter().enumerate() {
        let tr = Rect::from_min_size(pos2(tabs.min.x + tw * i as f32, tabs.min.y), vec2(tw, tabs.height()));
        let on = tab == i;
        let resp = ui.interact(tr, egui::Id::new(("systm-insp-tab", i, slot)), Sense::click());
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, *name));
        app.auto.add(&format!("inspector.tab.{}", name.to_lowercase()), tr, name);
        let col = if on { INK } else if resp.hovered() { MUTED } else { FAINT };
        ico::paint(&p, pos2(tr.center().x, tr.min.y + 20.0), 17.0, *icon, col);
        if tw > 46.0 {
            p.text(pos2(tr.center().x, tr.max.y - 11.0), egui::Align2::CENTER_CENTER, *name, Tokens::ui(10.5), col);
        }
        if on {
            p.rect_filled(Rect::from_min_size(pos2(tr.min.x + 8.0, tr.max.y - 2.0), vec2(tw - 16.0, 2.0)), 1.0, super::ICE);
        }
        if resp.clicked() {
            app.systm.inspector.tab[slot] = Some(i);
        }
    }
    p.line_segment([tabs.left_bottom(), tabs.right_bottom()], Stroke::new(1.0, LINE));
    let area = Rect::from_min_max(pos2(body.min.x, tabs.max.y), body.max);
    let frame = app.session.active_sequence().map(|s| (s.settings.width as f64, s.settings.height as f64)).unwrap_or((1920.0, 1080.0));
    let local = Tick((app.session.playhead().0 - clip.start.0).max(0));
    let id = json!(clip.id);
    let kind_of = |e: &EffectInstance| e.def().map(|d| d.kind);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical().id_salt(("systm-inspector", slot)).auto_shrink([false, false]).show(&mut child, |ui| {
        let top = ui.cursor().min;
        let mut rows = Rows { y: top.y, x0: area.min.x, x1: area.max.x, ui };
        // which of the clip's effects this tab lists, and under what names (Resolve's for the built-in three)
        let listed: Vec<(usize, String)> = clip
            .effects
            .iter()
            .enumerate()
            .filter(|(_, e)| match tab {
                0 => matches!(e.effect.as_str(), "motion" | "opacity" | "time_remap"),
                1 => kind_of(e) == Some(EffectKind::Audio),
                2 => kind_of(e) == Some(EffectKind::Video) && !matches!(e.effect.as_str(), "motion" | "opacity" | "time_remap"),
                _ => false,
            })
            .map(|(i, e)| {
                let name = match e.effect.as_str() {
                    "motion" => "Transform".to_string(),
                    "opacity" => "Composite".to_string(),
                    "time_remap" => "Speed Change".to_string(),
                    _ => e.def().map_or_else(|| e.effect.clone(), |d| d.name.to_string()),
                };
                (i, name)
            })
            .collect();
        match tab {
            0..=2 => {
                if tab == 1 {
                    let (_, _) = rows.section(app, "clip-volume", "Volume", true);
                    let at = rows.labelled("Clip Gain");
                    rows.ui.painter().text(at.left_center(), egui::Align2::LEFT_CENTER, format!("{:+.1} dB", clip.gain_db), Tokens::ui(11.5), INK);
                    rows.row(6.0);
                }
                if listed.is_empty() && tab != 1 {
                    let r = rows.row(110.0);
                    rows.ui.painter().text(r.center() - vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, "No effects on this clip", Tokens::ui(13.0), FAINT);
                    rows.ui.painter().text(r.center() + vec2(0.0, 12.0), egui::Align2::CENTER_CENTER, "Double-click one in the Effects library", Tokens::ui(10.5), FAINT);
                }
                for (i, name) in listed {
                    let e = &clip.effects[i];
                    let (toggled, reset) = rows.section(app, &format!("fx{i}"), &name, e.enabled);
                    if toggled {
                        run(app, "effects.toggleEnabled", json!({"clip": id, "index": i}));
                    }
                    if reset {
                        run(app, "effects.reset", json!({"clip": id, "index": i}));
                    }
                    if e.enabled {
                        effect_rows(app, &mut rows, &id, i, e, local, frame);
                    }
                }
            }
            5 => {
                let s = |t: Tick| format!("{:.2} s", t.0 as f64 / TICKS_PER_SECOND as f64);
                for (k, v) in [("Clip", clip.name.clone()), ("Start", s(clip.start)), ("Duration", s(clip.duration)), ("Source In", s(clip.source_in)), ("Speed", format!("{:.0}%", clip.speed * 100.0)), ("Effects", clip.effects.len().to_string())] {
                    let at = rows.labelled(k);
                    let v: String = v.chars().take((at.width() / 7.0) as usize).collect();
                    rows.ui.painter().text(at.left_center(), egui::Align2::LEFT_CENTER, v, Tokens::ui(11.5), INK);
                }
            }
            _ => {
                let r = rows.row(110.0);
                rows.ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "Nothing here for this clip", Tokens::ui(13.0), FAINT);
            }
        }
        let (used, w) = (rows.y - top.y, rows.x1 - rows.x0);
        rows.ui.allocate_space(vec2(w, used + 10.0));
    });
}
