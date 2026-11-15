//! The 3D page: Blender's own engine behind a page of SYSTM Cut.
//!
//! Blender cannot be built into another program, and it does not need to be: it runs in the
//! background as its own process (`blender -b`, started the first time the page is opened and
//! not before, so no other page pays for it) and this page is its interface: a viewport you
//! orbit, an outliner, properties, the add menu, the timeline, render. Every scene is a real
//! Blender scene, every operation is Blender's (`bpy`), and a `.blend` saved here opens in
//! Blender.
//!
//! The two talk over a local socket, one JSON line per request (`blender_server.py`). A viewport
//! frame comes back in about 70 ms as raw pixels, so orbiting is live.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
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

const SERVER: &str = include_str!("blender_server.py");

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
}

const TOOLS: [(Tool, &str, char, &str); 4] = [(Tool::Select, "Select and orbit (W)", ico::MOUSE_POINTER_2, "select"), (Tool::Move, "Move (G)", ico::MOVE, "move"), (Tool::Rotate, "Rotate (R)", ico::ROTATE_CCW, "rotate"), (Tool::Scale, "Scale (S)", ico::MAXIMIZE_2, "scale")];

const ADD: [(&str, char, &str); 9] = [
    ("Cube", ico::SQUARE, "bpy.ops.mesh.primitive_cube_add(location=T)"),
    ("Sphere", ico::CIRCLE, "bpy.ops.mesh.primitive_uv_sphere_add(location=T); bpy.ops.object.shade_smooth()"),
    ("Cylinder", ico::CIRCLE_DOT, "bpy.ops.mesh.primitive_cylinder_add(location=T); bpy.ops.object.shade_smooth()"),
    ("Plane", ico::ROWS_3, "bpy.ops.mesh.primitive_plane_add(size=6, location=T)"),
    ("Torus", ico::CIRCLE, "bpy.ops.mesh.primitive_torus_add(location=T); bpy.ops.object.shade_smooth()"),
    ("Monkey", ico::BOT, "bpy.ops.mesh.primitive_monkey_add(location=T); bpy.ops.object.shade_smooth()"),
    ("Text", ico::TYPE, "bpy.ops.object.text_add(location=T); C.object.data.body='SYSTM'; C.object.data.extrude=0.08; C.object.rotation_euler[0]=1.5708"),
    ("Light", ico::SUN, "bpy.ops.object.light_add(type='AREA', location=(T[0]+3, T[1]-3, T[2]+5)); C.object.data.energy=800"),
    ("Camera", ico::MONITOR, "bpy.ops.object.camera_add(location=(T[0]+7, T[1]-7, T[2]+5)); C.object.rotation_euler=(1.1, 0, 0.785); C.scene.camera=C.object"),
];

const SWATCHES: [[u8; 3]; 8] = [[0xac, 0xf4, 0xf8], [0xf5, 0xf7, 0xf7], [0x14, 0x1c, 0x20], [0x08, 0x0c, 0x0e], [0xa8, 0xb4, 0xb8], [0xff, 0xd4, 0x8a], [0xff, 0x9c, 0x8a], [0x7b, 0xe3, 0xa1]];

enum Req {
    Call(u64, Value),
    Quit,
}

struct Resp {
    tag: u64,
    value: Result<Value, String>,
    /// A viewport frame: width, height, straight RGBA8.
    frame: Option<(usize, usize, Vec<u8>)>,
}

struct Link {
    child: Child,
    tx: Sender<Req>,
    rx: Receiver<Resp>,
}

pub struct ThreeState {
    link: Option<Link>,
    /// Blender is starting, or why it could not.
    status: String,
    scene: Value,
    tex: Option<egui::TextureHandle>,
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: [f32; 3],
    rendered: bool,
    pub tool: Tool,
    /// The viewport needs a new frame; one is being made (its tag).
    want_view: bool,
    view_in_flight: Option<u64>,
    /// Requests in flight other than views: tag and what to say when it lands.
    waiting: Vec<(u64, Option<String>)>,
    next_tag: u64,
    /// Movement of the pointer not yet applied to the object being transformed.
    pending_drag: egui::Vec2,
    pub message: String,
    last_render: Option<PathBuf>,
}

impl Default for ThreeState {
    fn default() -> Self {
        Self { link: None, status: String::new(), scene: Value::Null, tex: None, yaw: 0.9, pitch: 0.42, dist: 12.0, target: [0.0, 0.0, 0.6], rendered: false, tool: Tool::Select, want_view: true, view_in_flight: None, waiting: Vec::new(), next_tag: 1, pending_drag: egui::Vec2::ZERO, message: String::new(), last_render: None }
    }
}

fn blender_exe() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"), super::home().join("Applications/Blender.app/Contents/MacOS/Blender")];
    // Windows: C:\\Program Files\\Blender Foundation\\Blender <version>\\blender.exe, the newest first
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        let mut found: Vec<PathBuf> = std::fs::read_dir(PathBuf::from(pf).join("Blender Foundation")).into_iter().flatten().flatten().map(|e| e.path().join("blender.exe")).collect();
        found.sort();
        found.reverse();
        places.extend(found);
    }
    places.into_iter().find(|p| p.is_file()).or_else(|| super::which("blender"))
}

