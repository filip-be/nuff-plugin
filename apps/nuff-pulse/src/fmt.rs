//! Text and summaries the UI and the tray show, kept here so they are tested.

use crate::model::{Display, Row};

/// `950`, `12k`, `1.2M`.
pub fn tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

/// Context cell: `~124k/200k`, `124k`, or empty.
pub fn context(r: &Row) -> String {
    let approx = if r.context_approx { "~" } else { "" };
    match (r.context, r.context_window) {
        (Some(c), Some(w)) => format!("{approx}{}/{}", tokens(c), tokens(w)),
        (Some(c), None) => format!("{approx}{}", tokens(c)),
        _ => String::new(),
    }
}

/// Cache countdown in whole minutes, the unit the bar repaints at:
/// `cold`, `<1m`, `12m`, `1h05m`.
pub fn left(secs: f64) -> String {
    if secs <= 0.0 {
        "cold".into()
    } else if secs < 60.0 {
        "<1m".into()
    } else if secs >= 3600.0 {
        let m = (secs / 60.0) as u64;
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{}m", (secs / 60.0) as u64)
    }
}

/// Seconds until `left(expires - now)` changes, at most 60.
pub fn next_tick(expires: f64, now: f64) -> f64 {
    let l = expires - now;
    if l <= 0.0 {
        60.0
    } else {
        // `left` floors to whole minutes, so the text changes at each
        // multiple of 60 s left (and at 0).
        let r = l % 60.0;
        (if r < 0.05 { 60.0 } else { r }).min(60.0)
    }
}

/// Local clock time `HH:MM` of `t`, given the start of today (local).
pub fn clock(t: f64, today: f64) -> String {
    let s = (t - today).rem_euclid(86400.0) as u64;
    format!("{:02}:{:02}", s / 3600, (s / 60) % 60)
}

/// How long ago, compact: `now`, `5m`, `2h`.
pub fn ago(t: f64, now: f64) -> String {
    let d = (now - t).max(0.0);
    if d < 60.0 {
        "now".into()
    } else if d < 3600.0 {
        format!("{}m", (d / 60.0) as u64)
    } else {
        format!("{}h", (d / 3600.0) as u64)
    }
}

/// What the pill and the tray tooltip summarise.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub active: usize,
    pub working: usize,
    pub needs_you: usize,
    pub done: usize,
    pub error: usize,
    /// Rows with an alert (Needs you, Done, Error not yet seen).
    pub alerts: usize,
    /// A session needs you (seen or not): the tray blinks until answered.
    pub blink: bool,
    /// Smallest cache time left among active sessions, seconds.
    pub urgent_cache: Option<f64>,
}

pub fn summary(rows: &[Row], now: f64) -> Summary {
    let mut s = Summary::default();
    for r in rows.iter().filter(|r| r.active) {
        s.active += 1;
        match r.state {
            Display::Working => s.working += 1,
            Display::NeedsYou => {
                s.needs_you += 1;
                s.blink = true;
            }
            Display::Done => s.done += 1,
            Display::Error => s.error += 1,
            Display::Idle | Display::Ended => {}
        }
        if r.alert {
            s.alerts += 1;
        }
        if let Some(left) = r.cache_expires.map(|e| e - now).filter(|l| *l > 0.0) {
            s.urgent_cache = Some(s.urgent_cache.map_or(left, |u: f64| u.min(left)));
        }
    }
    s
}

/// Tray tooltip, at most 127 characters.
pub fn tooltip(s: &Summary) -> String {
    let mut parts = Vec::new();
    if s.needs_you > 0 {
        parts.push(format!("{} need you", s.needs_you));
    }
    if s.error > 0 {
        parts.push(format!("{} error", s.error));
    }
    if s.done > 0 {
        parts.push(format!("{} done", s.done));
    }
    if s.working > 0 {
        parts.push(format!("{} working", s.working));
    }
    let body = if s.active == 0 {
        "no active sessions".to_string()
    } else if parts.is_empty() {
        format!("{} idle", s.active)
    } else {
        parts.join(", ")
    };
    crate::util::short(&format!("nuff-pulse: {body}"), 127)
}

/// Resume command run in the session's folder, for the shell it runs in:
/// PowerShell for Windows sessions, a POSIX shell for WSL ones. The command
/// does not run when the folder cannot be entered.
pub fn resume(r: &Row) -> String {
    let Some(cwd) = r.cwd.as_deref().filter(|c| !c.is_empty()) else {
        return r.resume_command.clone();
    };
    match r.key.origin {
        crate::model::Origin::Windows => {
            format!(
                "Set-Location -LiteralPath '{}' -ErrorAction Stop; {}",
                cwd.replace('\'', "''"),
                r.resume_command
            )
        }
        crate::model::Origin::Wsl(_) => format!(
            "cd '{}' && {}",
            cwd.replace('\'', "'\\''"),
            r.resume_command
        ),
    }
}

/// `Claude CLI · shop` / `Copilot CLI · WSL Ubuntu · shop`.
/// Dollars per GitHub AI credit.
pub const USD_PER_AI_CREDIT: f64 = 0.01;

