//! Workspace ▸ Scripts: plugins for SYSTM Cut, the way DaVinci Resolve lists its scripts.
//!
//! A plugin is a script file (`.py`, `.sh`, `.js`, `.lua`, or anything executable) in a category
//! folder, or a folder with a `plugin.json` (`{"name", "category", "command": [...], "description"}`).
//! They are found in the user's own folder and in the machine-wide one a `.pkg` installs to:
//!
//!   ~/Library/Application Support/SYSTM Cut/Scripts/<Category>/...
//!   /Library/Application Support/SYSTM Cut/Scripts/<Category>/...
//!
//! A plugin runs as its own process and drives the editor through the control channel: it gets
//! `SYSTM_CUT_PORT`, `SYSTM_CUT_TOKEN_FILE` and `SYSTM_CUT_CLI` (the bundled CLI) in its environment.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use egui::{Color32, RichText, Stroke};
use serde_json::Value;

use crate::SystmCutApp;
use crate::theme::Tokens;

#[derive(Clone)]
pub struct Plugin {
    pub name: String,
    pub category: String,
    pub description: String,
    /// The program and its arguments.
    pub command: Vec<String>,
    pub dir: PathBuf,
}

#[derive(Default)]
pub struct ScriptsState {
    pub plugins: Vec<Plugin>,
    scanned: Option<Instant>,
    pub menu_open: bool,
    /// When the menu was opened (the click that opens it must not close it).
    pub opened: f64,
    running: Vec<(String, Receiver<Option<String>>)>,
    pub last: String,
}

pub fn roots() -> Vec<PathBuf> {
    vec![super::data_dir().join("Scripts"), PathBuf::from("/Library/Application Support/SYSTM Cut/Scripts")]
}

fn runner(path: &Path) -> Option<Vec<String>> {
    let p = path.display().to_string();
    let which = |n: &str| super::which(n).map(|x| x.display().to_string());
    Some(match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "py" => vec![which("python3")?, p],
        "sh" => vec!["/bin/bash".into(), p],
        "js" | "mjs" => vec![which("node")?, p],
        "lua" => vec![which("lua")?, p],
        "" | "command" => vec![p],
        _ => return None,
    })
}

fn title(stem: &str) -> String {
    stem.replace(['_', '-'], " ")
}

