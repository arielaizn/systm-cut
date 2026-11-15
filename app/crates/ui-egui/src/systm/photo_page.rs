//! The Photo page: a layered image editor inside SYSTM Cut, laid out like Photoshop (an options
//! bar on top, tools in a column on the left, the canvas, and Layers / Adjustments on the right).
//!
//! The engine is PhotoCraft's (documents, layers, masks, blend modes, adjustment layers, type,
//! brushes, selections, filters, PSD), used as a library through its command interface; every
//! pixel of the interface here is ours. Agents drive the same commands (`photo.execute`).
//!
//! It is a page and not another app because it shares the film: a frame of the timeline opens as
//! a document, and a document goes onto the timeline.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use photocraft_color::PixelFormat;
use photocraft_doc::LayerContent;
use serde_json::{Value, json};

use super::ico;
use crate::SystmCutApp;
use crate::theme::Tokens;

const BG: Color32 = Color32::from_rgb(0x08, 0x0c, 0x0e);
const SURFACE: Color32 = Color32::from_rgb(0x10, 0x18, 0x1c);
const STRIP: Color32 = Color32::from_rgb(0x13, 0x1c, 0x21);
const WELL: Color32 = Color32::from_rgb(0x07, 0x0b, 0x0d);
const LINE: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x31);
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf0);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x9a, 0xa2);
const FAINT: Color32 = Color32::from_rgb(0x52, 0x62, 0x6a);

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Move,
    Marquee,
    Brush,
    Eraser,
    Type,
    Shape,
    Eyedropper,
    Hand,
}

const TOOLS: [(Tool, &str, char, &str); 8] = [
    (Tool::Move, "Move (V)", ico::MOVE, "move"),
    (Tool::Marquee, "Rectangular Marquee (M)", ico::SCAN_LINE, "marquee"),
    (Tool::Brush, "Brush (B)", ico::PENCIL, "brush"),
    (Tool::Eraser, "Eraser (E)", ico::DROPLET, "eraser"),
    (Tool::Type, "Type (T)", ico::TYPE, "type"),
    (Tool::Shape, "Rectangle (U)", ico::SQUARE, "shape"),
    (Tool::Eyedropper, "Eyedropper (I)", ico::CIRCLE_DOT, "eyedropper"),
    (Tool::Hand, "Hand (H)", ico::HAND, "hand"),
];