/// Cost in dollars: Claude's list-price total, or Copilot's AI credits at
/// $0.01 each. Empty when the session's log has no figure yet.
pub fn cost(r: &Row) -> String {
    if let Some(usd) = r.cost.usd {
        format!("${usd:.2}")
    } else if let Some(n) = r.cost.nano_aiu {
        format!("${:.2}", ai_credits(n) * USD_PER_AI_CREDIT)
    } else {
        String::new()
    }
}

/// The cost with Copilot's AI credits spelled out, for the detail pane.
pub fn cost_detail(r: &Row) -> String {
    match (r.cost.usd, r.cost.nano_aiu) {
        (None, Some(n)) => format!("{} ({:.1} AI credits)", cost(r), ai_credits(n)),
        _ => cost(r),
    }
}

fn ai_credits(nano: u64) -> f64 {
    nano as f64 / 1e9
}

pub fn place(r: &Row) -> String {
    let mut s = r.label.clone();
    if let crate::model::Origin::Wsl(d) = &r.key.origin {
        s.push_str(&format!(" · WSL {d}"));
    }
    if !r.workspace.is_empty() {
        s.push_str(" · ");
        s.push_str(&r.workspace);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Model, Origin, Runtime, SessionKey};

    fn row(state: Display, alert: bool, cache: Option<f64>) -> Row {
        let m = Model::default();
        let key = SessionKey {
            runtime: Runtime::Claude,
            origin: Origin::Windows,
            id: "abc".into(),
        };
        let mut r = m.row(&key, &Default::default(), 0.0);
        r.state = state;
        r.alert = alert;
        r.active = true;
        r.cache_expires = cache;
        r
    }

    #[test]
    fn countdowns() {
        assert_eq!(left(0.0), "cold");
        assert_eq!(left(59.0), "<1m");
        assert_eq!(left(61.0), "1m");
        assert_eq!(left(3599.0), "59m");
        assert_eq!(left(3600.0 + 5.0 * 60.0), "1h05m");
        assert!((next_tick(130.0, 0.0) - 10.0).abs() < 1e-9);
        assert_eq!(next_tick(120.0, 0.0), 60.0);
        assert_eq!(next_tick(-5.0, 0.0), 60.0);
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(124_500), "124k");
        assert_eq!(tokens(1_250_000), "1.2M");
        assert_eq!(clock(3600.0 * 9.0 + 65.0, 0.0), "09:01");
        assert_eq!(clock(-60.0, 0.0), "23:59");
        assert_eq!(ago(0.0, 7300.0), "2h");
    }

    #[test]
    fn summaries() {
        let rows = vec![
            row(Display::NeedsYou, true, None),
            row(Display::Done, true, Some(400.0)),
            row(Display::Working, false, Some(100.0)),
            row(Display::Idle, false, Some(-5.0)),
        ];
        let s = summary(&rows, 0.0);
        assert_eq!(
            (s.active, s.alerts, s.blink, s.working, s.done),
            (4, 2, true, 1, 1)
        );
        assert_eq!(s.urgent_cache, Some(100.0), "working sessions count too");
        assert_eq!(tooltip(&s), "nuff-pulse: 1 need you, 1 done, 1 working");
        assert_eq!(
            tooltip(&Summary::default()),
            "nuff-pulse: no active sessions"
        );
        // Seen but not answered: still blinks.
        assert!(summary(&[row(Display::NeedsYou, false, None)], 0.0).blink);
        assert!(!summary(&[row(Display::Done, true, None)], 0.0).blink);
    }

    #[test]
    fn resume_and_place() {
        let mut r = row(Display::Idle, false, None);
        r.resume_command = "claude --resume abc".into();
        assert_eq!(resume(&r), "claude --resume abc");
        r.cwd = Some("D:\\it's".into());
        assert_eq!(
            resume(&r),
            "Set-Location -LiteralPath 'D:\\it''s' -ErrorAction Stop; claude --resume abc"
        );
        r.label = "Copilot CLI".into();
        r.workspace = "shop".into();
        r.key.origin = Origin::Wsl("Ubuntu".into());
        assert_eq!(place(&r), "Copilot CLI · WSL Ubuntu · shop");
        r.cwd = Some("/home/o'brien".into());
        assert_eq!(resume(&r), "cd '/home/o'\\''brien' && claude --resume abc");
    }

    #[test]
    fn cost_in_dollars() {
        let mut r = row(Display::Idle, false, None);
        assert_eq!(cost(&r), "");
        r.cost.usd = Some(3.421);
        assert_eq!(
            (cost(&r), cost_detail(&r)),
            ("$3.42".into(), "$3.42".into())
        );
        r.cost.usd = None;
        // 6.66 AI credits at $0.01 each.
        r.cost.nano_aiu = Some(6_657_742_000);
        assert_eq!(
            (cost(&r), cost_detail(&r)),
            ("$0.07".into(), "$0.07 (6.7 AI credits)".into())
        );
    }
}
