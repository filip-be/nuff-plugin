//! The widget: a floating bar (the root window), the app window (a second
//! window, created hidden and shown on demand), the tray and hotkey (shell
//! thread) and the engine (worker thread).
//!
//! Threads: the engine thread owns the `Engine`, reads files and publishes
//! rows into `Shared`; the shell thread owns the tray icon and hotkey and
//! sends `ShellEvent`s; the UI thread draws and owns the windows. Each wakes
//! the UI with `request_repaint`, so the UI only repaints on change, once a
//! minute for countdowns, and while something works (the turning ring).

mod bar;
mod look;
mod window;

use eframe::egui::{self, ViewportBuilder, ViewportCommand, ViewportId};
use nuff_pulse::engine::{Engine, Options};
use nuff_pulse::model::{Row, SessionKey};
use nuff_pulse::roots::{self, Root};
use nuff_pulse::settings::{self, BarMode, Settings};
use nuff_pulse::shell::{Shell, ShellEvent, TrayState};
use nuff_pulse::{desktop, fmt, util, wsl};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const BAR_WIDTH: f32 = 460.0;
/// At startup, a Done older than this raises no alert.
const STARTUP_SEEN_DONE: f64 = 5.0 * 60.0;

pub fn app_viewport() -> ViewportId {
    ViewportId::from_hash_of("nuff-pulse-app")
}

/// What the engine thread publishes.
#[derive(Default)]
pub struct Published {
    pub rows: Vec<Row>,
    pub roots: Vec<Root>,
}

pub enum Cmd {
    Ack(SessionKey),
    Settings(Settings),
    Quit,
}

/// State shared by the threads.
pub struct Shared {
    pub data: Mutex<Published>,
    pub cmd: Mutex<Sender<Cmd>>,
    pub events: Mutex<VecDeque<ShellEvent>>,
    pub bar_visible: AtomicBool,
    pub app_open: AtomicBool,
    pub shell: Mutex<Option<Arc<Shell>>>,
}

impl Shared {
    pub fn send(&self, c: Cmd) {
        let _ = self.cmd.lock().unwrap().send(c);
    }

    pub fn rows(&self) -> Vec<Row> {
        self.data.lock().unwrap().rows.clone()
    }

    /// Pushes alert count, blink and tooltip to the tray.
    pub fn push_tray(&self, rows: &[Row]) {
        let Some(shell) = self.shell.lock().unwrap().clone() else { return };
        let s = fmt::summary(rows, util::now());
        shell.set_tray(TrayState {
            alerts: s.alerts,
            blink: s.blink,
            tooltip: fmt::tooltip(&s),
            bar_visible: self.bar_visible.load(Ordering::Relaxed),
        });
    }
}

/// Clears a row's alert and brings its window to the front.
pub fn open_row(shared: &Shared, r: &Row) {
    shared.send(Cmd::Ack(r.key.clone()));
    if can_focus(r) {
        nuff_pulse::focus::focus(r.focus_pid, r.focus_started, &r.workspace);
    }
}

/// Windows sessions with an agent pid, or WSL sessions (by title); never an
/// ended session, whose window is gone or belongs to something else now.
pub fn can_focus(r: &Row) -> bool {
    r.state != nuff_pulse::model::Display::Ended
        && (r.focus_pid.is_some() || matches!(r.key.origin, nuff_pulse::model::Origin::Wsl(_)))
}

/// Runs the widget; `open_app` also opens the app window. When a widget
/// already runs, it is asked to show its bar (or app window) instead.
pub fn run(open_app: bool) -> eframe::Result {
    if !desktop::claim_single_instance() {
        nuff_pulse::shell::signal_running_instance(open_app);
        return Ok(());
    }
    let settings = Settings::load();
    let icon = egui::IconData { rgba: nuff_pulse::icon::render(64, 0, true), width: 64, height: 64 };
    let opts = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("nuff-pulse")
            .with_app_id("nuff-pulse")
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_inner_size([BAR_WIDTH, 60.0])
            .with_icon(icon),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native("nuff-pulse", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, settings, open_app)))))
}