fn scan() -> Vec<Plugin> {
    let mut out: Vec<Plugin> = Vec::new();
    for root in roots() {
        for cat in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let cdir = cat.path();
            if !cdir.is_dir() {
                continue;
            }
            let category = cat.file_name().to_string_lossy().to_string();
            for e in std::fs::read_dir(&cdir).into_iter().flatten().flatten() {
                let p = e.path();
                let fname = e.file_name().to_string_lossy().to_string();
                if fname.starts_with('.') {
                    continue;
                }
                if p.is_dir() {
                    // a packaged plugin: plugin.json says what to run
                    let Some(m) = std::fs::read_to_string(p.join("plugin.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { continue };
                    let mut command: Vec<String> = m.get("command").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                    if command.is_empty() {
                        continue;
                    }
                    // a relative program or script is inside the plugin's folder
                    for c in command.iter_mut() {
                        if !c.starts_with('/') && p.join(&*c).exists() {
                            *c = p.join(&*c).display().to_string();
                        }
                    }
                    if let Some(first) = command.first().cloned()
                        && !first.starts_with('/')
                        && let Some(w) = super::which(&first)
                    {
                        command[0] = w.display().to_string();
                    }
                    out.push(Plugin {
                        name: m.get("name").and_then(Value::as_str).map_or_else(|| title(&fname), str::to_string),
                        category: m.get("category").and_then(Value::as_str).unwrap_or(&category).to_string(),
                        description: m.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                        command,
                        dir: p,
                    });
                } else if let Some(command) = runner(&p) {
                    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(fname);
                    out.push(Plugin { name: title(&stem), category: category.clone(), description: String::new(), command, dir: cdir.clone() });
                }
            }
        }
    }
    out.sort_by(|a, b| (a.category.to_lowercase(), a.name.to_lowercase()).cmp(&(b.category.to_lowercase(), b.name.to_lowercase())));
    out.dedup_by(|a, b| a.name == b.name && a.category == b.category);
    out
}

impl ScriptsState {
    pub fn refresh(&mut self) {
        if self.scanned.is_none_or(|t| t.elapsed().as_secs() > 4) {
            self.plugins = scan();
            self.scanned = Some(Instant::now());
        }
    }

    /// Start a plugin by name (any case). Returns what happened, in words.
    pub fn run(&mut self, name: &str, control: Option<&super::Control>) -> Result<String, String> {
        self.scanned = None;
        self.refresh();
        let p = self.plugins.iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())).cloned().ok_or_else(|| format!("no script named `{name}`"))?;
        let control = control.ok_or("the control channel is not running")?;
        let mut cmd = Command::new(&p.command[0]);
        cmd.args(&p.command[1..])
            .current_dir(&p.dir)
            .env("PATH", super::child_path())
            .env("SYSTM_CUT_PORT", control.port.to_string())
            .env("SYSTM_CUT_TOKEN_FILE", &control.token_file)
            .env("SYSTMCUT_CONTROL_TOKEN_FILE", &control.token_file)
            .env("SYSTM_AGENT_ID", format!("script:{}", p.name))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cli) = super::bridge_cli() {
            cmd.env("SYSTM_CUT_CLI", cli);
        }
        let mut child = cmd.spawn().map_err(|e| format!("could not start {}: {e}", p.name))?;
        let (tx, rx) = channel();
        let (out, err) = (child.stdout.take(), child.stderr.take());
        std::thread::spawn(move || {
            // the last line a script prints is its message to the user
            let mut last = None;
            if let Some(o) = out {
                for l in BufReader::new(o).lines().map_while(Result::ok) {
                    if !l.trim().is_empty() {
                        last = Some(l);
                    }
                }
            }
            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            if !ok && let Some(e) = err {
                last = BufReader::new(e).lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).last().or(last).map(|l| format!("failed: {l}"));
            }
            let _ = tx.send(last);
        });
        self.running.push((p.name.clone(), rx));
        self.last = format!("{}: running", p.name);
        Ok(self.last.clone())
    }

    /// Collect finished scripts. Returns true while any runs.
    pub fn poll(&mut self) -> bool {
        let mut done = Vec::new();
        self.running.retain(|(name, rx)| match rx.try_recv() {
            Ok(msg) => {
                done.push(format!("{name}: {}", msg.unwrap_or_else(|| "done".into())));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        if let Some(l) = done.pop() {
            self.last = l;
        }
        !self.running.is_empty()
    }

    pub fn list(&mut self) -> Value {
        self.scanned = None;
        self.refresh();
        Value::Array(self.plugins.iter().map(|p| serde_json::json!({"name": p.name, "category": p.category, "description": p.description})).collect())
    }
}

/// The Workspace menu, dropped from its button in the top bar: Scripts by category, as in Resolve.
pub fn menu(app: &mut SystmCutApp, ctx: &egui::Context, anchor: egui::Pos2) {
    if !app.systm.scripts.menu_open {
        return;
    }
    app.systm.scripts.refresh();
    let mut run = None;
    let mut close = false;
    let plugins = app.systm.scripts.plugins.clone();
    let area = egui::Area::new(egui::Id::new("systm-workspace-menu")).order(egui::Order::Foreground).fixed_pos(anchor).show(ctx, |ui| {
        egui::Frame::new().fill(Color32::from_rgb(0x14, 0x1d, 0x21)).stroke(Stroke::new(1.0, Color32::from_rgb(0x2c, 0x39, 0x3f))).corner_radius(egui::CornerRadius::same(14)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(250.0);
            ui.label(RichText::new("WORKSPACE").font(Tokens::semibold(9.5)).color(Color32::from_rgb(0x86, 0x97, 0xa0)));
            ui.add_space(2.0);
            ui.label(RichText::new("Scripts").font(Tokens::semibold(13.0)).color(Color32::from_rgb(0xf1, 0xf5, 0xf6)));
            ui.add_space(4.0);
            if plugins.is_empty() {
                ui.label(RichText::new("No scripts installed yet.").size(12.0).color(Color32::from_rgb(0x86, 0x97, 0xa0)));
            }
            let mut cat = String::new();
            for p in &plugins {
                if p.category != cat {
                    cat = p.category.clone();
                    ui.add_space(4.0);
                    ui.label(RichText::new(cat.to_uppercase()).font(Tokens::semibold(9.0)).color(super::ICE.gamma_multiply(0.8)));
                }
                let b = ui.add(egui::Button::new(RichText::new(&p.name).size(12.5)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(egui::vec2(234.0, 24.0)));
                let b = if p.description.is_empty() { b } else { b.on_hover_text(&p.description) };
                if b.clicked() {
                    run = Some(p.name.clone());
                }
            }
            ui.add_space(6.0);
            ui.separator();
            if ui.add(egui::Button::new(RichText::new("Open Scripts Folder").size(12.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                let dir = super::data_dir().join("Scripts");
                let _ = std::fs::create_dir_all(dir.join("Utility"));
                let _ = Command::new("open").arg(dir).spawn();
                close = true;
            }
        });
    });
    let age = ctx.input(|i| i.time) - app.systm.scripts.opened;
    if (age > 0.25 && area.response.clicked_elsewhere()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if let Some(name) = run {
        let control = app.systm.control.clone();
        let msg = match app.systm.scripts.run(&name, control.as_ref()) {
            Ok(m) | Err(m) => m,
        };
        app.status(msg);
        close = true;
    }
    if close {
        app.systm.scripts.menu_open = false;
    }
}
