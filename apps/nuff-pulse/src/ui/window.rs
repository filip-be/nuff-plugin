//! The app window: a table of active and today's ended sessions with a
//! detail pane, and the settings.

use super::look;
use super::{Cmd, Shared, open_row};
use eframe::egui::{self, Align, Layout, RichText, Ui, ViewportCommand};
use nuff_pulse::model::{Row, SessionKey};
use nuff_pulse::settings::{self, ExtraRoot, Settings};
use nuff_pulse::{fmt, util};
use std::sync::Mutex;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sessions,
    Settings,
}

pub struct WinState {
    pub open: bool,
    pub tab: Tab,
    pub selected: Option<SessionKey>,
    /// Settings being edited; saved ones go to `apply` for the UI thread.
    pub edit: Settings,
    pub apply: Option<Settings>,
    pub status: String,
    pub hotkey_ok: Option<bool>,
    /// Context window overrides as text rows while editing.
    windows: Vec<(String, String)>,
}

impl WinState {
    pub fn new(s: &Settings) -> WinState {
        let mut w = WinState {
            open: false,
            tab: Tab::Sessions,
            selected: None,
            edit: s.clone(),
            apply: None,
            status: String::new(),
            hotkey_ok: None,
            windows: Vec::new(),
        };
        w.load_windows();
        w
    }

    fn load_windows(&mut self) {
        self.windows = self
            .edit
            .context_windows
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
    }
}

pub fn show(ui: &mut Ui, shared: &Shared, win: &Mutex<WinState>) {
    let ctx = ui.ctx().clone();
    let mut w = win.lock().unwrap();
    if ctx.input(|i| i.viewport().close_requested()) {
        // Closing hides the window; it is opened again from the tray or bar.
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        w.open = false;
        shared.app_open.store(false, Ordering::Relaxed);
        return;
    }
    let rows = shared.rows();
    let now = util::now();
    egui::Panel::top("tabs").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut w.tab, Tab::Sessions, "Sessions");
            ui.selectable_value(&mut w.tab, Tab::Settings, "Settings");
        });
    });
    match w.tab {
        Tab::Sessions => sessions(ui, shared, &mut w, &rows, now),
        Tab::Settings => settings_tab(ui, shared, &mut w),
    }
    let mut next = 60.0f64;
    for r in rows.iter().filter(|r| r.active) {
        if let Some(e) = r.cache_expires {
            next = next.min(fmt::next_tick(e, now));
        }
    }
    ctx.request_repaint_after(std::time::Duration::from_secs_f64(next.max(0.05)));
}

fn sessions(ui: &mut Ui, shared: &Shared, w: &mut WinState, rows: &[Row], now: f64) {
    if w.selected
        .as_ref()
        .is_none_or(|k| !rows.iter().any(|r| &r.key == k))
    {
        w.selected = rows.first().map(|r| r.key.clone());
    }
    let selected = w
        .selected
        .as_ref()
        .and_then(|k| rows.iter().find(|r| &r.key == k))
        .cloned();
    egui::Panel::right("detail")
        .resizable(true)
        .default_size(380.0)
        .min_size(260.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match &selected {
                    Some(r) => detail(ui, shared, r, now),
                    None => {
                        ui.add_space(8.0);
                        ui.label(RichText::new("Select a session").weak());
                    }
                });
        });
    egui::CentralPanel::default().show(ui, |ui| {
        if rows.is_empty() {
            ui.label(RichText::new("No sessions today").weak());
            return;
        }
        let t = ui.input(|i| i.time);
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("sessions")
                .striped(true)
                .num_columns(8)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for h in [
                        "", "State", "Title", "Runtime", "Context", "Cache", "Model", "Cost",
                    ] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in rows {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        look::ring(ui, rect.center(), 5.0, r.state, r.alert, t);
                        ui.label(
                            RichText::new(r.state.label()).color(look::state_color(ui, r.state)),
                        );
                        let is_sel = w.selected.as_ref() == Some(&r.key);
                        let title = if r.hidden {
                            format!("{} (hidden)", util::short(&r.title, 40))
                        } else {
                            util::short(&r.title, 48)
                        };
                        if ui.selectable_label(is_sel, title).clicked() {
                            w.selected = Some(r.key.clone());
                            if r.alert {
                                shared.send(Cmd::Ack(r.key.clone()));
                            }
                        }
                        let runtime = match &r.key.origin {
                            nuff_pulse::model::Origin::Wsl(d) => format!("{} · WSL {d}", r.label),
                            nuff_pulse::model::Origin::Windows => r.label.clone(),
                        };
                        ui.label(runtime).on_hover_text(fmt::place(r));
                        ui.label(
                            RichText::new(fmt::context(r))
                                .color(look::level_color(ui, r.context_level)),
                        );
                        match r.cache_expires {
                            Some(e) => {
                                let left = e - now;
                                ui.label(RichText::new(fmt::left(left)).color(look::cache_color(
                                    ui,
                                    nuff_pulse::model::cache_level(left),
                                )))
                            }
                            None => ui.label(""),
                        };
                        ui.label(r.model.clone().unwrap_or_default());
                        ui.label(fmt::cost(r));
                        ui.end_row();
                    }
                });
        });
    });
}