fn worker(port: u16, rx: Receiver<Req>, tx: Sender<Resp>) {
    // Blender takes a second or two to come up: keep knocking
    let mut stream = None;
    for _ in 0..120 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            stream = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    let Some(stream) = stream else {
        let _ = tx.send(Resp { tag: 0, value: Err("Blender did not start".into()), frame: None });
        return;
    };
    let _ = tx.send(Resp { tag: 0, value: Ok(Value::Null), frame: None });
    let mut reader = BufReader::new(stream.try_clone().expect("socket clone"));
    let mut writer = stream;
    while let Ok(req) = rx.recv() {
        let (tag, body) = match req {
            Req::Call(tag, body) => (tag, body),
            Req::Quit => {
                let _ = writeln!(writer, "{}", json!({"op": "quit"}));
                return;
            }
        };
        let raw = body.get("raw").and_then(Value::as_str).map(str::to_string);
        if writeln!(writer, "{body}").is_err() {
            let _ = tx.send(Resp { tag, value: Err("lost Blender".into()), frame: None });
            return;
        }
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            let _ = tx.send(Resp { tag, value: Err("Blender closed".into()), frame: None });
            return;
        }
        let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let value = if v.get("ok").and_then(Value::as_bool) == Some(true) { Ok(v.get("data").cloned().unwrap_or(Value::Null)) } else { Err(v.get("error").and_then(Value::as_str).unwrap_or("Blender error").to_string()) };
        // a viewport frame is left as raw pixels behind an 8-byte size header
        let frame = raw.filter(|_| value.is_ok()).and_then(|p| {
            let mut f = std::fs::File::open(p).ok()?;
            let mut head = [0u8; 8];
            f.read_exact(&mut head).ok()?;
            let (w, h) = (u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize, u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize);
            let mut px = vec![0u8; w.checked_mul(h)?.checked_mul(4)?];
            f.read_exact(&mut px).ok()?;
            Some((w, h, px))
        });
        if tx.send(Resp { tag, value, frame }).is_err() {
            return;
        }
    }
}

impl ThreeState {
    pub fn running(&self) -> bool {
        self.link.is_some()
    }

    /// Start Blender in the background (the first time the page is shown).
    fn start(&mut self) {
        let Some(exe) = blender_exe() else {
            self.status = "Blender is not installed. Get it from blender.org and reopen this page.".into();
            return;
        };
        let dir = super::data_dir().join("3D");
        let _ = std::fs::create_dir_all(&dir);
        let script = dir.join("blender_server.py");
        if let Err(e) = std::fs::write(&script, SERVER) {
            self.status = format!("Could not prepare the 3D engine: {e}");
            return;
        }
        let Some(port) = TcpListener::bind(("127.0.0.1", 0)).ok().and_then(|l| l.local_addr().ok()).map(|a| a.port()) else {
            self.status = "No free port for the 3D engine".into();
            return;
        };
        match Command::new(exe).args(["-b", "--factory-startup", "-noaudio", "--python"]).arg(&script).arg("--").arg(port.to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => {
                let (tx, wrx) = channel();
                let (wtx, rx) = channel();
                std::thread::spawn(move || worker(port, wrx, wtx));
                self.link = Some(Link { child, tx, rx });
                self.status = "Starting Blender…".into();
            }
            Err(e) => self.status = format!("Could not start Blender: {e}"),
        }
    }

