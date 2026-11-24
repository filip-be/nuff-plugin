//! Pulse Lines: the hook files written by the nuff-pulse-cc and
//! nuff-pulse-copilot plugins, `~/.nuff-pulse/sessions/<runtime>-<id>.hooks.jsonl`.
//! The format is owned and documented by those plugins' READMEs.

use crate::model::{Runtime, Sig, State};
use crate::util;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostInfo {
    pub os: Option<String>,
    pub distro: Option<String>,
    pub agent_pid: Option<u32>,
    pub agent_started: Option<f64>,
    pub pids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PulseLine {
    pub t: f64,
    pub runtime: Runtime,
    pub session_id: String,
    pub event: String,
    pub state: Option<State>,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub cwd: Option<String>,
    pub log_path: Option<String>,
    pub host: Option<HostInfo>,
}

pub fn parse_state(s: &str) -> Option<State> {
    Some(match s {
        "working" => State::Working,
        "needs_you" => State::NeedsYou,
        "done" => State::Done,
        "error" => State::Error,
        "ended" => State::Ended,
        _ => return None,
    })
}

/// Parses one Pulse Line; `None` for invalid JSON or a missing required field
/// (`v`, `t`, `runtime`, `session_id`).
pub fn parse_line(line: &str) -> Option<PulseLine> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("v")?.as_u64()? < 1 {
        return None;
    }
    let runtime = match util::s(&v, "runtime")? {
        "claude" => Runtime::Claude,
        "copilot" => Runtime::Copilot,
        _ => return None,
    };
    let session_id = util::s(&v, "session_id")?;
    if !valid_id(session_id) {
        return None;
    }
    let host = v.get("host").filter(|h| h.is_object()).map(|h| HostInfo {
        os: util::s(h, "os").map(str::to_string),
        distro: util::s(h, "distro").map(str::to_string),
        agent_pid: util::u(h, "agent_pid").and_then(|p| u32::try_from(p).ok()),
        agent_started: util::f(h, "agent_started"),
        pids: h
            .get("pids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_u64().and_then(|p| u32::try_from(p).ok()))
                    .collect()
            })
            .unwrap_or_default(),
    });
    let text = |k: &str| util::s(&v, k).map(str::to_string);
    Some(PulseLine {
        t: util::f(&v, "t")?,
        runtime,
        session_id: session_id.to_string(),
        event: text("event").unwrap_or_default(),
        state: util::s(&v, "state").and_then(parse_state),
        reason: text("reason"),
        detail: text("detail"),
        cwd: text("cwd"),
        log_path: text("log_path"),
        host,
    })
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

/// `claude-<id>.hooks.jsonl` → (Claude, id).
pub fn parse_file_name(name: &str) -> Option<(Runtime, String)> {
    let stem = name.strip_suffix(".hooks.jsonl")?;
    let (rt, id) = stem.split_once('-')?;
    let runtime = match rt {
        "claude" => Runtime::Claude,
        "copilot" => Runtime::Copilot,
        _ => return None,
    };
    valid_id(id).then(|| (runtime, id.to_string()))
}

/// What the hook lines of one session say so far.
#[derive(Debug, Clone, Default)]
pub struct HookState {
    /// Newest line with a state.
    pub sig: Option<Sig>,
    /// Newest line of any kind.
    pub last_t: Option<f64>,
    pub cwd: Option<String>,
    pub log_path: Option<String>,
    pub host: Option<HostInfo>,
    /// Session start reason (`startup`, `resume`, …) from the newest seed line.
    pub start_reason: Option<String>,
    /// Time of the line each field above came from (cwd, log_path, host, start).
    field_t: [f64; 4],
}

/// Sets `field` from `value` when `value` is present and `t` is not older
/// than the line the field came from.
fn merge<T: Clone>(field: &mut Option<T>, field_t: &mut f64, value: &Option<T>, t: f64) {
    if value.is_some() && (field.is_none() || t >= *field_t) {
        *field = value.clone();
        *field_t = t;
    }
}

