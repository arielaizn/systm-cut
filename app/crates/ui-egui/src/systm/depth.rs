//! SYSTM Depth as a panel: the 3D-layer edit engine (`~/content-os/engine/depth/depth.mjs`: Blender,
//! Apple Vision matte, Codex image layers) run from inside the editor, one step or the whole way,
//! and the finished video brought into the project.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use egui::{Color32, RichText};
use serde_json::{Value, json};

use super::ui_kit;
use crate::SystmCutApp;

pub struct Proj {
    pub id: String,
    pub dir: PathBuf,
    pub look: String,
    pub idea: String,
    pub beats: usize,
    pub layers: usize,
    pub frames: usize,
    pub video: Option<PathBuf>,
}

struct Job {
    child: Child,
    rx: Receiver<String>,
    what: String,
}

#[derive(Default)]
pub struct DepthState {
    projects: Vec<Proj>,
    scanned: Option<Instant>,
    selected: Option<String>,
    job: Option<Job>,
    log: Vec<String>,
}

fn count(dir: &std::path::Path, ext: &str) -> usize {
    std::fs::read_dir(dir).map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == ext)).count()).unwrap_or(0)
}

fn scan() -> Vec<Proj> {
    let root = super::content_os().join("projects");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let dir = e.path();
        let d = dir.join("edit").join("depth");
        if !d.join("src.mp4").is_file() {
            continue;
        }
        let plan: Value = std::fs::read_to_string(d.join("plan.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        let mut videos: Vec<PathBuf> = std::fs::read_dir(&d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("depth-v") && n.ends_with(".mp4")))
            .collect();
        videos.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        out.push(Proj {
            id: e.file_name().to_string_lossy().to_string(),
            look: plan.get("look").and_then(Value::as_str).unwrap_or("").to_string(),
            idea: plan.pointer("/concept/idea").and_then(Value::as_str).unwrap_or("").to_string(),
            beats: plan.get("beats").and_then(Value::as_array).map_or(0, Vec::len),
            layers: count(&d.join("layers"), "png"),
            frames: count(&d.join("frames").join("world"), "png"),
            video: videos.pop(),
            dir,
        });
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

impl DepthState {
    /// The step that is running, for the top bar.
    pub fn running(&self) -> Option<&str> {
        self.job.as_ref().map(|j| j.what.as_str())
    }

    /// Read the running step's output. Returns true while a step runs.
    pub fn poll(&mut self) -> bool {
        let Some(job) = &mut self.job else { return false };
        while let Ok(l) = job.rx.try_recv() {
            self.log.push(l);
        }
        if self.log.len() > 400 {
            let cut = self.log.len() - 400;
            self.log.drain(..cut);
        }
        if let Ok(Some(st)) = job.child.try_wait() {
            let what = job.what.clone();
            self.log.push(if st.success() { format!("{what}: done") } else { format!("{what}: failed ({st})") });
            self.job = None;
            self.scanned = None;
        }
        self.job.is_some()
    }

    fn run(&mut self, id: &str, step: &[&str]) {
        let engine = super::content_os().join("engine").join("depth").join("depth.mjs");
        let Some(node) = super::which("node") else {
            self.log.push("node is not installed".into());
            return;
        };
        let mut cmd = Command::new(node);
        cmd.arg(&engine).arg(step[0]).arg(id).args(&step[1..]).env("PATH", super::child_path()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        match cmd.spawn() {
            Ok(mut child) => {
                let (tx, rx) = channel();
                for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        for l in BufReader::new(pipe).lines().map_while(Result::ok) {
                            if tx.send(l).is_err() {
                                break;
                            }
                        }
                    });
                }
                self.log.push(format!("{} {id}: started", step.join(" ")));
                self.job = Some(Job { child, rx, what: format!("{} {id}", step[0]) });
            }
            Err(e) => self.log.push(format!("could not start the engine: {e}")),
        }
    }
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: egui::Rect) {
    let dim = Color32::from_rgb(0x7d, 0x8a, 0x91);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)));
    let ui = &mut child;
    if app.systm.depth.scanned.is_none_or(|t| t.elapsed().as_secs() > 5) {
        app.systm.depth.projects = scan();
        app.systm.depth.scanned = Some(Instant::now());
    }
    let busy = app.systm.depth.job.is_some();
    let mut run: Option<(String, Vec<&'static str>)> = None;
    let mut import: Option<PathBuf> = None;
    let mut reveal: Option<PathBuf> = None;
    let st = &mut app.systm.depth;
    ui.horizontal(|ui| {
        ui.label(RichText::new("SYSTM DEPTH").small().strong().color(dim));
        if let Some(j) = &st.job {
            ui.spinner();
            ui.label(RichText::new(&j.what).small().color(super::ICE));
        }
    });
    if st.projects.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new("No depth projects yet.").color(dim));
        ui.label(RichText::new("A project appears here after `depth.mjs init` in ~/content-os/projects.").small().color(dim));
        return;
    }
    let log_h = 110.0;
    let list_h = (ui.available_height() - log_h - 8.0).max(60.0);
    egui::ScrollArea::vertical().id_salt("systm-depth-list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        for p in &st.projects {
            let open = st.selected.as_deref() == Some(p.id.as_str());
            let title = RichText::new(&p.id).strong().color(if open { super::ICE } else { Color32::from_rgb(0xdd, 0xe4, 0xe7) });
            if ui.add(egui::Label::new(title).sense(egui::Sense::click())).clicked() {
                st.selected = if open { None } else { Some(p.id.clone()) };
            }
            let state = if p.video.is_some() {
                "finished"
            } else if p.frames > 0 {
                "rendering"
            } else if p.layers > 0 {
                "layers painted"
            } else if p.beats > 0 {
                "planned"
            } else {
                "new"
            };
            ui.label(RichText::new(format!("{state}  ·  {}  ·  {} beats  ·  {} layers  ·  {} frames", if p.look.is_empty() { "no look" } else { &p.look }, p.beats, p.layers, p.frames)).small().color(dim));
            if open {
                if !p.idea.is_empty() {
                    ui.label(RichText::new(&p.idea).small().italics());
                }
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        for (label, step) in [("Plan", vec!["autoplan"]), ("Layers", vec!["layers"]), ("Text", vec!["text"]), ("Render", vec!["render", "--resume"]), ("Finish", vec!["finish"])] {
                            if ui_kit::ghost(ui, label).clicked() {
                                run = Some((p.id.clone(), step));
                            }
                        }
                        if ui_kit::primary(ui, "Run all").clicked() {
                            run = Some((p.id.clone(), vec!["run"]));
                        }
                        if ui_kit::ghost(ui, "Open in Blender").clicked() {
                            run = Some((p.id.clone(), vec!["open"]));
                        }
                    });
                    if let Some(v) = &p.video {
                        if ui_kit::primary(ui, "Add to project").clicked() {
                            import = Some(v.clone());
                        }
                        if ui_kit::ghost(ui, "Show file").clicked() {
                            reveal = Some(v.clone());
                        }
                    }
                });
            }
            ui.add_space(6.0);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("systm-depth-log").max_height(log_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        for l in st.log.iter().rev().take(60).collect::<Vec<_>>().into_iter().rev() {
            ui.label(RichText::new(l).small().monospace().color(dim));
        }
    });
    if let Some((id, step)) = run {
        app.systm.depth.run(&id, &step);
    }
    if let Some(v) = reveal {
        let _ = Command::new("open").arg("-R").arg(v).spawn();
    }
    if let Some(v) = import {
        match app.session.execute("file.import", json!({"paths": [v.to_string_lossy()]})) {
            Ok(_) => app.status(format!("Added {}", v.file_name().unwrap_or_default().to_string_lossy())),
            Err(e) => app.status(format!("Could not add the video: {e}")),
        }
    }
}
