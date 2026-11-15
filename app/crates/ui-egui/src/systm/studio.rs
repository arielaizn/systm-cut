//! Studio: the companion tools beside the editor (image, motion, 3D, audio, finishing). Each opens
//! as its own app; agents can drive them with their own cursors through agent-desktop.

use std::path::PathBuf;
use std::process::Command;

use egui::{Color32, RichText};

use super::ui_kit;
use crate::SystmCutApp;

#[derive(Default)]
pub struct StudioState {
    note: String,
}

struct Tool {
    name: &'static str,
    role: &'static str,
    /// App bundle names to look for, first match wins.
    apps: &'static [&'static str],
}

const TOOLS: &[Tool] = &[
    Tool { name: "3D", role: "Scenes, depth layers, camera moves", apps: &["Blender"] },
    Tool { name: "Image", role: "Stills, mattes, retouching", apps: &["PhotoCraft", "Affinity Photo 2", "Adobe Photoshop 2026", "Pixelmator Pro", "Preview"] },
    Tool { name: "Motion", role: "Compositing and motion graphics", apps: &["EffectCraft", "DaVinci Resolve", "Adobe After Effects 2026"] },
    Tool { name: "Audio", role: "Mixing, cleanup, sound design", apps: &["SoundCraft", "Logic Pro", "Audacity", "GarageBand"] },
    Tool { name: "Finishing", role: "Colour and delivery", apps: &["DaVinci Resolve"] },
];

fn find(app: &str) -> Option<PathBuf> {
    let home = super::home();
    [PathBuf::from("/Applications"), home.join("Applications"), PathBuf::from("/System/Applications"), PathBuf::from("/Applications/DaVinci Resolve")]
        .into_iter()
        .map(|d| d.join(format!("{app}.app")))
        .find(|p| p.exists())
}

pub fn show(app: &mut SystmCutApp, ui: &mut egui::Ui, rect: egui::Rect) {
    let dim = Color32::from_rgb(0x7d, 0x8a, 0x91);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)));
    let ui = &mut child;
    ui.label(RichText::new("STUDIO").small().strong().color(dim));
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("systm-studio").auto_shrink([false, false]).show(ui, |ui| {
        for t in TOOLS {
            let found = t.apps.iter().find_map(|a| find(a).map(|p| (*a, p)));
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t.name).strong());
                    ui.label(RichText::new(t.role).small().color(dim));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &found {
                    Some((name, path)) => {
                        if ui_kit::primary(ui, &format!("Open {name}")).clicked() {
                            app.systm.studio.note = match Command::new("open").arg(path).spawn() {
                                Ok(_) => format!("Opened {name}"),
                                Err(e) => format!("Could not open {name}: {e}"),
                            };
                        }
                    }
                    None => {
                        ui.label(RichText::new("not installed").small().color(dim));
                    }
                });
            });
            ui.add_space(8.0);
        }
        if !app.systm.studio.note.is_empty() {
            ui.label(RichText::new(&app.systm.studio.note).small().color(super::ICE));
        }
    });
}
