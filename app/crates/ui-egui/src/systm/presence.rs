//! Agent cursors: where each agent's last command landed, drawn over the editor.
//!
//! Every control request an agent's bridge sends carries `_agent`. The request is mapped to a
//! point in the window (the element it names, the point it clicks, or the panel its command
//! belongs to) and that agent's cursor glides there.

use std::collections::HashMap;

use egui::{Color32, Pos2, Stroke, pos2, vec2};
use serde_json::Value;

use crate::SystmCutApp;
use crate::dock::PanelKind;

pub struct Cursor {
    pos: Pos2,
    target: Pos2,
    /// What it is doing, shown beside the cursor for a moment.
    label: String,
    /// Time of the last command, and of the last click (for the ring).
    seen: f64,
    click: f64,
}

#[derive(Default)]
pub struct Presence {
    cursors: HashMap<String, Cursor>,
}

/// How long a cursor stays after its agent's last command.
const LINGER: f64 = 12.0;

/// The panel an engine command or control method acts on.
fn panel_of(method: &str, command: &str) -> Option<PanelKind> {
    let c = if command.is_empty() { method } else { command };
    let head = c.split('.').next().unwrap_or("");
    Some(match head {
        "timeline" | "sequence" | "clip" | "edit" | "marker" | "track" | "razor" | "trim" => PanelKind::Timeline,
        "file" | "project" | "bin" | "media" => PanelKind::Project,
        "effect" | "effects" | "transition" => PanelKind::EffectControls,
        "color" | "lumetri" => PanelKind::LumetriColor,
        "audio" | "mixer" => PanelKind::AudioTrackMixer,
        "graphics" | "title" | "caption" | "text" => PanelKind::EssentialGraphics,
        "playback" | "transport" | "view" | "monitor" | "export" => PanelKind::Program,
        _ => return None,
    })
}

/// Note a control request (called at the top of `control::handle`).
pub fn observe(app: &mut SystmCutApp, ctx: &egui::Context, method: &str, p: &Value) {
    let Some(agent) = p.get("_agent").and_then(Value::as_str) else { return };
    let now = ctx.input(|i| i.time);
    let s = |k: &str| p.get(k).and_then(Value::as_str);
    let command = s("command").unwrap_or("");
    let mut target = None;
    if let (Some(x), Some(y)) = (p.get("x").and_then(Value::as_f64), p.get("y").and_then(Value::as_f64)) {
        target = Some(pos2(x as f32, y as f32));
    }
    if target.is_none()
        && let Some(e) = s("id").or(s("element")).and_then(|id| app.auto.find(id))
    {
        target = Some(pos2(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0));
    }
    if target.is_none()
        && let Some(r) = panel_of(method, command).and_then(|k| app.systm.panel_rects.get(&k))
    {
        // not dead centre every time: a small walk inside the panel, so repeated commands read as work
        let n = app.systm.presence.cursors.get(agent).map(|c| c.seen).unwrap_or(0.0);
        let (jx, jy) = (((n * 7.3).sin() * 0.5 + 0.5) as f32, ((n * 3.1).cos() * 0.5 + 0.5) as f32);
        target = Some(r.min + vec2(r.width() * (0.25 + 0.5 * jx), r.height() * (0.3 + 0.4 * jy)));
    }
    let screen = ctx.content_rect();
    let c = app.systm.presence.cursors.entry(agent.to_string()).or_insert_with(|| {
        let start = pos2(screen.right() - 80.0, screen.center().y);
        Cursor { pos: start, target: start, label: String::new(), seen: now, click: -10.0 }
    });
    if let Some(t) = target {
        c.target = t;
    }
    c.seen = now;
    c.label = if command.is_empty() { method.to_string() } else { command.to_string() };
    if method != "ui.move" && !method.ends_with(".get") && !method.ends_with(".query") && method != "state.get" {
        c.click = now;
    }
    ctx.request_repaint();
}

impl Presence {
    pub fn animating(&self, now: f64) -> bool {
        self.cursors.values().any(|c| now - c.seen < LINGER)
    }
}

/// Draw the cursors over everything (called after the panels are drawn).
pub fn paint(app: &mut SystmCutApp, ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("systm-presence")));
    app.systm.presence.cursors.retain(|_, c| now - c.seen < LINGER);
    for (id, c) in &mut app.systm.presence.cursors {
        // critically damped glide to the target
        let k = 1.0 - (-dt * 9.0).exp();
        c.pos += (c.target - c.pos) * k;
        let col = super::agents::color_of(id);
        let fade = (((LINGER - (now - c.seen)) / 1.5).clamp(0.0, 1.0)) as f32;
        let col = col.gamma_multiply(fade);
        let since = (now - c.click) as f32;
        if (0.0..0.45).contains(&since) {
            let u = since / 0.45;
            painter.circle_stroke(c.pos, 6.0 + 22.0 * u, Stroke::new(2.0, col.gamma_multiply(1.0 - u)));
        }
        // the pointer: a rounded arrowhead in the agent's colour inside a white rim, and its name on a tag
        let p = c.pos;
        let tip = |dx: f32, dy: f32| p + vec2(dx, dy);
        let arrow = vec![tip(0.0, 0.0), tip(21.0, 8.4), tip(11.2, 12.4), tip(7.0, 22.0)];
        let white = Color32::from_white_alpha((255.0 * fade) as u8);
        painter.add(egui::Shape::convex_polygon(arrow.iter().map(|q| *q + vec2(0.0, 2.5)).collect(), Color32::from_black_alpha((70.0 * fade) as u8), Stroke::new(7.0, Color32::from_black_alpha((40.0 * fade) as u8))));
        painter.add(egui::Shape::convex_polygon(arrow.clone(), white, Stroke::new(6.0, white)));
        painter.add(egui::Shape::convex_polygon(arrow, col, Stroke::new(1.5, col)));
        let name = super::agents::identity(id).0;
        let text = if c.label.is_empty() || now - c.seen > 3.0 { name } else { format!("{name}  {}", c.label) };
        let galley = painter.layout_no_wrap(text, crate::theme::Tokens::semibold(11.5), super::INK);
        let at = p + vec2(16.0, 22.0);
        let r = egui::Rect::from_min_size(at, galley.size() + vec2(16.0, 8.0));
        painter.rect(r, r.height() / 2.0, col, Stroke::new(2.0, white), egui::StrokeKind::Outside);
        painter.galley(at + vec2(8.0, 4.0), galley, super::INK.gamma_multiply(fade));
    }
}
