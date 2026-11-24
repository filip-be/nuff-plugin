//! The floating bar: a pill (counts and the most urgent cache countdown) or
//! a list with one row per active session. Dragging the background moves
//! it; clicking a row clears its alert and focuses its window.

use super::look;
use super::{open_row, Shared};
use eframe::egui::{self, Align, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2, ViewportCommand};
use nuff_pulse::fmt;
use nuff_pulse::model::{Display, Row};
use nuff_pulse::settings::BarMode;

const ROW_H: f32 = 42.0;
const HEADER_H: f32 = 24.0;
const PAD: f32 = 6.0;
const MAX_ROWS: usize = 8;
const PILL_H: f32 = 30.0;

pub struct BarOut {
    /// Size the bar window should have.
    pub size: Vec2,
    pub toggle_mode: bool,
    pub open_app: bool,
}

fn frame(ui: &Ui) -> egui::Frame {
    egui::Frame::NONE
        .fill(ui.visuals().panel_fill)
        .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
        .inner_margin(PAD)
}

/// Starts a window drag when the background is dragged.
fn drag_background(ui: &Ui, resp: &egui::Response) {
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

pub fn show(ui: &mut Ui, shared: &Shared, rows: &[Row], now: f64, mode: BarMode) -> BarOut {
    let active: Vec<&Row> = rows.iter().filter(|r| r.active).collect();
    let mut out = BarOut { size: Vec2::ZERO, toggle_mode: false, open_app: false };
    let t = ui.input(|i| i.time);
    let full = ui.max_rect();
    // The whole window is a drag handle; widgets on top take their clicks.
    let bg = ui.interact(full, ui.scope_id().with("bar-bg"), Sense::click_and_drag());
    drag_background(ui, &bg);
    frame(ui).show(ui, |ui| match mode {
        BarMode::Pill => {
            out.size = pill(ui, &active, now, t, &bg, &mut out.toggle_mode);
        }
        BarMode::List => {
            let h = list(ui, shared, &active, now, t, &mut out);
            out.size = Vec2::new(super::BAR_WIDTH, (h + 2.0 * PAD + 2.0).ceil());
        }
    });
    out
}

fn pill(ui: &mut Ui, active: &[&Row], now: f64, t: f64, bg: &egui::Response, toggle: &mut bool) -> Vec2 {
    let s = fmt::summary(&active.iter().map(|r| (*r).clone()).collect::<Vec<_>>(), now);
    let resp = ui
        .horizontal(|ui| {
            ui.set_height(PILL_H - 2.0 * PAD);
            let lead = active.first().map_or(Display::Idle, |r| r.state);
            let alert = active.first().is_some_and(|r| r.alert);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            let state = if s.working > 0 && !alert { Display::Working } else { lead };
            look::ring(ui, rect.center(), 5.5, state, alert, t);
            let mut any = false;
            for (n, st) in [(s.needs_you, Display::NeedsYou), (s.error, Display::Error), (s.done, Display::Done), (s.working, Display::Working)] {
                if n > 0 {
                    any = true;
                    ui.label(RichText::new(format!("{n} {}", st.label().to_lowercase())).color(look::state_color(ui, st)).strong());
                }
            }
            if !any {
                let text = if s.active == 0 { "no sessions".to_string() } else { format!("{} idle", s.active) };
                ui.label(RichText::new(text).weak());
            }
            if let Some(left) = s.urgent_cache {
                ui.separator();
                let lvl = nuff_pulse::model::cache_level(left);
                ui.label(RichText::new(format!("cache {}", fmt::left(left))).color(look::cache_color(ui, lvl)));
            }
        })
        .response;
    if bg.clicked() || resp.clicked() {
        *toggle = true;
    }
    let w = resp.rect.width() + 2.0 * PAD + 4.0;
    Vec2::new(w.max(80.0), PILL_H)
}

/// Draws the list; returns the height its content needs (the window may
/// still be smaller than that this frame).
fn list(ui: &mut Ui, shared: &Shared, active: &[&Row], now: f64, t: f64, out: &mut BarOut) -> f32 {
    let width = super::BAR_WIDTH - 2.0 * PAD;
    ui.set_width(width);
    // Header: counts, collapse and open-app buttons.
    let header = ui.horizontal(|ui| {
        ui.set_height(HEADER_H - 4.0);
        ui.label(RichText::new("nuff-pulse").small().strong());
        let s = fmt::summary(&active.iter().map(|r| (*r).clone()).collect::<Vec<_>>(), now);
        if s.alerts > 0 {
            ui.label(RichText::new(format!("{} new", s.alerts)).small().color(look::state_color(ui, Display::NeedsYou)));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("–").on_hover_text("Collapse to pill (hotkey toggles)").clicked() {
                out.toggle_mode = true;
            }
            if ui.small_button("app").on_hover_text("Open the app window").clicked() {
                out.open_app = true;
            }
        });
    });
    let header_h = header.response.rect.height() + ui.spacing().item_spacing.y;
    if active.is_empty() {
        ui.add_space(4.0);
        let r = ui.label(RichText::new("No active sessions").weak());
        return header_h + 4.0 + r.rect.height();
    }
    let shown = active.len().min(MAX_ROWS);
    let rows_h = shown as f32 * ROW_H;
    egui::ScrollArea::vertical().max_height(rows_h).min_scrolled_height(rows_h).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for r in active {
            row(ui, shared, r, now, t);
        }
    });
    header_h + rows_h
}