fn kv(ui: &mut Ui, k: &str, v: impl Into<egui::WidgetText>) {
    ui.label(RichText::new(k).weak());
    ui.add(egui::Label::new(v).wrap());
    ui.end_row();
}

fn detail(ui: &mut Ui, shared: &Shared, r: &Row, now: f64) {
    let today = util::start_of_today(now);
    ui.add_space(4.0);
    ui.heading(&r.title);
    ui.horizontal(|ui| {
        let state = format!(
            "{} since {}",
            r.state.label(),
            fmt::clock(r.state_since, today)
        );
        ui.label(
            RichText::new(state)
                .color(look::state_color(ui, r.state))
                .strong(),
        );
        if r.cache_expiring {
            ui.label(
                RichText::new("cache expiring")
                    .color(look::level_color(ui, nuff_pulse::model::Level::Amber)),
            );
        }
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(super::can_focus(r), egui::Button::new("Focus window"))
            .clicked()
        {
            open_row(shared, r);
        }
        if ui
            .button("Copy resume command")
            .on_hover_text(fmt::resume(r))
            .clicked()
        {
            ui.ctx().copy_text(fmt::resume(r));
        }
        if r.active
            && ui
                .button("Hide from bar")
                .on_hover_text("Until its next activity")
                .clicked()
        {
            shared.send(Cmd::Hide(r.key.clone()));
        }
        if r.hidden
            && ui
                .button("Show on bar")
                .on_hover_text("Hidden until its next activity")
                .clicked()
        {
            shared.send(Cmd::Unhide(r.key.clone()));
        }
    });
    ui.add_space(6.0);
    egui::Grid::new("detail-grid")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            if let Some(d) = r.detail.as_ref().or(r.reason.as_ref()) {
                kv(ui, "Waiting on", d.as_str());
            }
            if let Some(a) = &r.activity {
                kv(ui, "Activity", a.as_str());
            }
            kv(ui, "Where", fmt::place(r));
            if let Some(c) = &r.cwd {
                kv(ui, "Workspace", c.as_str());
            }
            if let Some(b) = &r.branch {
                kv(ui, "Branch", b.as_str());
            }
            if let Some(m) = &r.model {
                let m = match &r.effort {
                    Some(e) => format!("{m} ({e})"),
                    None => m.clone(),
                };
                kv(ui, "Model", m);
            }
            if let Some(c) = r.context {
                ui.label(RichText::new("Context").weak());
                match r.context_window {
                    Some(win) if win > 0 => {
                        let frac = (c as f32 / win as f32).clamp(0.0, 1.0);
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .text(format!("{} ({:.0} %)", fmt::context(r), frac * 100.0))
                                .fill(look::level_color(ui, r.context_level).gamma_multiply(0.7)),
                        );
                    }
                    _ => {
                        ui.label(
                            RichText::new(fmt::context(r))
                                .color(look::level_color(ui, r.context_level)),
                        );
                    }
                }
                ui.end_row();
            }
            if let Some(e) = r.cache_expires {
                let left = e - now;
                let mut text = if left > 0.0 {
                    format!("{} left, expires {}", fmt::left(left), fmt::clock(e, today))
                } else {
                    format!("cold since {}", fmt::clock(e, today))
                };
                if let Some(ttl) = r.cache_ttl {
                    text.push_str(&format!(" · TTL {}", fmt::left(ttl)));
                }
                ui.label(RichText::new("Cache").weak());
                ui.label(
                    RichText::new(text)
                        .color(look::cache_color(ui, nuff_pulse::model::cache_level(left))),
                );
                ui.end_row();
            }
            if r.subagents > 0 {
                kv(ui, "Subagents", r.subagents.to_string());
            }
            if let Some(p) = &r.permission_mode {
                kv(ui, "Permission mode", p.as_str());
            }
            let c = fmt::cost_detail(r);
            if !c.is_empty() {
                kv(ui, "Cost", c);
            }
            kv(
                ui,
                "Last activity",
                format!(
                    "{} ({} ago)",
                    fmt::clock(r.last_activity, today),
                    fmt::ago(r.last_activity, now)
                ),
            );
            kv(ui, "Session", r.key.id.as_str());
        });
    if let Some(p) = &r.last_prompt {
        ui.add_space(6.0);
        ui.label(RichText::new("Last prompt").weak());
        ui.add(egui::Label::new(util::short(p, 400)).wrap());
    }
    if !r.history.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Recent state changes").weak());
        for (t, s) in r.history.iter().rev().take(10) {
            ui.label(format!("{}  {}", fmt::clock(*t, today), state_name(*s)));
        }
    }
}