const BLENDS: [&str; 12] = ["Normal", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light", "Darken", "Lighten", "Color Dodge", "Color Burn", "Difference", "Luminosity"];
const ADJUST: [(&str, &str); 8] = [("Brightness/Contrast", "brightnessContrast"), ("Levels", "levels"), ("Curves", "curves"), ("Exposure", "exposure"), ("Vibrance", "vibrance"), ("Hue/Saturation", "hueSaturation"), ("Color Balance", "colorBalance"), ("Black & White", "blackWhite")];
const SIZES: [(&str, u32, u32); 4] = [("Reel cover 9:16", 1080, 1920), ("YouTube 16:9", 1920, 1080), ("Square 1:1", 1080, 1080), ("Carousel 4:5", 1080, 1350)];
const SWATCHES: [[u8; 3]; 8] = [[0xac, 0xf4, 0xf8], [0xf5, 0xf7, 0xf7], [0x08, 0x0c, 0x0e], [0x14, 0x1c, 0x20], [0xa8, 0xb4, 0xb8], [0xff, 0xd4, 0x8a], [0xff, 0x9c, 0x8a], [0x7b, 0xe3, 0xa1]];

pub struct PhotoState {
    session: Box<photocraft_engine::Session>,
    tex: Option<egui::TextureHandle>,
    dirty: bool,
    /// 0 = fit the canvas.
    zoom: f32,
    pan: egui::Vec2,
    pub tool: Tool,
    color: [u8; 3],
    size: f32,
    opacity: f32,
    font_size: f32,
    /// Points of the stroke or the corners of the drag in flight, in document pixels.
    stroke: Vec<[f32; 2]>,
    /// Where a text is being typed (document pixels), and what.
    typing: Option<([f32; 2], String)>,
    right_tab: usize,
    pub message: String,
}

impl Default for PhotoState {
    fn default() -> Self {
        Self { session: Box::default(), tex: None, dirty: true, zoom: 0.0, pan: egui::Vec2::ZERO, tool: Tool::Move, color: [0xac, 0xf4, 0xf8], size: 24.0, opacity: 1.0, font_size: 120.0, stroke: Vec::new(), typing: None, right_tab: 0, message: String::new() }
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl PhotoState {
    /// Run an engine command. The picture is redrawn after any command.
    pub fn execute(&mut self, command: &str, params: Value) -> Result<Value, String> {
        let r = self.session.execute(command, params).map_err(|e| e.to_string());
        self.dirty = true;
        if let Err(e) = &r {
            self.message = e.clone();
        }
        r
    }

    fn run(&mut self, command: &str, params: Value) -> bool {
        self.execute(command, params).is_ok()
    }

    pub fn has_document(&self) -> bool {
        self.session.active().is_some()
    }

    /// Open an image or a layered document from disk (PSD, PNG, JPEG, TIFF, WebP…).
    pub fn open(&mut self, path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let name = std::path::Path::new(path).file_name().map_or_else(|| "image".to_string(), |n| n.to_string_lossy().to_string());
        photocraft_engine::file_cmds::open_bytes_as(&mut self.session, &name, &bytes, None, Some(path.to_string())).map_err(|e| e.to_string())?;
        self.dirty = true;
        self.zoom = 0.0;
        self.pan = egui::Vec2::ZERO;
        Ok(())
    }

    /// What the agents and the CLI see of the document: its size and its layers, top first.
    pub fn state(&self) -> Value {
        let Some(d) = self.session.active() else { return json!({"document": null}) };
        json!({
            "document": {"name": d.doc.name, "width": d.doc.size.width, "height": d.doc.size.height, "path": d.path},
            "activeLayer": d.active_layer,
            "layers": d.doc.layers.iter().rev().map(|l| json!({"id": l.id, "name": l.name, "kind": l.content.kind_name(), "visible": l.visible, "opacity": l.opacity, "blend": format!("{:?}", l.blend)})).collect::<Vec<_>>(),
            "tool": TOOLS.iter().find(|t| t.0 == self.tool).map(|t| t.3),
            "foreground": hex(self.color),
        })
    }

    /// Every command of the engine with its parameters (for `photo.commands`).
    pub fn commands(filter: &str) -> Value {
        let f = filter.to_lowercase();
        Value::Array(photocraft_engine::command_specs().iter().filter(|c| f.is_empty() || c.id.to_lowercase().contains(&f) || c.label.to_lowercase().contains(&f)).map(|c| json!({"id": c.id, "label": c.label, "params": c.params})).collect())
    }

    /// The flattened picture as straight RGBA8, with its size.
    fn flatten(&self) -> Option<(usize, usize, Vec<u8>)> {
        let d = self.session.active()?;
        let (w, h) = (d.doc.size.width as usize, d.doc.size.height as usize);
        if w == 0 || h == 0 || w * h > 64_000_000 {
            return None;
        }
        let surface = photocraft_compose::composite_to_surface(&d.doc, d.doc.bounds(), PixelFormat::RGBA8, None);
        let mut px = vec![[0u8; 4]; w * h];
        surface.read_rgba8_into(d.doc.bounds(), &mut px);
        Some((w, h, px.into_iter().flatten().collect()))
    }

    /// Save the flattened picture as a PNG (for the timeline, and for agents to look at their work).
    pub fn export_png(&self, path: &str) -> Result<(), String> {
        let (w, h, px) = self.flatten().ok_or("no document open")?;
        let png = crate::control::encode_png(&px, w as u32, h as u32)?;
        std::fs::write(path, png).map_err(|e| e.to_string())
    }
}

fn tool_button(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect, id: &str, tip: &str, icon: char, on: bool) -> bool {
    let resp = ui.interact(r, egui::Id::new(("systm-photo", id)), Sense::click());
    let (t, s) = (tip.to_string(), on);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, s, t.clone()));
    app.auto.add(&format!("photo.{id}"), r, tip);
    let p = ui.painter();
    if on {
        p.rect(r.shrink(3.0), 8.0, super::ICE.gamma_multiply(0.14), Stroke::new(1.0, super::ICE.gamma_multiply(0.45)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(r.shrink(3.0), 8.0, Color32::from_white_alpha(12));
    }
    ico::paint(p, r.center(), 16.0, icon, if on { super::ICE } else if resp.hovered() { INK } else { MUTED });
    resp.on_hover_text(tip).clicked()
}

fn text_button(app: &mut SystmCutApp, ui: &mut egui::Ui, at: Pos2, id: &str, label: &str, primary: bool) -> (bool, f32) {
    let g = ui.painter().layout_no_wrap(label.to_string(), Tokens::ui(11.5), INK);
    let r = Rect::from_min_size(at, vec2(g.size().x + 20.0, 24.0));
    let resp = ui.interact(r, egui::Id::new(("systm-photo-b", id)), Sense::click());
    let l = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
    app.auto.add(&format!("photo.{id}"), r, label);
    let p = ui.painter();
    if primary {
        p.rect_filled(r, 7.0, if resp.hovered() { super::ICE } else { super::ICE.gamma_multiply(0.85) });
        p.galley(pos2(r.min.x + 10.0, r.center().y - g.size().y / 2.0), g, super::INK);
    } else {
        p.rect(r, 7.0, if resp.hovered() { Color32::from_white_alpha(14) } else { Color32::TRANSPARENT }, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        p.galley(pos2(r.min.x + 10.0, r.center().y - g.size().y / 2.0), g, if resp.hovered() { INK } else { MUTED });
    }
    (resp.clicked(), r.max.x + 6.0)
}

/// The start screen of the page: nothing open yet.
fn welcome(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect) {
    let p = ui.painter().clone();
    let c = r.center();
    p.text(pos2(c.x, c.y - 96.0), egui::Align2::CENTER_CENTER, "PHOTO", egui::FontId::new(34.0, egui::FontFamily::Name("display".into())), INK);
    p.text(pos2(c.x, c.y - 62.0), egui::Align2::CENTER_CENTER, "Covers, thumbnails and stills, in layers, beside the film", Tokens::ui(13.0), MUTED);
    let mut x = c.x - 300.0;
    for (i, (label, w, h)) in SIZES.iter().enumerate() {
        let card = Rect::from_min_size(pos2(x, c.y - 26.0), vec2(140.0, 96.0));
        let resp = ui.interact(card, egui::Id::new(("systm-photo-new", i)), Sense::click());
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, *label));
        app.auto.add(&format!("photo.new.{i}"), card, label);
        p.rect(card, 12.0, if resp.hovered() { Color32::from_rgb(0x16, 0x22, 0x28) } else { SURFACE }, Stroke::new(1.0, if resp.hovered() { super::ICE.gamma_multiply(0.6) } else { LINE }), egui::StrokeKind::Inside);
        // the shape of the format
        let k = 44.0 / (*w.max(h)) as f32;
        p.rect_stroke(Rect::from_center_size(pos2(card.center().x, card.min.y + 34.0), vec2(*w as f32 * k, *h as f32 * k)), 3.0, Stroke::new(1.5, super::ICE), egui::StrokeKind::Inside);
        p.text(pos2(card.center().x, card.max.y - 26.0), egui::Align2::CENTER_CENTER, *label, Tokens::semibold(11.0), INK);
        p.text(pos2(card.center().x, card.max.y - 11.0), egui::Align2::CENTER_CENTER, format!("{w} × {h}"), Tokens::ui(10.0), FAINT);
        if resp.clicked() {
            app.systm.photo.run("file.new", json!({"width": w, "height": h, "background": "#080c0e", "name": label}));
        }
        x += 153.0;
    }
    let (frame, nx) = text_button(app, ui, pos2(c.x - 150.0, c.y + 92.0), "from-frame", "Open the timeline's frame", true);
    if frame {
        frame_to_photo(app);
    }
    let (open, _) = text_button(app, ui, pos2(nx + 4.0, c.y + 92.0), "open", "Open a file…", false);
    if open {
        open_dialog(app);
    }
    if !app.systm.photo.message.is_empty() {
        p.text(pos2(c.x, c.y + 140.0), egui::Align2::CENTER_CENTER, &app.systm.photo.message, Tokens::ui(11.5), Color32::from_rgb(0xff, 0xd4, 0x8a));
    }
}

fn open_dialog(app: &mut SystmCutApp) {
    // the system's own chooser; it blocks for the moment it is open, like any Open dialog
    let script = "POSIX path of (choose file with prompt \"Open in Photo\")";
    let Ok(out) = std::process::Command::new("osascript").args(["-e", script]).output() else { return };
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !path.is_empty()
        && let Err(e) = app.systm.photo.open(&path)
    {
        app.systm.photo.message = e;
    }
}

/// The frame under the playhead becomes a document.
pub fn frame_to_photo(app: &mut SystmCutApp) {
    let path = super::data_dir().join("frame-to-photo.png");
    let _ = std::fs::create_dir_all(super::data_dir());
    let _ = std::fs::remove_file(&path);
    match app.session.execute("file.exportFrame", json!({"path": path.to_string_lossy(), "format": "png", "target": "program", "import": false})) {
        Ok(_) if path.is_file() => {
            if let Err(e) = app.systm.photo.open(&path.to_string_lossy()) {
                app.systm.photo.message = e;
            }
        }
        Ok(_) => app.systm.photo.message = "The frame is still rendering: try again in a moment".into(),
        Err(e) => app.systm.photo.message = format!("{e}"),
    }
}

/// The document goes onto the timeline at the playhead, as a still.
pub fn photo_to_timeline(app: &mut SystmCutApp) {
    let name = app.systm.photo.session.active().map(|d| d.doc.name.clone()).unwrap_or_else(|| "photo".into());
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let dir = super::data_dir().join("Photo");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{safe}.png"));
    if let Err(e) = app.systm.photo.export_png(&path.to_string_lossy()) {
        app.systm.photo.message = e;
        return;
    }
    let ph = app.session.playhead();
    match app.session.execute("file.import", json!({"paths": [path.to_string_lossy()]})) {
        Ok(v) => {
            let item = v.get("items").and_then(Value::as_array).and_then(|a| a.first()).cloned().or_else(|| v.get("item").cloned());
            if let Some(item) = item {
                let item = item.get("id").cloned().unwrap_or(item);
                if let Err(e) = app.session.execute("timeline.place", json!({"item": item, "time": ph.0, "track": "new", "insert": false})) {
                    app.systm.photo.message = format!("Imported; could not place it: {e}");
                    return;
                }
            }
            app.systm.photo.message = "On the timeline, at the playhead".into();
        }
        Err(e) => app.systm.photo.message = format!("{e}"),
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, body: Rect) {
    ui.painter().rect_filled(body, 0.0, BG);
    if !app.systm.photo.has_document() {
        welcome(app, ui, body);
        return;
    }
    let p = ui.painter().clone();
    // ---- geometry: options bar, tools column, canvas, right column
    let options = Rect::from_min_size(body.min, vec2(body.width(), 38.0));
    let tools = Rect::from_min_max(pos2(body.min.x, options.max.y), pos2(body.min.x + 46.0, body.max.y));
    let right_w = (body.width() * 0.24).clamp(250.0, 330.0);
    let right = Rect::from_min_max(pos2(body.max.x - right_w, options.max.y), body.max);
    let canvas = Rect::from_min_max(pos2(tools.max.x, options.max.y), pos2(right.min.x, body.max.y));

    // ---- the picture
    if app.systm.photo.dirty || app.systm.photo.tex.is_none() {
        if let Some((w, h, px)) = app.systm.photo.flatten() {
            let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &px);
            match &mut app.systm.photo.tex {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => app.systm.photo.tex = Some(ui.ctx().load_texture("systm-photo", img, egui::TextureOptions::LINEAR)),
            }
        }
        app.systm.photo.dirty = false;
    }
    let (dw, dh, doc_name, active, layers) = {
        let d = app.systm.photo.session.active();
        match d {
            Some(d) => (
                d.doc.size.width as f32,
                d.doc.size.height as f32,
                d.doc.name.clone(),
                d.active_layer,
                d.doc.layers.iter().rev().map(|l| (l.id, l.name.clone(), l.content.kind_name(), l.visible, l.opacity, format!("{:?}", l.blend), matches!(l.content, LayerContent::Text(_)))).collect::<Vec<_>>(),
            ),
            None => return,
        }
    };
    p.rect_filled(canvas, 0.0, Color32::from_rgb(0x05, 0x08, 0x0a));
    let fit = ((canvas.width() - 60.0) / dw).min((canvas.height() - 60.0) / dh).max(0.01);
    let scale = if app.systm.photo.zoom > 0.0 { app.systm.photo.zoom } else { fit };
    let origin = canvas.center() - vec2(dw, dh) * scale / 2.0 + app.systm.photo.pan;
    let doc_rect = Rect::from_min_size(origin, vec2(dw, dh) * scale);
    let pc = ui.painter_at(canvas);
    // shadow, checkerboard under transparency, the picture
    pc.rect_filled(doc_rect.translate(vec2(0.0, 8.0)).expand(6.0), 6.0, Color32::from_black_alpha(90));
    let cell = 12.0;
    let vis = doc_rect.intersect(canvas);
    if vis.is_positive() {
        pc.rect_filled(vis, 0.0, Color32::from_gray(52));
        let (mut y, mut row) = (doc_rect.min.y, 0);
        while y < vis.max.y {
            let mut x = doc_rect.min.x + if row % 2 == 0 { 0.0 } else { cell };
            while x < vis.max.x {
                let r = Rect::from_min_size(pos2(x, y), vec2(cell, cell)).intersect(vis);
                if r.is_positive() {
                    pc.rect_filled(r, 0.0, Color32::from_gray(72));
                }
                x += cell * 2.0;
            }
            y += cell;
            row += 1;
        }
    }
    if let Some(t) = &app.systm.photo.tex {
        pc.image(t.id(), doc_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    pc.rect_stroke(doc_rect, 0.0, Stroke::new(1.0, Color32::from_black_alpha(160)), egui::StrokeKind::Outside);
    let to_doc = |pt: Pos2| [(pt.x - origin.x) / scale, (pt.y - origin.y) / scale];
    let to_screen = |d: [f32; 2]| pos2(origin.x + d[0] * scale, origin.y + d[1] * scale);

    // ---- canvas input
    let resp = ui.interact(canvas, egui::Id::new("systm-photo-canvas"), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Canvas, true, "Photo canvas"));
    app.auto.add("photo.canvas", canvas, "Photo canvas");
    let tool = app.systm.photo.tool;
    if resp.hovered() {
        let (scroll, zoom, cmd) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.command));
        if (zoom - 1.0).abs() > 0.001 || (cmd && scroll.y != 0.0) {
            let f = if (zoom - 1.0).abs() > 0.001 { zoom } else { (scroll.y * 0.01).exp() };
            app.systm.photo.zoom = (scale * f).clamp(0.02, 32.0);
        } else if scroll != egui::Vec2::ZERO {
            app.systm.photo.pan += scroll;
        }
        ui.ctx().set_cursor_icon(match tool {
            Tool::Hand => egui::CursorIcon::Grab,
            Tool::Type => egui::CursorIcon::Text,
            Tool::Move => egui::CursorIcon::Move,
            _ => egui::CursorIcon::Crosshair,
        });
    }
    let pointer = resp.interact_pointer_pos();
    if let Some(pt) = pointer {
        let d = to_doc(pt);
        if resp.drag_started() {
            app.systm.photo.stroke = vec![d];
        } else if resp.dragged() {
            match tool {
                Tool::Hand => app.systm.photo.pan += resp.drag_delta(),
                Tool::Brush | Tool::Eraser => {
                    if app.systm.photo.stroke.last().is_none_or(|l| (l[0] - d[0]).hypot(l[1] - d[1]) > 1.5 / scale.max(0.01)) {
                        app.systm.photo.stroke.push(d);
                    }
                }
                _ => {
                    app.systm.photo.stroke.truncate(1);
                    app.systm.photo.stroke.push(d);
                }
            }
        }
        if resp.clicked() {
            match tool {
                Tool::Type => app.systm.photo.typing = Some((d, String::new())),
                Tool::Eyedropper => {
                    if let Some((w, h, px)) = app.systm.photo.flatten() {
                        let (x, y) = (d[0] as usize, d[1] as usize);
                        if x < w && y < h {
                            let i = (y * w + x) * 4;
                            app.systm.photo.color = [px[i], px[i + 1], px[i + 2]];
                        }
                    }
                }
                Tool::Marquee => {
                    app.systm.photo.run("select.deselect", json!({}));
                }
                _ => {}
            }
        }
    }
    // the stroke or the drag, as it will land
    let stroke = app.systm.photo.stroke.clone();
    let col = Color32::from_rgb(app.systm.photo.color[0], app.systm.photo.color[1], app.systm.photo.color[2]);
    if stroke.len() >= 2 {
        match tool {
            Tool::Brush | Tool::Eraser => {
                pc.add(egui::Shape::line(stroke.iter().map(|d| to_screen(*d)).collect(), Stroke::new((app.systm.photo.size * scale).max(1.0), if tool == Tool::Eraser { Color32::from_white_alpha(70) } else { col })));
            }
            Tool::Marquee | Tool::Shape => {
                let r = Rect::from_two_pos(to_screen(stroke[0]), to_screen(stroke[1]));
                if tool == Tool::Shape {
                    pc.rect_filled(r, 0.0, col.gamma_multiply(0.6));
                }
                pc.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
            }
            Tool::Move => {
                let a = to_screen(stroke[0]);
                pc.line_segment([a, to_screen(stroke[1])], Stroke::new(1.0, super::ICE));
            }
            _ => {}
        }
    }
    if resp.drag_stopped() && stroke.len() >= 2 {
        let (a, b) = (stroke[0], stroke[stroke.len() - 1]);
        let st = &mut app.systm.photo;
        let rect = json!({"x": a[0].min(b[0]).round() as i32, "y": a[1].min(b[1]).round() as i32, "width": (a[0] - b[0]).abs().round().max(1.0) as u32, "height": (a[1] - b[1]).abs().round().max(1.0) as u32});
        match tool {
            Tool::Move => {
                st.run("layer.translate", json!({"dx": (b[0] - a[0]).round() as i32, "dy": (b[1] - a[1]).round() as i32}));
            }
            Tool::Marquee => {
                st.run("select.rect", rect);
            }
            Tool::Shape => {
                let colour = hex(st.color);
                if st.run("select.rect", rect) {
                    st.run("layer.newFillLayer.solidColor", json!({"color": colour}));
                    st.run("select.deselect", json!({}));
                }
            }
            Tool::Brush | Tool::Eraser => {
                let colour = hex(st.color);
                st.run("tools.setColors", json!({"foreground": colour}));
                let points: Vec<Value> = stroke.iter().map(|d| json!([d[0], d[1], 1.0])).collect();
                let mut params = json!({"points": points, "size": st.size, "hardness": 0.85, "opacity": st.opacity});
                if tool == Tool::Eraser {
                    params["erase"] = json!(true);
                    params["tool"] = json!("eraser");
                }
                st.run("paint.stroke", params);
            }
            _ => {}
        }
    }
    if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
        app.systm.photo.stroke.clear();
    }
    // typing: a field where the text will stand; Enter sets it, Escape drops it
    if let Some((at, mut text)) = app.systm.photo.typing.take() {
        let s = to_screen(at);
        let field = Rect::from_min_size(pos2(s.x, s.y - 30.0), vec2(280.0, 30.0));
        ui.painter().rect(field, 6.0, WELL, Stroke::new(1.0, super::ICE), egui::StrokeKind::Outside);
        let edit = ui.put(field.shrink2(vec2(8.0, 4.0)), egui::TextEdit::singleline(&mut text).hint_text("Type, then Enter").frame(egui::Frame::NONE));
        edit.request_focus();
        let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if enter && !text.trim().is_empty() {
            let st = &mut app.systm.photo;
            let c = st.color;
            st.run("type.create", json!({"x": at[0], "y": at[1], "text": text, "size": st.font_size, "font": "SYSTM Display V2", "color": [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]}));
        } else if !esc && !enter {
            app.systm.photo.typing = Some((at, text));
        }
    }

    // ---- options bar: what the current tool needs, then the document's actions
    p.rect_filled(options, 0.0, STRIP);
    p.line_segment([options.left_bottom(), options.right_bottom()], Stroke::new(1.0, LINE));
    let cy = options.center().y;
    let name = TOOLS.iter().find(|t| t.0 == tool).map_or("", |t| t.1);
    p.text(pos2(options.min.x + 58.0, cy), egui::Align2::LEFT_CENTER, name.split(" (").next().unwrap_or(name), Tokens::semibold(11.5), INK);
    let mut x = options.min.x + 190.0;
    // colour: the brand's swatches, the current one ringed
    for (i, sw) in SWATCHES.iter().enumerate() {
        let r = Rect::from_center_size(pos2(x + 9.0, cy), vec2(18.0, 18.0));
        let resp = ui.interact(r, egui::Id::new(("systm-photo-sw", i)), Sense::click());
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Colour {}", hex(SWATCHES[i]))));
        p.circle_filled(r.center(), 8.0, Color32::from_rgb(sw[0], sw[1], sw[2]));
        p.circle_stroke(r.center(), 8.0, Stroke::new(1.0, Color32::from_white_alpha(40)));
        if app.systm.photo.color == *sw {
            p.circle_stroke(r.center(), 10.5, Stroke::new(1.5, super::ICE));
        }
        if resp.clicked() {
            app.systm.photo.color = *sw;
        }
        x += 23.0;
    }
    x += 10.0;
    let mut number = |ui: &mut egui::Ui, x: &mut f32, label: &str, v: &mut f32, lo: f32, hi: f32| {
        ui.painter().text(pos2(*x, cy), egui::Align2::LEFT_CENTER, label, Tokens::ui(11.0), MUTED);
        let lw = label.len() as f32 * 6.4 + 8.0;
        let r = Rect::from_min_size(pos2(*x + lw, cy - 11.0), vec2(56.0, 22.0));
        ui.painter().rect(r, 5.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(4.0, 1.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
        child.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        child.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        child.style_mut().visuals.override_text_color = Some(INK);
        child.add(egui::DragValue::new(v).range(lo..=hi).speed((hi - lo) / 300.0).fixed_decimals(0));
        *x = r.max.x + 14.0;
    };
    match tool {
        Tool::Brush | Tool::Eraser => {
            let mut size = app.systm.photo.size;
            number(ui, &mut x, "Size", &mut size, 1.0, 600.0);
            app.systm.photo.size = size;
            let mut op = app.systm.photo.opacity * 100.0;
            number(ui, &mut x, "Opacity", &mut op, 1.0, 100.0);
            app.systm.photo.opacity = op / 100.0;
        }
        Tool::Type => {
            let mut size = app.systm.photo.font_size;
            number(ui, &mut x, "Size", &mut size, 6.0, 1200.0);
            app.systm.photo.font_size = size;
            p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, "SYSTM Display V2", Tokens::ui(11.0), MUTED);
        }
        _ => {}
    }
    // right side of the bar: the document
    let mut rx = options.max.x - 12.0;
    let mut right_button = |app: &mut SystmCutApp, ui: &mut egui::Ui, rx: &mut f32, id: &str, label: &str, primary: bool| {
        let w = ui.painter().layout_no_wrap(label.to_string(), Tokens::ui(11.5), INK).size().x + 20.0;
        let (c, _) = text_button(app, ui, pos2(*rx - w, cy - 12.0), id, label, primary);
        *rx -= w + 6.0;
        c
    };
    if right_button(app, ui, &mut rx, "to-timeline", "To timeline", true) {
        photo_to_timeline(app);
    }
    if right_button(app, ui, &mut rx, "export", "Export PNG", false) {
        let path = super::home().join("Downloads").join(format!("{}.png", doc_name.replace('/', "-")));
        app.systm.photo.message = match app.systm.photo.export_png(&path.to_string_lossy()) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => e,
        };
    }
    if right_button(app, ui, &mut rx, "open", "Open…", false) {
        open_dialog(app);
    }
    if right_button(app, ui, &mut rx, "frame", "Frame from timeline", false) {
        frame_to_photo(app);
    }
    if tool_button(app, ui, Rect::from_center_size(pos2(rx - 16.0, cy), vec2(30.0, 30.0)), "redo", "Redo", ico::REDO_2, false) {
        app.systm.photo.run("edit.redo", json!({}));
    }
    if tool_button(app, ui, Rect::from_center_size(pos2(rx - 48.0, cy), vec2(30.0, 30.0)), "undo", "Undo", ico::UNDO_2, false) {
        app.systm.photo.run("edit.undo", json!({}));
    }

    // ---- tools column
    p.rect_filled(tools, 0.0, STRIP);
    p.line_segment([tools.right_top(), tools.right_bottom()], Stroke::new(1.0, LINE));
    for (i, (t, tip, icon, id)) in TOOLS.iter().enumerate() {
        let r = Rect::from_min_size(pos2(tools.min.x + 3.0, tools.min.y + 8.0 + i as f32 * 40.0), vec2(40.0, 38.0));
        if tool_button(app, ui, r, &format!("tool.{id}"), tip, *icon, tool == *t) {
            app.systm.photo.tool = *t;
        }
    }
    // the colour in hand, under the tools
    let chip = Rect::from_center_size(pos2(tools.center().x, tools.min.y + 8.0 + TOOLS.len() as f32 * 40.0 + 22.0), vec2(26.0, 26.0));
    p.rect(chip, 6.0, col, Stroke::new(1.5, Color32::from_white_alpha(120)), egui::StrokeKind::Inside);

    // ---- right column: Layers / Adjustments
    p.rect_filled(right, 0.0, SURFACE);
    p.line_segment([right.left_top(), right.left_bottom()], Stroke::new(1.0, LINE));
    let tabs = Rect::from_min_size(right.min, vec2(right.width(), 32.0));
    p.rect_filled(tabs, 0.0, STRIP);
    p.line_segment([tabs.left_bottom(), tabs.right_bottom()], Stroke::new(1.0, LINE));
    for (i, name) in ["Layers", "Adjustments"].iter().enumerate() {
        let tr = Rect::from_min_size(pos2(tabs.min.x + i as f32 * 110.0, tabs.min.y), vec2(110.0, tabs.height()));
        let on = app.systm.photo.right_tab == i;
        let resp = ui.interact(tr, egui::Id::new(("systm-photo-tab", i)), Sense::click());
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, *name));
        app.auto.add(&format!("photo.tab.{}", name.to_lowercase()), tr, name);
        p.text(tr.center(), egui::Align2::CENTER_CENTER, *name, Tokens::semibold(11.5), if on { INK } else { MUTED });
        if on {
            p.rect_filled(Rect::from_min_size(pos2(tr.min.x + 14.0, tr.max.y - 2.0), vec2(tr.width() - 28.0, 2.0)), 1.0, super::ICE);
        }
        if resp.clicked() {
            app.systm.photo.right_tab = i;
        }
    }
    let panel = Rect::from_min_max(pos2(right.min.x, tabs.max.y), right.max);
    if app.systm.photo.right_tab == 1 {
        // adjustments: one click adds it as an adjustment layer above the current layer
        let mut y = panel.min.y + 10.0;
        p.text(pos2(panel.min.x + 14.0, y + 6.0), egui::Align2::LEFT_CENTER, "ADD AN ADJUSTMENT LAYER", Tokens::semibold(9.5), FAINT);
        y += 22.0;
        for (label, id) in ADJUST {
            let r = Rect::from_min_size(pos2(panel.min.x + 8.0, y), vec2(panel.width() - 16.0, 30.0));
            let resp = ui.interact(r, egui::Id::new(("systm-photo-adj", id)), Sense::click());
            resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            app.auto.add(&format!("photo.adjust.{id}"), r, label);
            if resp.hovered() {
                p.rect_filled(r, 7.0, Color32::from_white_alpha(10));
            }
            ico::paint(&p, pos2(r.min.x + 16.0, r.center().y), 13.0, ico::SLIDERS_HORIZONTAL, if resp.hovered() { super::ICE } else { MUTED });
            p.text(pos2(r.min.x + 34.0, r.center().y), egui::Align2::LEFT_CENTER, label, Tokens::ui(12.0), INK);
            if resp.clicked() {
                app.systm.photo.run(&format!("layer.newAdjustmentLayer.{id}"), json!({}));
            }
            y += 32.0;
        }
        y += 10.0;
        p.text(pos2(panel.min.x + 14.0, y + 6.0), egui::Align2::LEFT_CENTER, "FILTERS", Tokens::semibold(9.5), FAINT);
        y += 22.0;
        for (label, cmd, params) in [("Gaussian Blur 8", "filter.blur.gaussianBlur", json!({"radius": 8})), ("Sharpen", "filter.sharpen.sharpen", json!({})), ("Add Noise 4%", "filter.noise.addNoise", json!({"amount": 4}))] {
            let r = Rect::from_min_size(pos2(panel.min.x + 8.0, y), vec2(panel.width() - 16.0, 30.0));
            let resp = ui.interact(r, egui::Id::new(("systm-photo-f", cmd)), Sense::click());
            resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            if resp.hovered() {
                p.rect_filled(r, 7.0, Color32::from_white_alpha(10));
            }
            ico::paint(&p, pos2(r.min.x + 16.0, r.center().y), 13.0, ico::SPARKLES, if resp.hovered() { super::ICE } else { MUTED });
            p.text(pos2(r.min.x + 34.0, r.center().y), egui::Align2::LEFT_CENTER, label, Tokens::ui(12.0), INK);
            if resp.clicked() {
                app.systm.photo.run(cmd, params);
            }
            y += 32.0;
        }
    } else {
        // the active layer's blend and opacity, as Photoshop puts them over the list
        let head = Rect::from_min_size(panel.min, vec2(panel.width(), 40.0));
        let cur = layers.iter().find(|l| Some(l.0) == active).cloned();
        if let Some((id, _, _, _, opacity, blend, _)) = &cur {
            let b = Rect::from_min_size(pos2(head.min.x + 10.0, head.min.y + 8.0), vec2(head.width() * 0.5 - 14.0, 24.0));
            p.rect(b, 6.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            let pretty = BLENDS.iter().find(|x| x.replace(' ', "").eq_ignore_ascii_case(blend)).copied().unwrap_or("Normal");
            p.text(pos2(b.min.x + 8.0, b.center().y), egui::Align2::LEFT_CENTER, pretty, Tokens::ui(11.5), INK);
            ico::paint(&p, pos2(b.max.x - 12.0, b.center().y), 12.0, ico::CHEVRON_DOWN, MUTED);
            let resp = ui.interact(b, egui::Id::new("systm-photo-blend"), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Blend mode"));
            if resp.on_hover_text("Click for the next blend mode (Shift: the one before)").clicked() {
                let i = BLENDS.iter().position(|x| *x == pretty).unwrap_or(0);
                let n = if ui.input(|i| i.modifiers.shift) { (i + BLENDS.len() - 1) % BLENDS.len() } else { (i + 1) % BLENDS.len() };
                app.systm.photo.run("layer.setProps", json!({"layer": id, "blend": BLENDS[n]}));
            }
            // opacity
            let o = Rect::from_min_max(pos2(b.max.x + 10.0, head.min.y + 8.0), pos2(head.max.x - 10.0, head.min.y + 32.0));
            p.rect(o, 6.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_max(o.min, pos2(o.min.x + o.width() * opacity.clamp(0.0, 1.0), o.max.y)).shrink(1.0), 5.0, super::ICE.gamma_multiply(0.22));
            p.text(o.center(), egui::Align2::CENTER_CENTER, format!("Opacity {:.0}%", opacity * 100.0), Tokens::ui(11.0), INK);
            let resp = ui.interact(o, egui::Id::new("systm-photo-opacity"), Sense::click_and_drag());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Layer opacity"));
            if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
                let v = ((pt.x - o.min.x) / o.width()).clamp(0.0, 1.0);
                app.systm.photo.run("layer.setProps", json!({"layer": id, "opacity": v}));
            }
        }
        p.line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, LINE));
        let foot = Rect::from_min_max(pos2(panel.min.x, panel.max.y - 38.0), panel.max);
        let list = Rect::from_min_max(pos2(panel.min.x, head.max.y), pos2(panel.max.x, foot.min.y));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        egui::ScrollArea::vertical().id_salt("systm-photo-layers").auto_shrink([false, false]).show(&mut child, |ui| {
            for (id, name, kind, visible, _, _, is_text) in &layers {
                let (row, resp) = ui.allocate_exact_size(vec2(list.width(), 44.0), Sense::click());
                let on = Some(*id) == active;
                let nm = name.clone();
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, format!("Layer {nm}")));
                app.auto.add(&format!("photo.layer.{}", id.0), row, name);
                let p = ui.painter().clone();
                if on {
                    p.rect_filled(row, 0.0, super::ICE.gamma_multiply(0.1));
                    p.rect_filled(Rect::from_min_size(row.min, vec2(2.0, row.height())), 0.0, super::ICE);
                } else if resp.hovered() {
                    p.rect_filled(row, 0.0, Color32::from_white_alpha(6));
                }
                p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, LINE));
                // the eye
                let eye = Rect::from_center_size(pos2(row.min.x + 20.0, row.center().y), vec2(28.0, 28.0));
                let er = ui.interact(eye, egui::Id::new(("systm-photo-eye", id.0)), Sense::click());
                let v = *visible;
                er.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, v, "Layer visible"));
                ico::paint(&p, eye.center(), 14.0, if *visible { ico::EYE } else { ico::EYE_OFF }, if *visible { INK } else { FAINT });
                if er.clicked() {
                    app.systm.photo.run("layer.setProps", json!({"layer": id, "visible": !visible}));
                }
                let icon = if *is_text {
                    ico::TYPE
                } else {
                    match *kind {
                        "Adjustment" => ico::SLIDERS_HORIZONTAL,
                        "Fill" | "Shape" => ico::SQUARE,
                        "Group" => ico::FOLDER,
                        _ => ico::IMAGE,
                    }
                };
                let thumb = Rect::from_center_size(pos2(row.min.x + 58.0, row.center().y), vec2(32.0, 32.0));
                p.rect(thumb, 5.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
                ico::paint(&p, thumb.center(), 15.0, icon, if on { super::ICE } else { MUTED });
                let label: String = name.chars().take(((row.width() - 100.0) / 6.8) as usize).collect();
                p.text(pos2(row.min.x + 84.0, row.center().y - 7.0), egui::Align2::LEFT_CENTER, label, Tokens::ui(12.0), if on { INK } else { MUTED });
                p.text(pos2(row.min.x + 84.0, row.center().y + 9.0), egui::Align2::LEFT_CENTER, *kind, Tokens::ui(10.0), FAINT);
                if resp.clicked() {
                    app.systm.photo.run("layer.select", json!({"layer": id}));
                }
            }
        });
        // the foot: new, duplicate, forward, back, delete
        p.rect_filled(foot, 0.0, STRIP);
        p.line_segment([foot.left_top(), foot.right_top()], Stroke::new(1.0, LINE));
        let acts: [(&str, &str, char, &str, Value); 5] = [
            ("layer-new", "New layer", ico::PLUS, "layer.new.layer", json!({})),
            ("layer-dup", "Duplicate layer", ico::COPY, "layer.duplicate", json!({})),
            ("layer-up", "Bring forward", ico::CHEVRON_RIGHT, "layer.arrange.bringForward", json!({})),
            ("layer-down", "Send backward", ico::CHEVRON_LEFT, "layer.arrange.sendBackward", json!({})),
            ("layer-del", "Delete layer", ico::TRASH_2, "layer.delete", json!({})),
        ];
        let step = foot.width() / acts.len() as f32;
        for (i, (id, tip, icon, cmd, params)) in acts.into_iter().enumerate() {
            let r = Rect::from_center_size(pos2(foot.min.x + step * (i as f32 + 0.5), foot.center().y), vec2(34.0, 30.0));
            if tool_button(app, ui, r, id, tip, icon, false) {
                app.systm.photo.run(cmd, params);
            }
        }
    }
    // the document's name and zoom, and the last message, along the bottom of the canvas
    let info = format!("{doc_name}   {} × {}   {:.0}%", dw as u32, dh as u32, scale * 100.0);
    pc.text(pos2(canvas.min.x + 12.0, canvas.max.y - 12.0), egui::Align2::LEFT_CENTER, info, Tokens::ui(10.5), FAINT);
    if !app.systm.photo.message.is_empty() {
        pc.text(pos2(canvas.max.x - 12.0, canvas.max.y - 12.0), egui::Align2::RIGHT_CENTER, &app.systm.photo.message, Tokens::ui(10.5), Color32::from_rgb(0xff, 0xd4, 0x8a));
    }
    // tool keys, when the pointer is over the canvas and nothing is being typed
    if resp.hovered() && app.systm.photo.typing.is_none() {
        for (key, t) in [(egui::Key::V, Tool::Move), (egui::Key::M, Tool::Marquee), (egui::Key::B, Tool::Brush), (egui::Key::E, Tool::Eraser), (egui::Key::T, Tool::Type), (egui::Key::U, Tool::Shape), (egui::Key::I, Tool::Eyedropper), (egui::Key::H, Tool::Hand)] {
            if ui.input(|i| i.key_pressed(key) && !i.modifiers.any()) {
                app.systm.photo.tool = t;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Num0) && i.modifiers.command) {
            app.systm.photo.zoom = 0.0;
            app.systm.photo.pan = egui::Vec2::ZERO;
        }
    }
}