    fn send(&mut self, body: Value, say: Option<String>) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        if let Some(l) = &self.link {
            let _ = l.tx.send(Req::Call(tag, body));
            self.waiting.push((tag, say));
        }
        tag
    }

    /// Run Python in Blender (`bpy`, `C`, `D`, `Vector`, `math` in scope; `T` is the view's target).
    pub fn exec(&mut self, code: &str, say: Option<&str>) {
        let t = self.target;
        self.send(json!({"op": "exec", "code": format!("T=({}, {}, {})\n{code}", t[0], t[1], t[2])}), say.map(str::to_string));
    }

    /// The same, waiting for the answer (agents and the CLI; the window waits with them).
    pub fn call_blocking(&mut self, body: Value, timeout: Duration) -> Result<Value, String> {
        if self.link.is_none() {
            self.start();
        }
        let tag = self.send(body, None);
        let until = Instant::now() + timeout;
        loop {
            let Some(l) = &self.link else { return Err(self.status.clone()) };
            match l.rx.recv_timeout(until.saturating_duration_since(Instant::now()).min(Duration::from_millis(200))) {
                Ok(r) if r.tag == tag => {
                    self.waiting.retain(|w| w.0 != tag);
                    self.absorb(&r);
                    self.want_view = true;
                    return r.value;
                }
                Ok(r) => self.absorb(&r),
                Err(_) if Instant::now() >= until => return Err("the 3D engine did not answer in time".into()),
                Err(_) => {}
            }
        }
    }

    fn absorb(&mut self, r: &Resp) {
        if r.tag == 0 {
            match &r.value {
                Ok(_) => {
                    self.status.clear();
                    self.send(json!({"op": "scene"}), None);
                    self.want_view = true;
                }
                Err(e) => {
                    self.status = e.clone();
                    self.link = None;
                }
            }
            return;
        }
        if let Ok(v) = &r.value {
            // any answer that carries the scene refreshes the outliner
            if let Some(s) = v.get("scene").filter(|s| s.get("objects").is_some()).or_else(|| v.get("objects").map(|_| v)) {
                self.scene = s.clone();
            }
        }
    }

    /// Take what Blender answered. Returns true while something is in flight.
    pub fn poll(&mut self, ctx: Option<&egui::Context>) -> bool {
        let mut got = Vec::new();
        if let Some(l) = &self.link {
            while let Ok(r) = l.rx.try_recv() {
                got.push(r);
            }
        }
        for r in got {
            self.absorb(&r);
            if Some(r.tag) == self.view_in_flight {
                self.view_in_flight = None;
                if let (Some((w, h, px)), Some(ctx)) = (&r.frame, ctx) {
                    let img = egui::ColorImage::from_rgba_unmultiplied([*w, *h], px);
                    match &mut self.tex {
                        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                        None => self.tex = Some(ctx.load_texture("systm-3d", img, egui::TextureOptions::LINEAR)),
                    }
                }
                if let Err(e) = &r.value {
                    self.message = e.clone();
                }
            } else if let Some(i) = self.waiting.iter().position(|w| w.0 == r.tag) {
                let (_, say) = self.waiting.remove(i);
                match (&r.value, say) {
                    (Err(e), _) => self.message = e.clone(),
                    (Ok(_), Some(s)) => self.message = s,
                    _ => {}
                }
                self.want_view = true;
            }
        }
        self.view_in_flight.is_some() || !self.waiting.is_empty() || (self.link.is_some() && !self.status.is_empty())
    }

    pub fn scene(&self) -> Value {
        self.scene.clone()
    }

    fn objects(&self) -> Vec<Obj> {
        self.scene
            .get("objects")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|o| {
                let v3 = |k: &str| -> [f64; 3] {
                    let a = o.get(k).and_then(Value::as_array);
                    let g = |i: usize| a.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
                    [g(0), g(1), g(2)]
                };
                Some(Obj { name: o.get("name")?.as_str()?.to_string(), kind: o.get("type").and_then(Value::as_str).unwrap_or("").to_string(), selected: o.get("selected").and_then(Value::as_bool).unwrap_or(false), active: o.get("active").and_then(Value::as_bool).unwrap_or(false), visible: o.get("visible").and_then(Value::as_bool).unwrap_or(true), loc: v3("location"), rot: v3("rotation"), scale: v3("scale"), modifiers: o.get("modifiers").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default() })
            })
            .collect()
    }

    pub fn shutdown(&mut self) {
        if let Some(mut l) = self.link.take() {
            let _ = l.tx.send(Req::Quit);
            std::thread::sleep(Duration::from_millis(120));
            let _ = l.child.kill();
            let _ = l.child.wait();
        }
    }
}

impl Drop for ThreeState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Clone)]
struct Obj {
    name: String,
    kind: String,
    selected: bool,
    active: bool,
    visible: bool,
    loc: [f64; 3],
    rot: [f64; 3],
    scale: [f64; 3],
    modifiers: Vec<String>,
}