fn row(ui: &mut Ui, shared: &Shared, r: &Row, now: f64, t: f64) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    if resp.clicked() {
        open_row(shared, r);
    }
    resp.on_hover_text(hover(r));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(4.0, 3.0))).layout(Layout::left_to_right(Align::Center)));
    let ui = &mut child;
    let (ring_rect, _) = ui.allocate_exact_size(Vec2::new(18.0, ROW_H - 6.0), Sense::hover());
    look::ring(ui, ring_rect.center(), 6.0, r.state, r.alert, t);
    let right_w = 150.0;
    let mid_w = (ui.available_width() - right_w - 6.0).max(60.0);
    ui.allocate_ui_with_layout(Vec2::new(mid_w, ROW_H - 6.0), Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let title = RichText::new(&r.title).strong();
        ui.add(egui::Label::new(title).truncate().selectable(false));
        let mut second = r.state.label().to_string();
        if r.cache_expiring {
            second.push_str(" · cache expiring");
        }
        // What it does or waits on; a finished turn's last tool is noise.
        let what = match r.state {
            Display::Working => r.activity.as_ref(),
            Display::NeedsYou | Display::Error => r.detail.as_ref().or(r.reason.as_ref()).or(r.activity.as_ref()),
            _ => None,
        };
        if let Some(a) = what {
            second.push_str(" · ");
            second.push_str(a);
        }
        if r.subagents > 0 {
            second.push_str(&format!(" · {} subagent{}", r.subagents, if r.subagents == 1 { "" } else { "s" }));
        }
        let color = if r.alert { look::state_color(ui, r.state) } else { ui.visuals().weak_text_color() };
        ui.add(egui::Label::new(RichText::new(second).small().color(color)).truncate().selectable(false));
    });
    ui.allocate_ui_with_layout(Vec2::new(right_w, ROW_H - 6.0), Layout::top_down(Align::Max), |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(e) = r.cache_expires {
                    let left = e - now;
                    let lvl = nuff_pulse::model::cache_level(left);
                    ui.label(RichText::new(fmt::left(left)).color(look::cache_color(ui, lvl)).monospace())
                        .on_hover_text("Prompt cache time left");
                }
                let ctx_text = fmt::context(r);
                if !ctx_text.is_empty() {
                    ui.label(RichText::new(ctx_text).color(look::level_color(ui, r.context_level)).monospace())
                        .on_hover_text("Context: tokens / window");
                }
            });
        });
        ui.add(egui::Label::new(RichText::new(fmt::place(r)).small().color(ui.visuals().weak_text_color())).truncate().selectable(false));
    });
}

fn hover(r: &Row) -> String {
    let mut s = format!("{}\n{}", r.title, fmt::place(r));
    if let Some(m) = &r.model {
        s.push_str(&format!("\n{m}"));
    }
    if let Some(p) = &r.last_prompt {
        s.push_str(&format!("\n“{}”", nuff_pulse::util::short(p, 80)));
    }
    s.push_str("\nClick: clear alert and focus window");
    s
}
