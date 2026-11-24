//! The floating bar: a pill (counts and the most urgent cache countdown) or
//! a list with one row per active session. Dragging anywhere on it moves
//! it; clicking the pill toggles the list, clicking a row clears its alert
//! and focuses its window.

use super::look;
use super::{open_row, Shared};
use eframe::egui::{
    self, Align, CornerRadius, CursorIcon, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2,
    ViewportCommand,
};
use nuff_pulse::fmt;
use nuff_pulse::model::{Display, Row};
use nuff_pulse::settings::BarMode;

const ROW_H: f32 = 48.0;
const HEADER_H: f32 = 34.0;
const PAD: f32 = 8.0;
const MAX_ROWS: usize = 8;
const PILL_H: f32 = 40.0;
/// Width of the figures column (context, cache, where).
const FIGURES_W: f32 = 160.0;

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

/// Starts a window drag when `resp` is dragged.
fn drag_window(ui: &Ui, resp: &egui::Response) {
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

pub fn show(ui: &mut Ui, shared: &Shared, rows: &[Row], now: f64, mode: BarMode) -> BarOut {
    // Text on the bar is never selectable: a selectable label takes the
    // press, so dragging or clicking on text would do nothing.
    ui.style_mut().interaction.selectable_labels = false;
    let active: Vec<&Row> = rows.iter().filter(|r| r.active).collect();
    let mut out = BarOut { size: Vec2::ZERO, toggle_mode: false, open_app: false };
    let t = ui.input(|i| i.time);
    let full = ui.max_rect();
    // The whole window is a drag handle; rows and buttons on top take their
    // clicks (and pass drags on to the window too).
    let bg = ui.interact(full, ui.scope_id().with("bar-bg"), Sense::click_and_drag());
    drag_window(ui, &bg);
    frame(ui).show(ui, |ui| match mode {
        BarMode::Pill => {
            out.size = pill(ui, &active, now, t, &bg, &mut out.toggle_mode);
        }
        BarMode::List => {
            bg.clone().on_hover_cursor(CursorIcon::Grab);
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
            ui.set_height(PILL_H - 2.0 * PAD - 2.0);
            ui.spacing_mut().item_spacing.x = 10.0;
            let lead = active.first().map_or(Display::Idle, |r| r.state);
            let alert = active.first().is_some_and(|r| r.alert);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
            let state = if s.working > 0 && !alert { Display::Working } else { lead };
            look::ring(ui, rect.center(), 6.5, state, alert, t);
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
    if bg.clicked() {
        *toggle = true;
    }
    let w = resp.rect.width() + 2.0 * PAD + 2.0 + 6.0;
    Vec2::new(w.max(96.0), PILL_H)
}

/// Six dots marking the header as the place to drag the bar by.
fn grip(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 16.0), Sense::hover());
    let c = ui.visuals().weak_text_color();
    for i in 0..3 {
        for j in 0..2 {
            let p = Pos2::new(rect.left() + 2.0 + j as f32 * 5.0, rect.top() + 3.0 + i as f32 * 5.0);
            ui.painter().circle_filled(p, 1.3, c);
        }
    }
}

/// Draws the list; returns the height its content needs (the window may
/// still be smaller than that this frame).
fn list(ui: &mut Ui, shared: &Shared, active: &[&Row], now: f64, t: f64, out: &mut BarOut) -> f32 {
    let width = super::BAR_WIDTH - 2.0 * PAD - 2.0;
    ui.set_width(width);
    // Header: grip, counts, open-app and collapse buttons.
    let header = ui.horizontal(|ui| {
        ui.set_height(HEADER_H - 4.0);
        grip(ui);
        ui.label(RichText::new("nuff-pulse").strong());
        let s = fmt::summary(&active.iter().map(|r| (*r).clone()).collect::<Vec<_>>(), now);
        if s.active > 0 {
            ui.label(RichText::new(format!("{} session{}", s.active, if s.active == 1 { "" } else { "s" })).weak());
        }
        if s.alerts > 0 {
            ui.label(RichText::new(format!("{} new", s.alerts)).color(look::state_color(ui, Display::NeedsYou)));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let btn = |text: &str| egui::Button::new(text).min_size(Vec2::new(30.0, 26.0));
            if ui.add(btn("–")).on_hover_text("Collapse to pill (hotkey toggles)").clicked() {
                out.toggle_mode = true;
            }
            if ui.add(btn("app")).on_hover_text("Open the app window").clicked() {
                out.open_app = true;
            }
        });
    });
    let header_h = header.response.rect.height() + ui.spacing().item_spacing.y;
    if active.is_empty() {
        ui.add_space(6.0);
        let r = ui.label(RichText::new("No active sessions").weak());
        return header_h + 6.0 + r.rect.height();
    }
    let shown = active.len().min(MAX_ROWS);
    let rows_h = shown as f32 * ROW_H;
    egui::ScrollArea::vertical().max_height(rows_h).min_scrolled_height(rows_h).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, r) in active.iter().enumerate() {
            row(ui, shared, r, now, t, i > 0);
        }
    });
    header_h + rows_h
}

