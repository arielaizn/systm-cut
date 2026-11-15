//! The SYSTM sound bank (`~/content-os/sound/index.json`): browse SFX and music by category,
//! listen, and add a sound to the project.

use std::path::PathBuf;
use std::process::{Child, Command};

use egui::{Color32, RichText};
use serde_json::{Value, json};

use super::ui_kit;
use crate::SystmCutApp;

pub struct Sound {
    pub id: String,
    pub file: PathBuf,
    pub category: String,
    pub tags: String,
    pub dur: f64,
}

#[derive(Default)]
pub struct BankState {
    sounds: Vec<Sound>,
    cats: Vec<String>,
    loaded: bool,
    cat: String,
    search: String,
    playing: Option<(String, Child)>,
}

fn load() -> Vec<Sound> {
    let root = super::content_os().join("sound");
    let v: Value = std::fs::read_to_string(root.join("index.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| {
            Some(Sound {
                id: x.get("id")?.as_str()?.to_string(),
                file: root.join(x.get("file")?.as_str()?),
                category: x.get("category").and_then(Value::as_str).unwrap_or("").to_string(),
                tags: x.get("tags").and_then(Value::as_array).map(|t| t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default(),
                dur: x.get("dur").and_then(Value::as_f64).unwrap_or(0.0),
            })
        })
        .collect()
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: egui::Rect) {
    let dim = Color32::from_rgb(0x7d, 0x8a, 0x91);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)));
    let ui = &mut child;
    let st = &mut app.systm.bank;
    if !st.loaded {
        st.sounds = load();
        st.cats = st.sounds.iter().map(|s| s.category.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        st.loaded = true;
    }
    if let Some((_, c)) = &mut st.playing
        && matches!(c.try_wait(), Ok(Some(_)))
    {
        st.playing = None;
    }
    let mut import: Option<PathBuf> = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("SOUND BANK").small().strong().color(dim));
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search").desired_width(140.0));
        if ui_kit::ghost(ui, "Reload").clicked() {
            st.loaded = false;
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui_kit::pill(ui, "all", st.cat.is_empty()).clicked() {
            st.cat.clear();
        }
        for c in st.cats.clone() {
            if ui_kit::pill(ui, &c, st.cat == c).clicked() {
                st.cat = c;
            }
        }
    });
    if st.sounds.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new("The sound bank was not found (~/content-os/sound/index.json).").color(dim));
        return;
    }
    let q = st.search.to_lowercase();
    let mut play: Option<(String, PathBuf)> = None;
    let mut stop = false;
    egui::ScrollArea::vertical().id_salt("systm-bank").auto_shrink([false, false]).show(ui, |ui| {
        for s in st.sounds.iter().filter(|s| (st.cat.is_empty() || s.category == st.cat) && (q.is_empty() || s.id.contains(&q) || s.tags.contains(&q))) {
            ui.horizontal(|ui| {
                let on = st.playing.as_ref().is_some_and(|(id, _)| id == &s.id);
                if ui_kit::pill(ui, if on { "Stop" } else { "Play" }, on).clicked() {
                    if on { stop = true } else { play = Some((s.id.clone(), s.file.clone())) }
                }
                if ui_kit::ghost(ui, "Add").clicked() {
                    import = Some(s.file.clone());
                }
                ui.label(&s.id);
                ui.label(RichText::new(format!("{}  {:.1}s  {}", s.category, s.dur, s.tags)).small().color(dim));
            });
        }
    });
    if stop || play.is_some() {
        if let Some((_, mut c)) = st.playing.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    if let Some((id, file)) = play
        && let Ok(c) = Command::new("afplay").arg(file).spawn()
    {
        st.playing = Some((id, c));
    }
    if let Some(v) = import {
        match app.session.execute("file.import", json!({"paths": [v.to_string_lossy()]})) {
            Ok(_) => app.status(format!("Added {}", v.file_name().unwrap_or_default().to_string_lossy())),
            Err(e) => app.status(format!("Could not add the sound: {e}")),
        }
    }
}