impl HookState {
    /// Applies a line; lines may arrive out of order: per field, the newest
    /// line that has it wins.
    pub fn apply(&mut self, line: &PulseLine) {
        let t = line.t;
        self.last_t = Some(self.last_t.map_or(t, |x| x.max(t)));
        let [ct, lt, ht, st] = &mut self.field_t;
        merge(&mut self.cwd, ct, &line.cwd, t);
        merge(&mut self.log_path, lt, &line.log_path, t);
        merge(&mut self.host, ht, &line.host, t);
        if line.state.is_none() {
            merge(&mut self.start_reason, st, &line.reason, t);
        }
        if let Some(state) = line.state
            && self.sig.as_ref().is_none_or(|s| line.t >= s.t)
        {
            self.sig = Some(Sig {
                t: line.t,
                state,
                reason: line.reason.clone(),
                detail: line.detail.clone(),
                provisional: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"v":1,"t":1764000000.123,"runtime":"claude","session_id":"6f1c-ab_9","event":"PermissionRequest","state":"needs_you","reason":"permission_prompt","detail":"Allow Bash: dotnet ef","cwd":"D:\\work\\shop","log_path":"C:\\x.jsonl","host":{"os":"windows","pids":[1234,5678],"agent_pid":1234,"agent_started":1763999000.0},"extra":1}"#;

    #[test]
    fn parses_a_full_line() {
        let l = parse_line(LINE).unwrap();
        assert_eq!(l.runtime, Runtime::Claude);
        assert_eq!(l.session_id, "6f1c-ab_9");
        assert_eq!(l.state, Some(State::NeedsYou));
        assert_eq!(l.detail.as_deref(), Some("Allow Bash: dotnet ef"));
        let h = l.host.unwrap();
        assert_eq!(h.agent_pid, Some(1234));
        assert_eq!(h.pids, vec![1234, 5678]);
        assert_eq!(h.agent_started, Some(1763999000.0));
    }

    #[test]
    fn rejects_bad_lines() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"v":1,"t":1,"runtime":"other","session_id":"a"}"#).is_none());
        assert!(parse_line(r#"{"v":1,"t":1,"runtime":"claude","session_id":"../x"}"#).is_none());
        assert!(parse_line(r#"{"t":1,"runtime":"claude","session_id":"a"}"#).is_none());
        let min =
            parse_line(r#"{"v":1,"t":1,"runtime":"copilot","session_id":"a","state":"bogus"}"#)
                .unwrap();
        assert_eq!(min.state, None);
        assert!(min.host.is_none());
    }

    #[test]
    fn file_names() {
        assert_eq!(
            parse_file_name("claude-abc-1.hooks.jsonl"),
            Some((Runtime::Claude, "abc-1".into()))
        );
        assert_eq!(
            parse_file_name("copilot-x.hooks.jsonl"),
            Some((Runtime::Copilot, "x".into()))
        );
        assert_eq!(parse_file_name("claude-abc.jsonl"), None);
        assert_eq!(parse_file_name("other-abc.hooks.jsonl"), None);
    }

    #[test]
    fn newest_line_wins_even_out_of_order() {
        let mk = |t: f64, state: &str| {
            parse_line(&format!(r#"{{"v":1,"t":{t},"runtime":"claude","session_id":"a","state":"{state}","cwd":"c{t}"}}"#)).unwrap()
        };
        let mut h = HookState::default();
        h.apply(&mk(10.0, "done"));
        h.apply(&mk(5.0, "working"));
        assert_eq!(h.sig.as_ref().unwrap().state, State::Done);
        assert_eq!(h.cwd.as_deref(), Some("c10"));
        h.apply(&mk(11.0, "working"));
        assert_eq!(h.sig.unwrap().state, State::Working);
    }

    #[test]
    fn fields_merge_per_field() {
        let mut h = HookState::default();
        // A newer line without host, then an older line with it.
        h.apply(
            &parse_line(r#"{"v":1,"t":20,"runtime":"claude","session_id":"a","state":"done"}"#)
                .unwrap(),
        );
        h.apply(&parse_line(r#"{"v":1,"t":10,"runtime":"claude","session_id":"a","cwd":"C:\\x","host":{"os":"windows","agent_pid":5}}"#).unwrap());
        assert_eq!(h.host.as_ref().unwrap().agent_pid, Some(5));
        assert_eq!(h.cwd.as_deref(), Some("C:\\x"));
        assert_eq!(h.last_t, Some(20.0));
        h.apply(
            &parse_line(r#"{"v":1,"t":5,"runtime":"claude","session_id":"a","cwd":"C:\\old"}"#)
                .unwrap(),
        );
        assert_eq!(h.cwd.as_deref(), Some("C:\\x"));
    }
}