/// A one-line child ui filling `rect`.
fn line(ui: &mut Ui, rect: Rect, layout: Layout, add: impl FnOnce(&mut Ui)) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut child);
}

fn row(ui: &mut Ui, shared: &Shared, r: &Row, now: f64, t: f64, separator: bool) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click_and_drag());
    drag_window(ui, &resp);
    let p = ui.painter();
    if separator {
        p.hline(rect.x_range(), rect.top(), Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color.gamma_multiply(0.6)));
    }
    if r.alert {
        // Alert rows stand out: a tinted background and a coloured edge.
        let c = look::state_color(ui, r.state);
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), CornerRadius::same(4), c.gamma_multiply(0.10));
        let edge = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 7.0), Vec2::new(3.0, ROW_H - 14.0));
        p.rect_filled(edge, CornerRadius::same(2), c);
    }
    if resp.hovered() {
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), CornerRadius::same(4), ui.visuals().widgets.hovered.weak_bg_fill.gamma_multiply(0.7));
    }
    if resp.clicked() {
        open_row(shared, r);
    }
    // Keep it short: tooltips are drawn inside the bar's own small window.
    resp.on_hover_text(hover(r));

    // Columns: ring | title + what it does | figures + where (right-aligned).
    let inner = rect.shrink2(Vec2::new(8.0, 6.0));
    let ring_c = Pos2::new(inner.left() + 8.0, inner.center().y);
    look::ring(ui, ring_c, 6.5, r.state, r.alert, t);
    let half = inner.height() / 2.0;
    let fig_left = inner.right() - FIGURES_W;
    let mid = Rect::from_min_max(Pos2::new(inner.left() + 26.0, inner.top()), Pos2::new(fig_left - 8.0, inner.bottom()));
    let fig = Rect::from_min_max(Pos2::new(fig_left, inner.top()), inner.right_bottom());
    let top = |c: Rect| Rect::from_min_size(c.min, Vec2::new(c.width(), half));
    let bottom = |c: Rect| Rect::from_min_size(c.min + Vec2::new(0.0, half), Vec2::new(c.width(), half));
    let left = Layout::left_to_right(Align::Center);
    let right = Layout::right_to_left(Align::Center);

    line(ui, top(mid), left, |ui| {
        ui.add(egui::Label::new(RichText::new(&r.title).strong()).truncate());
    });
    line(ui, bottom(mid), left, |ui| {
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
        ui.add(egui::Label::new(RichText::new(second).small().color(color)).truncate());
    });
    line(ui, top(fig), right, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if let Some(e) = r.cache_expires {
            let left = e - now;
            let lvl = nuff_pulse::model::cache_level(left);
            ui.label(RichText::new(fmt::left(left)).color(look::cache_color(ui, lvl)).monospace());
        }
        let ctx_text = fmt::context(r);
        if !ctx_text.is_empty() {
            ui.label(RichText::new(ctx_text).color(look::level_color(ui, r.context_level)).monospace());
        }
    });
    line(ui, bottom(fig), right, |ui| {
        ui.add(egui::Label::new(RichText::new(fmt::place(r)).small().color(ui.visuals().weak_text_color())).truncate());
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
