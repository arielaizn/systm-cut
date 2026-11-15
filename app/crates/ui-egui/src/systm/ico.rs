//! Lucide icons (ISC licence, assets/fonts/lucide.LICENSE), as glyphs of the `icons` font.

use egui::{Color32, FontFamily, FontId};

pub const MOUSE_POINTER_2: char = '\u{e1c3}';
pub const SCISSORS: char = '\u{e14e}';
pub const MAGNET: char = '\u{e2b5}';
pub const LINK: char = '\u{e102}';
pub const LOCK: char = '\u{e10b}';
pub const LOCK_OPEN: char = '\u{e10c}';
pub const EYE: char = '\u{e0ba}';
pub const EYE_OFF: char = '\u{e0bb}';
pub const VOLUME_2: char = '\u{e1ab}';
pub const VOLUME_X: char = '\u{e1ac}';
pub const ZOOM_IN: char = '\u{e1b6}';
pub const ZOOM_OUT: char = '\u{e1b7}';
pub const PLAY: char = '\u{e13c}';
pub const PAUSE: char = '\u{e12e}';
pub const SQUARE: char = '\u{e167}';
pub const SKIP_BACK: char = '\u{e15f}';
pub const SKIP_FORWARD: char = '\u{e160}';
pub const STEP_BACK: char = '\u{e3e9}';
pub const STEP_FORWARD: char = '\u{e3ea}';
pub const REPEAT: char = '\u{e146}';
pub const CHEVRON_DOWN: char = '\u{e06d}';
pub const CHEVRON_RIGHT: char = '\u{e06f}';
pub const CHEVRON_LEFT: char = '\u{e06e}';
pub const PLUS: char = '\u{e13d}';
pub const MINUS: char = '\u{e11c}';
pub const X: char = '\u{e1b2}';
pub const SEARCH: char = '\u{e151}';
pub const FOLDER: char = '\u{e0d7}';
pub const FOLDER_OPEN: char = '\u{e247}';
pub const FILM: char = '\u{e0d0}';
pub const MUSIC: char = '\u{e122}';
pub const IMAGE: char = '\u{e0f6}';
pub const TYPE: char = '\u{e198}';
pub const CAPTIONS: char = '\u{e3a4}';
pub const SLIDERS_HORIZONTAL: char = '\u{e29a}';
pub const SETTINGS: char = '\u{e154}';
pub const HOUSE: char = '\u{e0f5}';
pub const LAYERS: char = '\u{e529}';
pub const PALETTE: char = '\u{e1dd}';
pub const AUDIO_LINES: char = '\u{e55a}';
pub const DOWNLOAD: char = '\u{e0b2}';
pub const WAND_SPARKLES: char = '\u{e357}';
pub const SPARKLES: char = '\u{e412}';
pub const CIRCLE: char = '\u{e076}';
pub const DIAMOND: char = '\u{e2d2}';
pub const ROTATE_CCW: char = '\u{e148}';
pub const FLAG: char = '\u{e0d1}';
pub const TRASH_2: char = '\u{e18e}';
pub const COPY: char = '\u{e09e}';
pub const PENCIL: char = '\u{e1f9}';
pub const HISTORY: char = '\u{e1f5}';
pub const MAXIMIZE_2: char = '\u{e113}';
pub const MINIMIZE_2: char = '\u{e11b}';
pub const PAPERCLIP: char = '\u{e12d}';
pub const ARROW_UP: char = '\u{e04a}';
pub const WRENCH: char = '\u{e1b1}';
pub const GRID_3X3: char = '\u{e0e9}';
pub const LIST: char = '\u{e106}';
pub const CLAPPERBOARD: char = '\u{e29b}';
pub const BLEND: char = '\u{e59c}';
pub const GIT_BRANCH: char = '\u{e0e2}';
pub const GAUGE: char = '\u{e1bf}';
pub const MIC: char = '\u{e118}';
pub const HEADPHONES: char = '\u{e0f1}';
pub const UNDO_2: char = '\u{e2a1}';
pub const REDO_2: char = '\u{e2a0}';
pub const MOVE: char = '\u{e121}';
pub const CROP: char = '\u{e0ab}';
pub const TRIANGLE_RIGHT: char = '\u{e4ed}';
pub const HAND: char = '\u{e1d7}';
pub const TEXT_CURSOR: char = '\u{e264}';
pub const SPLIT: char = '\u{e440}';
pub const ALIGN_HORIZONTAL_SPACE_AROUND: char = '\u{e275}';
pub const STAR: char = '\u{e176}';
pub const LIBRARY: char = '\u{e100}';
pub const PANEL_LEFT: char = '\u{e12a}';
pub const PANEL_RIGHT: char = '\u{e431}';
pub const MONITOR: char = '\u{e11d}';
pub const SCAN_LINE: char = '\u{e258}';
pub const DROPLET: char = '\u{e0b4}';
pub const SUN: char = '\u{e178}';
pub const CONTRAST: char = '\u{e09d}';
pub const WAYPOINTS: char = '\u{e542}';
pub const WORKFLOW: char = '\u{e425}';
pub const PACKAGE: char = '\u{e129}';
pub const PUZZLE: char = '\u{e29c}';
pub const USERS: char = '\u{e1a4}';
pub const BOT: char = '\u{e1bb}';
pub const SEND: char = '\u{e152}';
pub const CHECK: char = '\u{e06c}';
pub const INFO: char = '\u{e0f9}';
pub const CIRCLE_DOT: char = '\u{e345}';
pub const MONITOR_PLAY: char = '\u{e485}';
pub const TIMER: char = '\u{e1e0}';
pub const ROWS_3: char = '\u{e58a}';
pub const COLUMNS_3: char = '\u{e099}';
pub const GALLERY_HORIZONTAL_END: char = '\u{e4cf}';

/// The icon font at `size`.
pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

/// Paint an icon centred on `c`.
pub fn paint(p: &egui::Painter, c: egui::Pos2, size: f32, icon: char, col: Color32) {
    p.text(c, egui::Align2::CENTER_CENTER, icon, font(size), col);
}
