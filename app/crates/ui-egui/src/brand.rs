//! The SYSTM Cut wordmark, drawn as type (no image files).

/// Draw the wordmark `height` points tall with its left edge at `left_center` (vertically centred).
/// Returns the drawn rect.
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, dark: bool) -> Option<egui::Rect> {
    let ink = if dark { egui::Color32::from_rgb(0xF2, 0xF5, 0xF7) } else { egui::Color32::from_rgb(0x0E, 0x10, 0x12) };
    let font = egui::FontId::new(height, egui::FontFamily::Name("display".into()));
    let r = ui.painter().text(left_center, egui::Align2::LEFT_CENTER, "SYSTM CUT", font, ink);
    Some(r)
}