/// A Python string literal of `s`.
fn py(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn button(app: &mut SystmCutApp, ui: &mut egui::Ui, r: Rect, id: &str, tip: &str, icon: char, on: bool) -> bool {
    let resp = ui.interact(r, egui::Id::new(("systm-3d", id)), Sense::click());
    let (t, s) = (tip.to_string(), on);
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, s, t.clone()));
    app.auto.add(&format!("three.{id}"), r, tip);
    let p = ui.painter();
    if on {
        p.rect(r.shrink(3.0), 8.0, super::ICE.gamma_multiply(0.14), Stroke::new(1.0, super::ICE.gamma_multiply(0.45)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(r.shrink(3.0), 8.0, Color32::from_white_alpha(12));
    }
    ico::paint(p, r.center(), 15.0, icon, if on { super::ICE } else if resp.hovered() { INK } else { MUTED });
    resp.on_hover_text(tip).clicked()
}

fn text_button(app: &mut SystmCutApp, ui: &mut egui::Ui, at: Pos2, id: &str, label: &str, primary: bool) -> (bool, f32) {
    let g = ui.painter().layout_no_wrap(label.to_string(), Tokens::ui(11.5), INK);
    let r = Rect::from_min_size(at, vec2(g.size().x + 20.0, 24.0));
    let resp = ui.interact(r, egui::Id::new(("systm-3d-b", id)), Sense::click());
    let l = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
    app.auto.add(&format!("three.{id}"), r, label);
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

fn choose_file(prompt: &str) -> Option<String> {
    let out = Command::new("osascript").args(["-e", &format!("POSIX path of (choose file with prompt {})", py(prompt))]).output().ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!p.is_empty()).then_some(p)
}

/// The render, as a still on the timeline at the playhead.
fn render_to_timeline(app: &mut SystmCutApp) {
    let Some(path) = app.systm.three.last_render.clone().filter(|p| p.is_file()) else {
        app.systm.three.message = "Render an image first".into();
        return;
    };
    let ph = app.session.playhead();
    match app.session.execute("file.import", json!({"paths": [path.to_string_lossy()]})) {
        Ok(v) => {
            let item = v.get("items").and_then(Value::as_array).and_then(|a| a.first()).cloned().or_else(|| v.get("item").cloned());
            if let Some(item) = item {
                let item = item.get("id").cloned().unwrap_or(item);
                let _ = app.session.execute("timeline.place", json!({"item": item, "time": ph.0, "track": "new", "insert": false}));
            }
            app.systm.three.message = "On the timeline, at the playhead".into();
        }
        Err(e) => app.systm.three.message = format!("{e}"),
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, body: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(body, 0.0, BG);
    if !app.systm.three.running() && app.systm.three.status.is_empty() {
        app.systm.three.start();
    }
    if app.systm.three.poll(Some(ui.ctx())) {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
    if !app.systm.three.running() || (app.systm.three.tex.is_none() && !app.systm.three.status.is_empty()) {
        p.text(body.center() - vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "3D", egui::FontId::new(34.0, egui::FontFamily::Name("display".into())), INK);
        p.text(body.center() + vec2(0.0, 22.0), egui::Align2::CENTER_CENTER, &app.systm.three.status, Tokens::ui(13.0), MUTED);
        if app.systm.three.running() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        return;
    }
    // ---- geometry: header, tools, viewport, right column, frame strip
    let header = Rect::from_min_size(body.min, vec2(body.width(), 38.0));
    let strip = Rect::from_min_max(pos2(body.min.x, body.max.y - 34.0), body.max);
    let tools = Rect::from_min_max(pos2(body.min.x, header.max.y), pos2(body.min.x + 46.0, strip.min.y));
    let right_w = (body.width() * 0.25).clamp(250.0, 330.0);
    let right = Rect::from_min_max(pos2(body.max.x - right_w, header.max.y), pos2(body.max.x, strip.min.y));
    let view = Rect::from_min_max(pos2(tools.max.x, header.max.y), pos2(right.min.x, strip.min.y));
    let objects = app.systm.three.objects();
    let active = objects.iter().find(|o| o.active).cloned();

    // ---- viewport
    p.rect_filled(view, 0.0, Color32::from_rgb(0x3d, 0x3d, 0x3d));
    if let Some(t) = &app.systm.three.tex {
        ui.painter_at(view).image(t.id(), view, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let resp = ui.interact(view, egui::Id::new("systm-3d-view"), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Canvas, true, "3D viewport"));
    app.auto.add("three.viewport", view, "3D viewport");
    let tool = app.systm.three.tool;
    let (shift, scroll) = ui.input(|i| (i.modifiers.shift, i.smooth_scroll_delta));
    {
        let st = &mut app.systm.three;
        // the view's own axes, to move things the way the pointer moves
        let (sy, cy, sp, cp) = (st.yaw.sin(), st.yaw.cos(), st.pitch.sin(), st.pitch.cos());
        let right_v = [cy, sy, 0.0];
        let up_v = [-sp * sy, sp * cy, cp];
        let per_px = st.dist * 0.72 / view.width().max(1.0);
        if resp.hovered() && scroll.y != 0.0 {
            st.dist = (st.dist * (-scroll.y * 0.004).exp()).clamp(0.2, 2000.0);
            st.want_view = true;
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            if tool == Tool::Select || active.is_none() {
                if shift {
                    for k in 0..3 {
                        st.target[k] += (-right_v[k] * d.x + up_v[k] * d.y) * per_px;
                    }
                } else {
                    st.yaw -= d.x * 0.008;
                    st.pitch = (st.pitch + d.y * 0.008).clamp(-1.5, 1.5);
                }
                st.want_view = true;
            } else {
                st.pending_drag += d;
            }
        }
        // transform the active object by what the pointer moved, as fast as Blender answers
        if st.pending_drag != egui::Vec2::ZERO && st.waiting.is_empty() && let Some(o) = &active {
            let d = std::mem::take(&mut st.pending_drag);
            let name = py(&o.name);
            let code = match tool {
                Tool::Move => {
                    let m: Vec<f32> = (0..3).map(|k| (right_v[k] * d.x - up_v[k] * d.y) * per_px).collect();
                    format!("o=D.objects[{name}]; o.location=(o.location[0]+{}, o.location[1]+{}, o.location[2]+{})", m[0], m[1], m[2])
                }
                Tool::Rotate => format!("o=D.objects[{name}]; o.rotation_euler[2]+={}", d.x * 0.01),
                _ => format!("o=D.objects[{name}]; k={}; o.scale=(o.scale[0]*k, o.scale[1]*k, o.scale[2]*k)", (1.0 + (d.x - d.y) * 0.005).max(0.01)),
            };
            st.exec(&code, None);
        }
        if resp.clicked() && let Some(pt) = resp.interact_pointer_pos() {
            let (x, y) = ((pt.x - view.min.x) / view.width(), (pt.y - view.min.y) / view.height());
            st.send(json!({"op": "pick", "yaw": st.yaw, "pitch": st.pitch, "dist": st.dist, "target": st.target, "x": x, "y": y, "aspect": view.width() / view.height().max(1.0), "extend": shift}), None);
        }
        // ask for a frame when the view changed and none is on its way
        if st.want_view && st.view_in_flight.is_none() && st.status.is_empty() {
            st.want_view = false;
            let dir = super::data_dir().join("3D");
            let ppp = ui.ctx().pixels_per_point().min(2.0);
            let (w, h) = (((view.width() * ppp) as u32).clamp(64, 2200) & !1, ((view.height() * ppp) as u32).clamp(64, 1600) & !1);
            let tag = st.next_tag;
            st.next_tag += 1;
            if let Some(l) = &st.link {
                let _ = l.tx.send(Req::Call(tag, json!({"op": "view", "yaw": st.yaw, "pitch": st.pitch, "dist": st.dist, "target": st.target, "w": w, "h": h, "rendered": st.rendered, "path": dir.join("view.png").to_string_lossy(), "raw": dir.join("view.rgba").to_string_lossy()})));
                st.view_in_flight = Some(tag);
            }
        }
    }
    // the axes, in the corner, turning with the view
    {
        let st = &app.systm.three;
        let c = pos2(view.max.x - 44.0, view.min.y + 44.0);
        let (sy, cy, sp, cp) = (st.yaw.sin(), st.yaw.cos(), st.pitch.sin(), st.pitch.cos());
        let proj = |v: [f32; 3]| vec2(v[0] * cy + v[1] * sy, -(v[0] * -sp * sy + v[1] * sp * cy + v[2] * cp)) * 26.0;
        let pv = ui.painter_at(view);
        pv.circle_filled(c, 34.0, Color32::from_black_alpha(70));
        for (axis, col, name) in [([1.0, 0.0, 0.0], Color32::from_rgb(0xff, 0x6b, 0x6b), "X"), ([0.0, 1.0, 0.0], Color32::from_rgb(0x7b, 0xe3, 0x8a), "Y"), ([0.0, 0.0, 1.0], Color32::from_rgb(0x6b, 0xa8, 0xff), "Z")] {
            let e = c + proj(axis);
            pv.line_segment([c, e], Stroke::new(2.0, col));
            pv.circle_filled(e, 7.0, col);
            pv.text(e, egui::Align2::CENTER_CENTER, name, Tokens::semibold(9.0), Color32::BLACK);
        }
    }

    // ---- header: Add, shading, files, render
    p.rect_filled(header, 0.0, STRIP);
    p.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, LINE));
    let cy = header.center().y;
    p.text(pos2(header.min.x + 58.0, cy), egui::Align2::LEFT_CENTER, "Add", Tokens::semibold(11.5), MUTED);
    let mut x = header.min.x + 92.0;
    for (name, icon, code) in ADD {
        if button(app, ui, Rect::from_center_size(pos2(x + 15.0, cy), vec2(32.0, 32.0)), &format!("add.{}", name.to_lowercase()), &format!("Add {name}"), icon, false) {
            app.systm.three.exec(code, Some(&format!("Added {name}")));
        }
        x += 33.0;
    }
    x += 12.0;
    let rendered = app.systm.three.rendered;
    for (label, on, value) in [("Solid", !rendered, false), ("Rendered", rendered, true)] {
        let g = p.layout_no_wrap(label.to_string(), Tokens::ui(11.5), INK);
        let r = Rect::from_min_size(pos2(x, cy - 12.0), vec2(g.size().x + 18.0, 24.0));
        let resp = ui.interact(r, egui::Id::new(("systm-3d-shade", label)), Sense::click());
        resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label));
        app.auto.add(&format!("three.shading.{}", label.to_lowercase()), r, label);
        if on {
            p.rect_filled(r, 7.0, super::ICE.gamma_multiply(0.16));
        }
        p.galley(pos2(r.min.x + 9.0, cy - g.size().y / 2.0), g, if on { super::ICE } else { MUTED });
        if resp.clicked() {
            app.systm.three.rendered = value;
            app.systm.three.want_view = true;
        }
        x = r.max.x + 4.0;
    }
    let mut rx = header.max.x - 12.0;
    let mut right_button = |app: &mut SystmCutApp, ui: &mut egui::Ui, rx: &mut f32, id: &str, label: &str, primary: bool| {
        let w = ui.painter().layout_no_wrap(label.to_string(), Tokens::ui(11.5), INK).size().x + 20.0;
        let (c, _) = text_button(app, ui, pos2(*rx - w, cy - 12.0), id, label, primary);
        *rx -= w + 6.0;
        c
    };
    if right_button(app, ui, &mut rx, "to-timeline", "To timeline", true) {
        render_to_timeline(app);
    }
    if right_button(app, ui, &mut rx, "render", "Render image", false) {
        let path = super::data_dir().join("3D").join(format!("render-{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())));
        let st = &mut app.systm.three;
        st.send(json!({"op": "render", "path": path.to_string_lossy(), "engine": "BLENDER_EEVEE"}), Some(format!("Rendered {}", path.display())));
        st.last_render = Some(path);
        st.message = "Rendering…".into();
    }
    if right_button(app, ui, &mut rx, "save", "Save .blend", false) {
        let path = super::home().join("Downloads").join("systm-cut-scene.blend");
        app.systm.three.exec(&format!("bpy.ops.wm.save_as_mainfile(filepath={})", py(&path.to_string_lossy())), Some(&format!("Saved {}", path.display())));
    }
    if right_button(app, ui, &mut rx, "import", "Import…", false) && let Some(path) = choose_file("Import a 3D file (.blend, .glb, .gltf, .fbx, .obj)") {
        let q = py(&path);
        let code = match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
            "blend" => format!("bpy.ops.wm.open_mainfile(filepath={q})"),
            "glb" | "gltf" => format!("bpy.ops.import_scene.gltf(filepath={q})"),
            "fbx" => format!("bpy.ops.import_scene.fbx(filepath={q})"),
            "obj" => format!("bpy.ops.wm.obj_import(filepath={q})"),
            "stl" => format!("bpy.ops.wm.stl_import(filepath={q})"),
            _ => "raise RuntimeError('unsupported 3D file')".into(),
        };
        app.systm.three.exec(&code, Some("Imported"));
    }

    // ---- tools column
    p.rect_filled(tools, 0.0, STRIP);
    p.line_segment([tools.right_top(), tools.right_bottom()], Stroke::new(1.0, LINE));
    for (i, (t, tip, icon, id)) in TOOLS.iter().enumerate() {
        if button(app, ui, Rect::from_min_size(pos2(tools.min.x + 3.0, tools.min.y + 8.0 + i as f32 * 40.0), vec2(40.0, 38.0)), &format!("tool.{id}"), tip, *icon, tool == *t) {
            app.systm.three.tool = *t;
        }
    }
    if resp.hovered() {
        for (key, t) in [(egui::Key::W, Tool::Select), (egui::Key::G, Tool::Move), (egui::Key::R, Tool::Rotate), (egui::Key::S, Tool::Scale)] {
            if ui.input(|i| i.key_pressed(key) && !i.modifiers.any()) {
                app.systm.three.tool = t;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::X) || i.key_pressed(egui::Key::Delete)) && let Some(o) = &active {
            app.systm.three.exec(&format!("bpy.data.objects.remove(D.objects[{}], do_unlink=True)", py(&o.name)), Some("Deleted"));
        }
    }

    // ---- right: outliner over properties
    p.rect_filled(right, 0.0, SURFACE);
    p.line_segment([right.left_top(), right.left_bottom()], Stroke::new(1.0, LINE));
    let out_h = (right.height() * 0.4).max(120.0);
    let outliner = Rect::from_min_size(right.min, vec2(right.width(), out_h));
    let head = Rect::from_min_size(outliner.min, vec2(outliner.width(), 28.0));
    p.rect_filled(head, 0.0, STRIP);
    p.text(pos2(head.min.x + 12.0, head.center().y), egui::Align2::LEFT_CENTER, "Scene", Tokens::semibold(11.5), INK);
    p.text(pos2(head.max.x - 12.0, head.center().y), egui::Align2::RIGHT_CENTER, format!("{} objects", objects.len()), Tokens::ui(10.5), FAINT);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(outliner.min.x, head.max.y), outliner.max)));
    egui::ScrollArea::vertical().id_salt("systm-3d-outliner").auto_shrink([false, false]).show(&mut child, |ui| {
        for o in &objects {
            let (row, resp) = ui.allocate_exact_size(vec2(outliner.width(), 28.0), Sense::click());
            let (on, nm) = (o.selected, o.name.clone());
            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, format!("Object {nm}")));
            app.auto.add(&format!("three.object.{}", o.name), row, &o.name);
            let p = ui.painter().clone();
            if o.active {
                p.rect_filled(row, 0.0, super::ICE.gamma_multiply(0.12));
            } else if o.selected || resp.hovered() {
                p.rect_filled(row, 0.0, Color32::from_white_alpha(8));
            }
            let icon = match o.kind.as_str() {
                "LIGHT" => ico::SUN,
                "CAMERA" => ico::MONITOR,
                "FONT" => ico::TYPE,
                "EMPTY" => ico::CIRCLE_DOT,
                _ => ico::PACKAGE,
            };
            ico::paint(&p, pos2(row.min.x + 18.0, row.center().y), 13.0, icon, if o.active { super::ICE } else { MUTED });
            let label: String = o.name.chars().take(((row.width() - 70.0) / 6.8) as usize).collect();
            p.text(pos2(row.min.x + 34.0, row.center().y), egui::Align2::LEFT_CENTER, label, Tokens::ui(12.0), if o.active { INK } else { MUTED });
            let eye = Rect::from_center_size(pos2(row.max.x - 18.0, row.center().y), vec2(26.0, 24.0));
            let er = ui.interact(eye, egui::Id::new(("systm-3d-eye", &o.name)), Sense::click());
            ico::paint(&p, eye.center(), 13.0, if o.visible { ico::EYE } else { ico::EYE_OFF }, if o.visible { MUTED } else { FAINT });
            if er.clicked() {
                app.systm.three.exec(&format!("o=D.objects[{}]; o.hide_viewport=o.hide_render={}", py(&o.name), if o.visible { "True" } else { "False" }), None);
            } else if resp.clicked() {
                let extend = ui.input(|i| i.modifiers.shift);
                app.systm.three.send(json!({"op": "select", "name": o.name, "extend": extend}), None);
            }
        }
    });
    // properties of the active object
    let props = Rect::from_min_max(pos2(right.min.x, outliner.max.y), right.max);
    p.line_segment([props.left_top(), props.right_top()], Stroke::new(1.0, LINE));
    let phead = Rect::from_min_size(props.min, vec2(props.width(), 28.0));
    p.rect_filled(phead, 0.0, STRIP);
    p.text(pos2(phead.min.x + 12.0, phead.center().y), egui::Align2::LEFT_CENTER, active.as_ref().map_or("Properties".to_string(), |o| o.name.chars().take(26).collect()), Tokens::semibold(11.5), INK);
    if let Some(o) = &active {
        let name = py(&o.name);
        let mut y = phead.max.y + 8.0;
        let lw = 74.0;
        let mut triple = |app: &mut SystmCutApp, ui: &mut egui::Ui, y: &mut f32, label: &str, v: [f64; 3], speed: f64, attr: &str, deg: bool| {
            ui.painter().text(pos2(props.min.x + lw, *y + 12.0), egui::Align2::RIGHT_CENTER, label, Tokens::ui(11.5), MUTED);
            let w = (props.width() - lw - 28.0) / 3.0;
            let mut vals = v;
            let mut changed = false;
            for k in 0..3 {
                let r = Rect::from_min_size(pos2(props.min.x + lw + 10.0 + k as f32 * (w + 3.0), *y + 1.0), vec2(w, 22.0));
                ui.painter().rect(r, 5.0, WELL, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(3.0, 1.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
                c.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                c.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
                c.style_mut().visuals.override_text_color = Some(INK);
                changed |= c.add(egui::DragValue::new(&mut vals[k]).speed(speed).fixed_decimals(2)).changed();
            }
            if changed {
                let f = if deg { "math.radians" } else { "" };
                app.systm.three.exec(&format!("D.objects[{name}].{attr}=({f}({}), {f}({}), {f}({}))", vals[0], vals[1], vals[2]), None);
            }
            *y += 27.0;
        };
        triple(app, ui, &mut y, "Location", o.loc, 0.02, "location", false);
        triple(app, ui, &mut y, "Rotation", o.rot, 0.5, "rotation_euler", true);
        triple(app, ui, &mut y, "Scale", o.scale, 0.01, "scale", false);
        y += 8.0;
        if o.kind == "MESH" || o.kind == "FONT" || o.kind == "CURVE" {
            p.text(pos2(props.min.x + 12.0, y + 6.0), egui::Align2::LEFT_CENTER, "MATERIAL", Tokens::semibold(9.5), FAINT);
            y += 20.0;
            for (i, sw) in SWATCHES.iter().enumerate() {
                let r = Rect::from_center_size(pos2(props.min.x + 24.0 + i as f32 * 27.0, y + 10.0), vec2(22.0, 22.0));
                let resp = ui.interact(r, egui::Id::new(("systm-3d-sw", i)), Sense::click());
                resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Material colour {i}")));
                p.circle_filled(r.center(), 9.5, Color32::from_rgb(sw[0], sw[1], sw[2]));
                p.circle_stroke(r.center(), 9.5, Stroke::new(1.0, Color32::from_white_alpha(if resp.hovered() { 160 } else { 40 })));
                if resp.clicked() {
                    // sRGB swatch to the linear colour Blender's shader wants
                    let lin = |c: u8| (c as f64 / 255.0).powf(2.2);
                    app.systm.three.exec(&format!("o=D.objects[{name}]\nm=o.active_material\nif m is None:\n    m=bpy.data.materials.new('SYSTM'); o.data.materials.append(m)\nm.use_nodes=True\nb=m.node_tree.nodes.get('Principled BSDF')\nb.inputs['Base Color'].default_value=({}, {}, {}, 1)", lin(sw[0]), lin(sw[1]), lin(sw[2])), None);
                }
            }
            y += 30.0;
            let mut x = props.min.x + 10.0;
            for (id, label, code) in [("metal", "Metal", "b.inputs['Metallic'].default_value=1; b.inputs['Roughness'].default_value=0.25"), ("matte", "Matte", "b.inputs['Metallic'].default_value=0; b.inputs['Roughness'].default_value=0.85"), ("glass", "Glass", "b.inputs['Transmission Weight'].default_value=1; b.inputs['Roughness'].default_value=0.05"), ("glow", "Glow", "b.inputs['Emission Color'].default_value=b.inputs['Base Color'].default_value; b.inputs['Emission Strength'].default_value=4")] {
                let (c, nx) = text_button(app, ui, pos2(x, y), &format!("mat.{id}"), label, false);
                if c {
                    app.systm.three.exec(&format!("o=D.objects[{name}]\nm=o.active_material\nif m is None:\n    m=bpy.data.materials.new('SYSTM'); o.data.materials.append(m)\nm.use_nodes=True\nb=m.node_tree.nodes.get('Principled BSDF')\n{code}"), None);
                }
                x = nx;
            }
            y += 34.0;
            p.text(pos2(props.min.x + 12.0, y + 6.0), egui::Align2::LEFT_CENTER, "MODIFIERS", Tokens::semibold(9.5), FAINT);
            if !o.modifiers.is_empty() {
                p.text(pos2(props.max.x - 12.0, y + 6.0), egui::Align2::RIGHT_CENTER, o.modifiers.join(", ").to_lowercase(), Tokens::ui(10.0), MUTED);
            }
            y += 20.0;
            let mut x = props.min.x + 10.0;
            for (id, label, code) in [("subsurf", "Subdivide", "o.modifiers.new('Subdivision','SUBSURF').levels=2"), ("bevel", "Bevel", "m=o.modifiers.new('Bevel','BEVEL'); m.width=0.05; m.segments=3"), ("solid", "Solidify", "o.modifiers.new('Solidify','SOLIDIFY').thickness=0.1")] {
                let (c, nx) = text_button(app, ui, pos2(x, y), &format!("mod.{id}"), label, false);
                if c {
                    app.systm.three.exec(&format!("o=D.objects[{name}]; {code}"), None);
                }
                x = nx;
            }
            y += 34.0;
        }
        let mut x = props.min.x + 10.0;
        for (id, label, code) in [("dup", "Duplicate", format!("o=D.objects[{name}]; n=o.copy(); n.data=o.data.copy() if o.data else None; C.collection.objects.link(n); n.location[0]+=1.5")), ("smooth", "Smooth", format!("for q in D.objects[{name}].data.polygons: q.use_smooth=True")), ("del", "Delete", format!("bpy.data.objects.remove(D.objects[{name}], do_unlink=True)"))] {
            let (c, nx) = text_button(app, ui, pos2(x, y), &format!("obj.{id}"), label, false);
            if c {
                app.systm.three.exec(&code, None);
            }
            x = nx;
        }
    } else {
        p.text(props.center(), egui::Align2::CENTER_CENTER, "Click an object", Tokens::ui(13.0), FAINT);
    }

    // ---- frame strip: where in time the scene is, and a keyframe for the active object
    p.rect_filled(strip, 0.0, STRIP);
    p.line_segment([strip.left_top(), strip.right_top()], Stroke::new(1.0, LINE));
    let sc = app.systm.three.scene.clone();
    let g = |k: &str, d: f64| sc.get(k).and_then(Value::as_f64).unwrap_or(d);
    let (frame, f0, f1) = (g("frame", 1.0), g("frame_start", 1.0), g("frame_end", 250.0));
    let sy = strip.center().y;
    p.text(pos2(strip.min.x + 14.0, sy), egui::Align2::LEFT_CENTER, format!("Frame {frame:.0}"), egui::FontId::monospace(11.5), super::ICE);
    let track = Rect::from_min_max(pos2(strip.min.x + 110.0, sy - 2.0), pos2(strip.max.x - 250.0, sy + 2.0));
    p.rect_filled(track, 2.0, LINE);
    let k = ((frame - f0) / (f1 - f0).max(1.0)).clamp(0.0, 1.0) as f32;
    let tr = ui.interact(track.expand2(vec2(4.0, 10.0)), egui::Id::new("systm-3d-frame"), Sense::click_and_drag());
    tr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Frame"));
    p.circle_filled(pos2(track.min.x + track.width() * k, sy), 5.5, super::ICE);
    if let Some(pt) = tr.interact_pointer_pos().filter(|_| tr.dragged() || tr.clicked()) {
        let f = (f0 + ((pt.x - track.min.x) / track.width()).clamp(0.0, 1.0) as f64 * (f1 - f0)).round();
        if (f - frame).abs() >= 1.0 && app.systm.three.waiting.is_empty() {
            app.systm.three.exec(&format!("C.scene.frame_set({f})"), None);
        }
    }
    let (key, _) = text_button(app, ui, pos2(strip.max.x - 236.0, sy - 12.0), "keyframe", "Insert keyframe", false);
    if key && let Some(o) = &active {
        app.systm.three.exec(&format!("o=D.objects[{}]\nfor a in ('location','rotation_euler','scale'): o.keyframe_insert(a)", py(&o.name)), Some("Keyframe set"));
    }
    if !app.systm.three.message.is_empty() {
        let msg: String = app.systm.three.message.chars().rev().take(46).collect::<String>().chars().rev().collect();
        p.text(pos2(strip.max.x - 12.0, sy), egui::Align2::RIGHT_CENTER, msg, Tokens::ui(10.5), Color32::from_rgb(0xff, 0xd4, 0x8a));
    }
}