fn state_name(s: nuff_pulse::model::State) -> &'static str {
    use nuff_pulse::model::State;
    match s {
        State::Idle => "Idle",
        State::Working => "Working",
        State::NeedsYou => "Needs you",
        State::Done => "Done",
        State::Error => "Error",
        State::Ended => "Ended",
    }
}

fn settings_tab(ui: &mut Ui, shared: &Shared, w: &mut WinState) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bar");
            ui.horizontal(|ui| {
                ui.label("Hotkey (pill ↔ list)");
                ui.add(egui::TextEdit::singleline(&mut w.edit.hotkey).desired_width(160.0));
                let parsed = settings::parse_hotkey(&w.edit.hotkey);
                let note = match (parsed, w.hotkey_ok) {
                    (None, _) => RichText::new("not a valid hotkey (e.g. Win+Alt+P)").color(look::level_color(ui, nuff_pulse::model::Level::Red)),
                    (Some(_), Some(false)) => RichText::new("in use by another app").color(look::level_color(ui, nuff_pulse::model::Level::Amber)),
                    _ => RichText::new("").weak(),
                };
                ui.label(note);
            });
            let mut autostart = nuff_pulse::desktop::autostart_enabled();
            if ui.checkbox(&mut autostart, "Start with Windows").changed() {
                nuff_pulse::desktop::set_autostart(autostart);
            }

            ui.add_space(10.0);
            ui.heading("Extra roots");
            ui.label(RichText::new("Folders read in addition to ~/.claude, ~/.copilot, ~/.nuff-pulse and running WSL distros.").weak());
            let mut remove = None;
            egui::Grid::new("roots").num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
                for (i, r) in w.edit.extra_roots.iter_mut().enumerate() {
                    egui::ComboBox::from_id_salt(("kind", i)).selected_text(r.kind.clone()).show_ui(ui, |ui| {
                        for k in ["claude", "copilot", "pulse"] {
                            ui.selectable_value(&mut r.kind, k.to_string(), k);
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut r.path).desired_width(360.0).hint_text("folder path"));
                    ui.checkbox(&mut r.poll, "poll").on_hover_text("Poll every 5 s instead of watching (network paths)");
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                w.edit.extra_roots.remove(i);
            }
            if ui.button("Add root").clicked() {
                w.edit.extra_roots.push(ExtraRoot { kind: "claude".into(), path: String::new(), poll: false });
            }

            ui.add_space(10.0);
            ui.heading("Context windows");
            ui.label(RichText::new("Model-name prefix → window in tokens; the longest matching prefix wins.").weak());
            let mut remove = None;
            egui::Grid::new("windows").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                for (i, (prefix, tokens)) in w.windows.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(prefix).desired_width(220.0).hint_text("claude-opus"));
                    ui.add(egui::TextEdit::singleline(tokens).desired_width(120.0).hint_text("1000000"));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                w.windows.remove(i);
            }
            if ui.button("Add window").clicked() {
                w.windows.push((String::new(), String::new()));
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    match parse_windows(&w.windows) {
                        _ if settings::parse_hotkey(&w.edit.hotkey).is_none() => {
                            w.status = "Not saved: the hotkey is not valid.".into();
                        }
                        Ok(map) => {
                            w.edit.context_windows = map;
                            w.apply = Some(w.edit.clone());
                            w.status = "Saving…".into();
                            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                        }
                        Err(e) => w.status = e,
                    }
                }
                if ui.button("Revert").clicked() {
                    w.edit = Settings::load();
                    w.load_windows();
                    w.status.clear();
                }
                ui.label(RichText::new(&w.status).weak());
            });

            ui.add_space(12.0);
            ui.heading("Roots in use");
            let roots = shared.data.lock().unwrap().roots.clone();
            for r in roots {
                let origin = match &r.origin {
                    nuff_pulse::model::Origin::Windows => String::new(),
                    nuff_pulse::model::Origin::Wsl(d) => format!(" (WSL {d})"),
                };
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.label(RichText::new(format!("{:?}{origin}{}", r.kind, if r.poll { ", polled" } else { "" })).weak());
                    ui.label(r.path.display().to_string());
                });
            }
            if let Some(p) = Settings::path() {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Settings file: {}", p.display())).weak());
            }
        });
    });
}

fn parse_windows(
    rows: &[(String, String)],
) -> Result<std::collections::BTreeMap<String, u64>, String> {
    let mut out = std::collections::BTreeMap::new();
    for (prefix, tokens) in rows {
        let (p, t) = (prefix.trim(), tokens.trim().replace(['_', ','], ""));
        if p.is_empty() && t.is_empty() {
            continue;
        }
        let n: u64 = t
            .parse()
            .map_err(|_| format!("Context window for '{p}' is not a number"))?;
        if p.is_empty() || n == 0 {
            return Err("Each context window needs a prefix and a size above 0".into());
        }
        out.insert(p.to_string(), n);
    }
    Ok(out)
}
