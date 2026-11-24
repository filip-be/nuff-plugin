//! Sessions and their reduced state.
//!
//! Each session has up to three state sources, each keeping only its newest
//! signal: the log (transcript or events.jsonl), Claude's status file and the
//! hook lines. The reducer orders them by timestamp and folds them, so the
//! newest wins, with one rule: a status `idle` after Working or Needs you
//! means the turn stopped (Done). A failed liveness check ends the session.

use crate::claude::{self, ClaudeLog, ClaudeStatus};
use crate::copilot::{self, CopilotLog, LockProcess, Workspace, PROVISIONAL_DONE_DELAY};
use crate::pulse::HookState;
use crate::util;
use std::collections::{HashMap, VecDeque};

/// A session counts as active this long after its last write or hook event.
pub const ACTIVE_WINDOW: f64 = 2.0 * 3600.0;
/// "Cache expiring" overlay threshold.
pub const CACHE_EXPIRING: f64 = 5.0 * 60.0;
const HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Runtime {
    Claude,
    Copilot,
}

impl Runtime {
    pub fn as_str(self) -> &'static str {
        match self {
            Runtime::Claude => "claude",
            Runtime::Copilot => "copilot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Nothing running and nothing to report.
    Idle,
    Working,
    NeedsYou,
    Done,
    Error,
    Ended,
}

/// One state observation.
#[derive(Debug, Clone, PartialEq)]
pub struct Sig {
    pub t: f64,
    pub state: State,
    pub reason: Option<String>,
    pub detail: Option<String>,
    /// A Done inferred from disk only; its alert waits a few seconds.
    pub provisional: bool,
}

/// Where a session runs. WSL sessions are told apart by distro, never by pid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    Windows,
    Wsl(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionKey {
    pub runtime: Runtime,
    pub origin: Origin,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Liveness {
    #[default]
    Unknown,
    Live,
    NotLive,
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub claude: Option<ClaudeLog>,
    pub copilot: Option<CopilotLog>,
    pub workspace: Option<Workspace>,
    pub status: Option<ClaudeStatus>,
    pub hooks: HookState,
    pub live: Liveness,
    pub live_t: f64,
    /// Copilot: pid of the live `inuse.<pid>.hold` and its process.
    pub lock_pid: Option<u32>,
    pub lock: Option<LockProcess>,
    pub acked_t: f64,
    pub history: VecDeque<(f64, State)>,
    /// Modification time of the session's log when last read.
    pub log_mtime: Option<f64>,
}

impl Session {
    pub fn log_sig(&self) -> Option<&Sig> {
        self.claude.as_ref().and_then(|c| c.sig.as_ref()).or(self.copilot.as_ref().and_then(|c| c.sig.as_ref()))
    }

    /// The reduced state, `None` before any signal.
    pub fn reduce(&self) -> Option<Sig> {
        const LOG: u8 = 0;
        const STATUS: u8 = 1;
        const HOOK: u8 = 2;
        let mut sigs: Vec<(u8, Sig)> = Vec::new();
        if let Some(s) = self.log_sig() {
            sigs.push((LOG, s.clone()));
        }
        if let Some(s) = self.status.as_ref().and_then(ClaudeStatus::sig) {
            sigs.push((STATUS, s));
        }
        if let Some(s) = &self.hooks.sig {
            sigs.push((HOOK, s.clone()));
        }
        sigs.sort_by(|a, b| a.1.t.total_cmp(&b.1.t).then(a.0.cmp(&b.0)));
        let mut cur: Option<Sig> = None;
        for (src, s) in sigs {
            let next = if src == STATUS && s.state == State::Idle {
                match &cur {
                    Some(c) if matches!(c.state, State::Working | State::NeedsYou) => {
                        Sig { t: s.t, state: State::Done, reason: None, detail: None, provisional: false }
                    }
                    Some(c) => c.clone(),
                    None => s,
                }
            } else {
                s
            };
            // Same state without its own reason keeps the earlier details.
            cur = Some(match cur {
                Some(c) if c.state == next.state && next.reason.is_none() && next.detail.is_none() => {
                    Sig { t: next.t, provisional: next.provisional && c.provisional, ..c }
                }
                _ => next,
            });
        }
        if self.live == Liveness::NotLive && cur.as_ref().is_none_or(|c| c.state != State::Ended) {
            let t = cur.as_ref().map_or(self.live_t, |c| c.t.max(self.live_t));
            cur = Some(Sig { t, state: State::Ended, reason: Some("not_live".into()), detail: None, provisional: false });
        }
        cur
    }

    /// Records a state change in the history.
    pub fn note_state(&mut self) {
        let Some(sig) = self.reduce() else { return };
        if self.history.back().is_none_or(|(_, s)| *s != sig.state) {
            self.history.push_back((sig.t, sig.state));
            while self.history.len() > HISTORY {
                self.history.pop_front();
            }
        }
    }

    /// Newest log write (record time or file modification time), status
    /// update or hook line.
    pub fn last_activity(&self) -> f64 {
        [
            self.log_mtime,
            self.claude.as_ref().and_then(|c| c.last_write_t),
            self.copilot.as_ref().and_then(|c| c.last_write_t),
            self.hooks.last_t,
            self.status.as_ref().and_then(|s| s.status_t),
        ]
        .into_iter()
        .flatten()
        .fold(0.0, f64::max)
    }
}

/// What the UI shows for a session's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Display {
    Working,
    NeedsYou,
    Done,
    Error,
    Idle,
    Ended,
}

impl Display {
    pub fn label(self) -> &'static str {
        match self {
            Display::Working => "Working",
            Display::NeedsYou => "Needs you",
            Display::Done => "Done",
            Display::Error => "Error",
            Display::Idle => "Idle",
            Display::Ended => "Ended",
        }
    }

    /// Lower sorts first.
    fn urgency(self, alert: bool) -> u8 {
        match (self, alert) {
            (Display::NeedsYou, true) => 0,
            (Display::Error, true) => 1,
            (Display::Done, true) => 2,
            (Display::NeedsYou, false) => 3,
            (Display::Working, _) => 4,
            (Display::Error, false) => 5,
            (Display::Done, false) | (Display::Idle, _) => 6,
            (Display::Ended, _) => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Amber,
    Red,
}

/// Context: amber ≥ 60 % (or ≥ 300k), red ≥ 80 % (or ≥ 500k).
pub fn context_level(tokens: u64, window: Option<u64>) -> Level {
    let pct = window.filter(|w| *w > 0).map(|w| tokens as f64 / w as f64).unwrap_or(0.0);
    if pct >= 0.8 || tokens >= 500_000 {
        Level::Red
    } else if pct >= 0.6 || tokens >= 300_000 {
        Level::Amber
    } else {
        Level::Ok
    }
}

/// Cache: green > 15 min left, amber ≤ 15 min, red when cold.
pub fn cache_level(remaining: f64) -> Level {
    if remaining <= 0.0 {
        Level::Red
    } else if remaining <= 15.0 * 60.0 {
        Level::Amber
    } else {
        Level::Ok
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Model-name prefix → context window, tokens; the longest prefix wins.
    pub context_windows: Vec<(String, u64)>,
}

impl Config {
    pub fn context_window(&self, model: Option<&str>, reported: Option<u64>) -> Option<u64> {
        let m = model.unwrap_or("");
        let configured = self
            .context_windows
            .iter()
            .filter(|(p, _)| !m.is_empty() && m.starts_with(p.as_str()))
            .max_by_key(|(p, _)| p.len())
            .map(|(_, w)| *w);
        configured.or(reported).or_else(|| builtin_window(m))
    }
}

/// Only windows the model name itself states. Claude transcripts do not
/// record the window, and current Claude models run past 200k, so unknown
/// windows fall back to the absolute thresholds (300k / 500k) unless the
/// settings name one.
fn builtin_window(model: &str) -> Option<u64> {
    model.contains("[1m]").then_some(1_000_000)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cost {
    /// Claude: list-price total (`cost-state.totalCostUSD`).
    pub usd: Option<f64>,
    /// Copilot: premium requests and nano AI units, shown separately.
    pub premium_requests: Option<f64>,
    pub nano_aiu: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub key: SessionKey,
    pub label: String,
    pub title: String,
    pub workspace: String,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub state: Display,
    pub alert: bool,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub state_since: f64,
    pub activity: Option<String>,
    pub subagents: usize,
    pub context: Option<u64>,
    pub context_window: Option<u64>,
    pub context_approx: bool,
    pub context_level: Level,
    pub cache_expires: Option<f64>,
    pub cache_level: Option<Level>,
    pub cache_expiring: bool,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub cost: Cost,
    pub last_prompt: Option<String>,
    pub permission_mode: Option<String>,
    /// Shown in the bar: not ended and active within the last 2 h.
    pub active: bool,
    pub last_activity: f64,
    pub history: Vec<(f64, State)>,
    pub resume_command: String,
    /// Process to start the focus walk from (Windows sessions only).
    pub focus_pid: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Model {
    pub sessions: HashMap<SessionKey, Session>,
    pub config: Config,
}

impl Model {
    pub fn session(&mut self, key: SessionKey) -> &mut Session {
        self.sessions.entry(key).or_default()
    }

    /// Clears the alert of a session (row clicked).
    pub fn acknowledge(&mut self, key: &SessionKey, now: f64) {
        if let Some(s) = self.sessions.get_mut(key) {
            s.acked_t = now;
        }
    }

    /// Drops sessions with no activity since `keep_since` that are not active.
    pub fn prune(&mut self, now: f64, keep_since: f64) {
        self.sessions.retain(|_, s| {
            let last = s.last_activity();
            last >= keep_since || (now - last < ACTIVE_WINDOW && s.reduce().is_none_or(|x| x.state != State::Ended))
        });
    }

    /// Rows of active sessions and of sessions active since `today`, most
    /// urgent first.
    pub fn rows(&self, now: f64, today: f64) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .sessions
            .iter()
            // A status file alone (a Claude process with no transcript or
            // hook line yet) is not activity.
            .filter(|(_, s)| s.claude.is_some() || s.copilot.is_some() || s.hooks.last_t.is_some())
            .map(|(k, s)| self.row(k, s, now))
            .filter(|r| r.active || r.last_activity >= today)
            .collect();
        rows.sort_by(|a, b| {
            a.state
                .urgency(a.alert)
                .cmp(&b.state.urgency(b.alert))
                .then(b.last_activity.total_cmp(&a.last_activity))
                .then(a.key.cmp(&b.key))
        });
        rows
    }

    pub fn row(&self, key: &SessionKey, s: &Session, now: f64) -> Row {
        let sig = s.reduce();
        let raw = sig.as_ref().map_or(State::Idle, |x| x.state);
        let since = sig.as_ref().map_or(0.0, |x| x.t);
        let unacked = since > s.acked_t;
        let (state, alert) = match raw {
            State::Idle => (Display::Idle, false),
            State::Working => (Display::Working, false),
            State::NeedsYou => (Display::NeedsYou, unacked),
            State::Error => (Display::Error, unacked),
            State::Ended => (Display::Ended, false),
            State::Done if !unacked => (Display::Idle, false),
            State::Done => {
                let provisional = sig.as_ref().is_some_and(|x| x.provisional);
                (Display::Done, !provisional || now - since >= PROVISIONAL_DONE_DELAY)
            }
        };
        let last_activity = s.last_activity();
        let active = raw != State::Ended && now - last_activity < ACTIVE_WINDOW;
        // Sessions that left the bar raise no alert.
        let alert = alert && active;
        let wsl = matches!(key.origin, Origin::Wsl(_));

        let mut r = Row {
            key: key.clone(),
            label: String::new(),
            title: String::new(),
            workspace: String::new(),
            cwd: None,
            branch: None,
            state,
            alert,
            reason: sig.as_ref().and_then(|x| x.reason.clone()),
            detail: sig.as_ref().and_then(|x| x.detail.clone()),
            state_since: since,
            activity: None,
            subagents: 0,
            context: None,
            context_window: None,
            context_approx: false,
            context_level: Level::Ok,
            cache_expires: None,
            cache_level: None,
            cache_expiring: false,
            model: None,
            effort: None,
            cost: Cost::default(),
            last_prompt: None,
            permission_mode: None,
            active,
            last_activity,
            history: s.history.iter().copied().collect(),
            resume_command: String::new(),
            focus_pid: None,
        };

        match key.runtime {
            Runtime::Claude => {
                let log = s.claude.as_ref();
                let entry = log.and_then(|l| l.entrypoint.as_deref()).or(s.status.as_ref().and_then(|x| x.entrypoint.as_deref()));
                r.label = claude::label(entry).to_string();
                r.title = log
                    .and_then(ClaudeLog::title)
                    .map(str::to_string)
                    .or(s.status.as_ref().and_then(|x| x.name.clone()))
                    .or(log.and_then(|l| l.first_prompt.clone()))
                    .unwrap_or_default();
                r.cwd = log
                    .and_then(|l| l.cwd.clone())
                    .or(s.hooks.cwd.clone())
                    .or(s.status.as_ref().and_then(|x| x.cwd.clone()));
                if let Some(l) = log {
                    r.branch = l.git_branch.clone();
                    r.activity = l.open_activity().map(str::to_string).or(l.activity.clone());
                    r.subagents = l.subagents();
                    r.context = l.context_tokens;
                    r.model = l.model.clone();
                    r.effort = l.effort.clone();
                    r.cost.usd = l.cost_usd;
                    r.last_prompt = l.last_prompt.clone();
                    r.permission_mode = l.permission_mode.clone();
                    r.cache_expires = l.cache_expires();
                }
                r.context_window = self.config.context_window(r.model.as_deref(), None);
                r.resume_command = format!("claude --resume {}", key.id);
                if !wsl {
                    r.focus_pid = s
                        .hooks
                        .host
                        .as_ref()
                        .and_then(|h| h.agent_pid)
                        .or(s.status.as_ref().map(|x| x.pid));
                }
            }
            Runtime::Copilot => {
                let log = s.copilot.as_ref();
                let ws = s.workspace.as_ref();
                r.label = copilot::label(ws.and_then(|w| w.client_name.as_deref()), s.lock.as_ref(), wsl).to_string();
                r.title = ws
                    .and_then(|w| w.name.clone())
                    .or(log.and_then(|l| l.first_prompt.clone()))
                    .unwrap_or_default();
                r.cwd = log.and_then(|l| l.cwd.clone()).or(ws.and_then(|w| w.cwd.clone())).or(s.hooks.cwd.clone());
                r.branch = log.and_then(|l| l.branch.clone()).or(ws.and_then(|w| w.branch.clone()));
                if let Some(l) = log {
                    r.activity = l.activity().map(str::to_string);
                    r.subagents = l.subagents();
                    r.context = l.context_tokens;
                    r.context_approx = l.context_approx;
                    r.model = l.model.clone();
                    r.effort = l.effort.clone();
                    r.cost.premium_requests = l.premium_requests;
                    r.cost.nano_aiu = l.nano_aiu;
                    r.last_prompt = l.last_prompt.clone();
                    r.permission_mode = l.permission_mode.clone();
                    r.cache_expires = l.cache_expires();
                }
                r.context_window =
                    self.config.context_window(r.model.as_deref(), log.and_then(|l| l.max_prompt_tokens));
                r.resume_command = format!("copilot --resume {}", key.id);
                if !wsl {
                    r.focus_pid = s.lock_pid;
                }
            }
        }
        r.workspace = r.cwd.as_deref().map(util::folder_name).unwrap_or_default();
        if r.title.is_empty() {
            r.title = if r.workspace.is_empty() { key.id.chars().take(8).collect() } else { r.workspace.clone() };
        }
        if let Some(c) = r.context {
            r.context_level = context_level(c, r.context_window);
        }
        if let Some(exp) = r.cache_expires {
            let left = exp - now;
            r.cache_level = Some(cache_level(left));
            r.cache_expiring = matches!(raw, State::Done) && left > 0.0 && left <= CACHE_EXPIRING;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse;

    fn sig(t: f64, state: State) -> Sig {
        Sig { t, state, reason: None, detail: None, provisional: false }
    }

    fn hook_line(t: f64, state: &str, reason: &str) -> pulse::PulseLine {
        pulse::parse_line(&format!(
            r#"{{"v":1,"t":{t},"runtime":"claude","session_id":"s","state":"{state}","reason":"{reason}","host":{{"os":"windows","agent_pid":42}}}}"#
        ))
        .unwrap()
    }

    fn status(t: f64, st: &str) -> ClaudeStatus {
        ClaudeStatus {
            pid: 7,
            session_id: "s".into(),
            cwd: None,
            entrypoint: Some("cli".into()),
            name: None,
            status: Some(st.into()),
            status_t: Some(t),
            proc_start: None,
        }
    }

    fn claude_session() -> Session {
        let mut log = ClaudeLog::new();
        log.sig = Some(sig(10.0, State::Working));
        log.last_write_t = Some(10.0);
        Session { claude: Some(log), ..Default::default() }
    }

    #[test]
    fn newest_source_wins() {
        let mut s = claude_session();
        s.hooks.apply(&hook_line(20.0, "needs_you", "permission_prompt"));
        assert_eq!(s.reduce().unwrap().state, State::NeedsYou);
        // The user approves: the status file turns busy at once.
        s.status = Some(status(25.0, "busy"));
        assert_eq!(s.reduce().unwrap().state, State::Working);
        s.hooks.apply(&hook_line(30.0, "done", ""));
        assert_eq!(s.reduce().unwrap().state, State::Done);
        // A transcript write older than the hook does not reopen the turn.
        s.claude.as_mut().unwrap().sig = Some(sig(29.0, State::Working));
        assert_eq!(s.reduce().unwrap().state, State::Done);
    }

    #[test]
    fn status_waiting_and_idle() {
        let mut s = claude_session();
        s.status = Some(status(20.0, "waiting"));
        assert_eq!(s.reduce().unwrap().state, State::NeedsYou);
        // Denied: idle right after waiting ends the turn.
        s.status = Some(status(21.0, "idle"));
        assert_eq!(s.reduce().unwrap().state, State::Done);
        // Idle after Done stays Done; idle on a fresh session is Idle.
        let fresh = Session { status: Some(status(1.0, "idle")), ..Default::default() };
        assert_eq!(fresh.reduce().unwrap().state, State::Idle);
    }

    #[test]
    fn same_state_keeps_details() {
        let mut s = claude_session();
        s.hooks.apply(&hook_line(20.0, "needs_you", "question"));
        s.status = Some(status(20.5, "waiting"));
        let r = s.reduce().unwrap();
        assert_eq!((r.state, r.reason.as_deref()), (State::NeedsYou, Some("question")));
    }

    #[test]
    fn not_live_ends_the_session() {
        let mut s = claude_session();
        s.live = Liveness::NotLive;
        s.live_t = 50.0;
        let r = s.reduce().unwrap();
        assert_eq!((r.state, r.t), (State::Ended, 50.0));
    }

    fn key() -> SessionKey {
        SessionKey { runtime: Runtime::Claude, origin: Origin::Windows, id: "s".into() }
    }

    #[test]
    fn alerts_and_acknowledgement() {
        let mut m = Model::default();
        let mut s = claude_session();
        s.hooks.apply(&hook_line(20.0, "done", ""));
        m.sessions.insert(key(), s);
        let r = m.row(&key(), &m.sessions[&key()], 21.0);
        assert_eq!((r.state, r.alert), (Display::Done, true));
        assert_eq!(r.focus_pid, Some(42));
        m.acknowledge(&key(), 22.0);
        let r = m.row(&key(), &m.sessions[&key()], 23.0);
        assert_eq!((r.state, r.alert), (Display::Idle, false));
    }

    #[test]
    fn provisional_done_alerts_after_delay() {
        let mut m = Model::default();
        let mut s = claude_session();
        s.claude.as_mut().unwrap().sig = Some(Sig { provisional: true, ..sig(20.0, State::Done) });
        m.sessions.insert(key(), s);
        assert!(!m.row(&key(), &m.sessions[&key()], 21.0).alert);
        assert!(m.row(&key(), &m.sessions[&key()], 23.5).alert);
    }

    #[test]
    fn activity_window_and_rows() {
        let mut m = Model::default();
        m.sessions.insert(key(), claude_session());
        let now = 10.0 + ACTIVE_WINDOW - 1.0;
        assert!(m.rows(now, 1e12).iter().any(|r| r.active));
        assert!(m.rows(10.0 + ACTIVE_WINDOW + 1.0, 1e12).is_empty());
        // Still listed as one of today's sessions.
        assert_eq!(m.rows(10.0 + ACTIVE_WINDOW + 1.0, 0.0).len(), 1);
    }

    #[test]
    fn levels() {
        assert_eq!(context_level(100_000, Some(200_000)), Level::Ok);
        assert_eq!(context_level(120_000, Some(200_000)), Level::Amber);
        assert_eq!(context_level(160_000, Some(200_000)), Level::Red);
        assert_eq!(context_level(300_000, None), Level::Amber);
        assert_eq!(context_level(500_000, Some(1_000_000)), Level::Red);
        assert_eq!(cache_level(16.0 * 60.0), Level::Ok);
        assert_eq!(cache_level(60.0), Level::Amber);
        assert_eq!(cache_level(0.0), Level::Red);
        let cfg = Config { context_windows: vec![("claude".into(), 1), ("claude-opus".into(), 2)] };
        assert_eq!(cfg.context_window(Some("claude-opus-5"), Some(9)), Some(2));
        assert_eq!(cfg.context_window(Some("gpt-6"), Some(9)), Some(9));
        assert_eq!(Config::default().context_window(Some("claude-x"), None), None);
        assert_eq!(Config::default().context_window(Some("claude-opus[1m]"), None), Some(1_000_000));
    }
}
