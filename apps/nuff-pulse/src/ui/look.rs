//! Colours and the state ring, shared by the bar and the app window.

use eframe::egui::{self, epaint::PathShape, Color32, Pos2, Stroke, Ui};
use nuff_pulse::model::{Display, Level};

pub fn state_color(ui: &Ui, s: Display) -> Color32 {
    let dark = ui.visuals().dark_mode;
    match s {
        Display::Working => if dark { Color32::from_rgb(0x60, 0xA5, 0xFA) } else { Color32::from_rgb(0x25, 0x63, 0xEB) },
        Display::NeedsYou => if dark { Color32::from_rgb(0xFB, 0xBF, 0x24) } else { Color32::from_rgb(0xD9, 0x77, 0x06) },
        Display::Done => if dark { Color32::from_rgb(0x4A, 0xDE, 0x80) } else { Color32::from_rgb(0x16, 0xA3, 0x4A) },
        Display::Error => if dark { Color32::from_rgb(0xF8, 0x71, 0x71) } else { Color32::from_rgb(0xDC, 0x26, 0x26) },
        Display::Idle => ui.visuals().weak_text_color(),
        Display::Ended => ui.visuals().weak_text_color().gamma_multiply(0.6),
    }
}

pub fn level_color(ui: &Ui, l: Level) -> Color32 {
    let dark = ui.visuals().dark_mode;
    match l {
        Level::Ok => ui.visuals().text_color(),
        Level::Amber => if dark { Color32::from_rgb(0xFB, 0xBF, 0x24) } else { Color32::from_rgb(0xB4, 0x53, 0x09) },
        Level::Red => if dark { Color32::from_rgb(0xF8, 0x71, 0x71) } else { Color32::from_rgb(0xB9, 0x1C, 0x1C) },
    }
}

/// Cache countdown colour: green above 15 min, amber below, red when cold.
pub fn cache_color(ui: &Ui, l: Level) -> Color32 {
    match l {
        Level::Ok => state_color(ui, Display::Done),
        other => level_color(ui, other),
    }
}

/// The state ring: filled while it alerts, an outline otherwise; a turning
/// arc while working (`t` is the animation time in seconds).
pub fn ring(ui: &Ui, center: Pos2, r: f32, s: Display, alert: bool, t: f64) {
    let c = state_color(ui, s);
    let p = ui.painter();
    match s {
        Display::Working => {
            p.circle_stroke(center, r, Stroke::new(r * 0.3, c.gamma_multiply(0.25)));
            let start = (t * std::f64::consts::TAU / 1.2) as f32;
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = start + i as f32 / 16.0 * std::f32::consts::PI * 1.2;
                    center + egui::vec2(a.cos(), a.sin()) * r
                })
                .collect();
            p.add(PathShape::line(pts, Stroke::new(r * 0.3, c)));
        }
        _ if alert => {
            p.circle_filled(center, r * 1.05, c);
        }
        Display::Ended => {
            p.circle_stroke(center, r * 0.8, Stroke::new(r * 0.2, c));
        }
        _ => {
            p.circle_stroke(center, r, Stroke::new(r * 0.3, c));
        }
    }
}