struct App {
    shared: Arc<Shared>,
    hwnd: isize,
    settings: Settings,
    fullscreen: bool,
    quitting: bool,
    layout: String,
    /// Bar top-left last placed or seen (physical pixels).
    placed: Option<[i32; 2]>,
    save_position_at: Option<f64>,
    placed_once: bool,
    last_size: egui::Vec2,
    win: Arc<Mutex<window::WinState>>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, settings: Settings, open_app: bool) -> App {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => 0,
        };
        if hwnd != 0 {
            desktop::round_corners(hwnd);
        }
        let ctx = cc.egui_ctx.clone();
        let (tx, rx) = channel();
        let shared = Arc::new(Shared {
            data: Mutex::new(Published::default()),
            cmd: Mutex::new(tx),
            events: Mutex::new(if open_app { VecDeque::from([ShellEvent::OpenApp]) } else { VecDeque::new() }),
            bar_visible: AtomicBool::new(settings.bar_visible),
            app_open: AtomicBool::new(false),
            shell: Mutex::new(None),
        });
        let ev_shared = shared.clone();
        let ev_ctx = ctx.clone();
        let shell = Shell::spawn(
            move |ev| {
                ev_shared.events.lock().unwrap().push_back(ev);
                ev_ctx.request_repaint_of(ViewportId::ROOT);
            },
            settings::parse_hotkey(&settings.hotkey),
            TrayState { tooltip: "nuff-pulse".into(), bar_visible: settings.bar_visible, ..Default::default() },
        );
        *shared.shell.lock().unwrap() = shell.map(Arc::new);
        {
            let (shared, ctx, settings) = (shared.clone(), ctx.clone(), settings.clone());
            let _ = std::thread::Builder::new().name("nuff-pulse-engine".into()).spawn(move || worker(ctx, shared, rx, settings));
        }
        let win = Arc::new(Mutex::new(window::WinState::new(&settings)));
        App {
            shared,
            hwnd,
            settings,
            fullscreen: false,
            quitting: false,
            layout: String::new(),
            placed: None,
            save_position_at: None,
            placed_once: false,
            last_size: egui::Vec2::ZERO,
            win,
        }
    }

    fn bar_should_show(&self) -> bool {
        self.settings.bar_visible && !self.fullscreen
    }

    fn apply_bar_visibility(&mut self) {
        if self.hwnd == 0 {
            return;
        }
        let want = self.bar_should_show();
        if desktop::is_visible(self.hwnd) != want {
            desktop::set_visible(self.hwnd, want);
        }
        self.shared.bar_visible.store(self.settings.bar_visible, Ordering::Relaxed);
        self.shared.push_tray(&self.shared.rows());
    }

    fn save_settings(&self) {
        let _ = self.settings.save();
    }

    fn open_app(&mut self, ctx: &egui::Context, settings_tab: bool) {
        {
            let mut w = self.win.lock().unwrap();
            w.open = true;
            if settings_tab {
                w.tab = window::Tab::Settings;
            }
            w.hotkey_ok = self.shared.shell.lock().unwrap().as_ref().and_then(|s| s.hotkey_ok());
        }
        self.shared.app_open.store(true, Ordering::Relaxed);
        let id = app_viewport();
        ctx.send_viewport_cmd_to(id, ViewportCommand::Visible(true));
        ctx.send_viewport_cmd_to(id, ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd_to(id, ViewportCommand::Focus);
        ctx.request_repaint_of(id);
    }

    /// Places the bar for the current monitor layout: the remembered spot,
    /// or the top-right corner of the primary work area.
    fn place_bar(&mut self) {
        if self.hwnd == 0 {
            return;
        }
        self.layout = settings::layout_key(&desktop::monitors());
        let size = desktop::window_rect(self.hwnd).map_or([460, 60], |r| [r[2] - r[0], r[3] - r[1]]);
        let pos = match self.settings.bar_positions.get(&self.layout) {
            Some(&[x, y]) if desktop::on_screen(x + 8, y + 8) => [x, y],
            _ => {
                let wa = desktop::work_area();
                [wa[2] - size[0] - 24, wa[1] + 24]
            }
        };
        desktop::move_window(self.hwnd, pos[0], pos[1]);
        self.placed = Some(pos);
    }

    /// Remembers where the user dragged the bar, per monitor layout; saved
    /// once the bar has rested for a second (a drag moves it many times).
    fn track_position(&mut self, ctx: &egui::Context) {
        if self.hwnd == 0 || !desktop::is_visible(self.hwnd) {
            return;
        }
        let Some(r) = desktop::window_rect(self.hwnd) else { return };
        let pos = [r[0], r[1]];
        let now = util::now();
        if self.placed != Some(pos) {
            self.placed = Some(pos);
            self.save_position_at = Some(now + 1.0);
            ctx.request_repaint_after(Duration::from_millis(1100));
        } else if self.save_position_at.is_some_and(|t| now >= t) {
            self.save_position_at = None;
            self.settings.bar_positions.insert(self.layout.clone(), pos);
            self.save_settings();
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let events: Vec<ShellEvent> = self.shared.events.lock().unwrap().drain(..).collect();
        for ev in events {
            match ev {
                ShellEvent::ToggleBar => {
                    self.settings.bar_visible = !self.settings.bar_visible;
                    self.save_settings();
                    self.apply_bar_visibility();
                }
                ShellEvent::ShowBar => {
                    self.settings.bar_visible = true;
                    self.save_settings();
                    self.apply_bar_visibility();
                }
                ShellEvent::ToggleMode => {
                    self.settings.bar_mode = match self.settings.bar_mode {
                        BarMode::Pill => BarMode::List,
                        BarMode::List => BarMode::Pill,
                    };
                    if !self.settings.bar_visible {
                        self.settings.bar_visible = true;
                        self.apply_bar_visibility();
                    }
                    self.save_settings();
                }
                ShellEvent::OpenApp => self.open_app(ctx, false),
                ShellEvent::OpenSettings => self.open_app(ctx, true),
                ShellEvent::Fullscreen(f) => {
                    self.fullscreen = f;
                    self.apply_bar_visibility();
                }
                ShellEvent::LayoutChanged => self.place_bar(),
                ShellEvent::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
            }
        }
        // Settings saved in the app window.
        let applied = self.win.lock().unwrap().apply.take();
        if let Some(mut s) = applied {
            s.bar_positions = self.settings.bar_positions.clone();
            s.bar_mode = self.settings.bar_mode;
            s.bar_visible = self.settings.bar_visible;
            let hotkey_changed = s.hotkey != self.settings.hotkey;
            self.settings = s;
            let saved = self.settings.save();
            self.shared.send(Cmd::Settings(self.settings.clone()));
            let shell = self.shared.shell.lock().unwrap().clone();
            if let (true, Some(shell)) = (hotkey_changed, shell) {
                shell.set_hotkey(settings::parse_hotkey(&self.settings.hotkey));
            }
            let mut w = self.win.lock().unwrap();
            w.status = match saved {
                Ok(()) => "Saved.".into(),
                Err(e) => format!("Could not save: {e}"),
            };
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.placed_once {
            self.placed_once = true;
            self.place_bar();
            self.shared.bar_visible.store(self.settings.bar_visible, Ordering::Relaxed);
        }
        self.handle_events(ctx);
        // eframe shows the root after its first frame, so a bar hidden when
        // the app last ran is hidden again here.
        if self.hwnd != 0 && self.placed.is_some() && desktop::is_visible(self.hwnd) && !self.bar_should_show() {
            desktop::set_visible(self.hwnd, false);
        }
        // The hotkey outcome arrives from the shell thread after a change.
        if let Some(shell) = self.shared.shell.lock().unwrap().as_ref() {
            let ok = shell.hotkey_ok();
            let mut w = self.win.lock().unwrap();
            if w.hotkey_ok != ok {
                w.hotkey_ok = ok;
                ctx.request_repaint_of(app_viewport());
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            // Alt+F4 on the bar hides it; Quit is in the tray menu.
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.settings.bar_visible = false;
            self.save_settings();
            self.apply_bar_visibility();
        }
        self.track_position(&ctx);

        // The app window: always registered, hidden until opened.
        let (shared, win) = (self.shared.clone(), self.win.clone());
        ctx.show_viewport_deferred(
            app_viewport(),
            ViewportBuilder::default()
                .with_title("nuff-pulse")
                .with_app_id("nuff-pulse")
                .with_inner_size([1000.0, 620.0])
                .with_min_inner_size([640.0, 360.0])
                .with_visible(false),
            move |ui, _class| window::show(ui, &shared, &win),
        );

        let rows = self.shared.rows();
        let now = util::now();
        let out = bar::show(ui, &self.shared, &rows, now, self.settings.bar_mode);
        if out.toggle_mode {
            self.settings.bar_mode = match self.settings.bar_mode {
                BarMode::Pill => BarMode::List,
                BarMode::List => BarMode::Pill,
            };
            self.save_settings();
            ctx.request_repaint();
        }
        if out.open_app {
            self.open_app(&ctx, false);
        }
        if (out.size - self.last_size).length() > 0.5 {
            self.last_size = out.size;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(out.size));
        }
        // Countdowns change once a minute; the ring turns while working.
        let mut next = 60.0f64;
        for r in rows.iter().filter(|r| r.active) {
            if let Some(e) = r.cache_expires {
                next = next.min(fmt::next_tick(e, now));
            }
        }
        let working = rows.iter().any(|r| r.active && r.state == nuff_pulse::model::Display::Working);
        if working {
            // The ring turns at 4 frames a second: enough to read as busy.
            next = next.min(0.25);
        }
        ctx.request_repaint_after(Duration::from_secs_f64(next.max(0.05)));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.send(Cmd::Quit);
        if let Some(shell) = self.shared.shell.lock().unwrap().take() {
            shell.close();
        }
        self.save_settings();
    }
}

/// The engine thread: reads files, follows WSL distros and settings, and
/// publishes rows when something the UI shows changed.
fn worker(ctx: egui::Context, shared: Arc<Shared>, rx: Receiver<Cmd>, settings: Settings) {
    let base = roots::default_roots();
    let mut extra = settings.roots();
    // WSL first, so the startup rule below covers WSL sessions too.
    let mut wsl_roots: Vec<Root> = wsl::discover().unwrap_or_default();
    let all = |extra: &[Root], wsl_roots: &[Root]| -> Vec<Root> {
        let mut v = base.clone();
        for r in extra.iter().chain(wsl_roots) {
            if !v.contains(r) {
                v.push(r.clone());
            }
        }
        v
    };
    let mut engine = Engine::new(all(&extra, &wsl_roots), Options::default());
    engine.set_config(settings.config());
    engine.start(util::now());
    // Turns that finished well before the widget started are not news.
    let now = util::now();
    for r in engine.model.rows(now, util::start_of_today(now)) {
        if r.alert && r.state == nuff_pulse::model::Display::Done && now - r.state_since > STARTUP_SEEN_DONE {
            engine.acknowledge(&r.key, now);
        }
    }
    let mut next_wsl = util::now() + wsl::REFRESH_EVERY;
    let mut last_shown = Vec::new();
    let mut last_publish = f64::MIN;
    let mut generation = u64::MAX;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Cmd::Ack(key)) => engine.acknowledge(&key, util::now()),
            Ok(Cmd::Settings(s)) => {
                extra = s.roots();
                engine.set_config(s.config());
                engine.set_roots(all(&extra, &wsl_roots), util::now());
            }
            Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let now = util::now();
        if now >= next_wsl {
            next_wsl = now + wsl::REFRESH_EVERY;
            if let Some(found) = wsl::discover().filter(|f| *f != wsl_roots) {
                wsl_roots = found;
                engine.set_roots(all(&extra, &wsl_roots), now);
            }
        }
        engine.poll(now);
        let rows = engine.model.rows(now, util::start_of_today(now));
        let shown: Vec<_> =
            rows.iter().map(|r| (r.key.clone(), r.state, r.alert, r.cache_expiring, r.active)).collect();
        if engine.generation() != generation || shown != last_shown || now - last_publish >= 30.0 {
            generation = engine.generation();
            last_shown = shown;
            last_publish = now;
            shared.push_tray(&rows);
            {
                let mut d = shared.data.lock().unwrap();
                d.rows = rows;
                d.roots = engine.roots().to_vec();
            }
            ctx.request_repaint_of(ViewportId::ROOT);
            if shared.app_open.load(Ordering::Relaxed) {
                ctx.request_repaint_of(app_viewport());
            }
        }
    }
}
