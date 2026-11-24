//! Claude Code: transcript parser (`~/.claude/projects/<enc-cwd>/<id>.jsonl`)
//! and session status files (`~/.claude/sessions/<pid>.json`).

use crate::model::{Sig, State};
use crate::util::{self, short};
use serde_json::Value;

const CACHE_TTL_5M: f64 = 300.0;
const CACHE_TTL_1H: f64 = 3600.0;

#[derive(Debug, Clone)]
struct OpenTool {
    id: String,
    name: String,
    summary: String,
}

/// What a transcript says so far. Feed it lines in file order.
#[derive(Debug, Clone, Default)]
pub struct ClaudeLog {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub entrypoint: Option<String>,
    pub version: Option<String>,
    pub custom_title: Option<String>,
    pub ai_title: Option<String>,
    pub first_prompt: Option<String>,
    pub last_prompt: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    /// input + cache read + cache creation of the last main-thread request.
    pub context_tokens: Option<u64>,
    /// Time of the last main-thread request (its assistant record).
    pub last_request_t: Option<f64>,
    /// Cache TTL of the last request that wrote cache.
    pub cache_ttl: f64,
    /// Last tool call, `Name: argument`.
    pub activity: Option<String>,
    pub cost_usd: Option<f64>,
    /// Newest record timestamp.
    pub last_write_t: Option<f64>,
    /// State derived from the transcript alone.
    pub sig: Option<Sig>,
    open: Vec<OpenTool>,
}

impl ClaudeLog {
    pub fn new() -> Self {
        ClaudeLog { cache_ttl: CACHE_TTL_5M, ..Default::default() }
    }

    pub fn apply_line(&mut self, line: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            self.apply(&v);
        }
    }

    /// Drops open tool calls and the derived state; called between the head
    /// and the tail of a log whose middle was skipped.
    pub fn forget_open(&mut self) {
        self.open.clear();
        self.sig = None;
    }

    /// Running Agent/Task tool calls.
    pub fn subagents(&self) -> usize {
        self.open.iter().filter(|t| t.name == "Agent" || t.name == "Task").count()
    }

    /// The most recent tool call still running, else `None`.
    pub fn open_activity(&self) -> Option<&str> {
        self.open.last().map(|t| t.summary.as_str())
    }

    pub fn title(&self) -> Option<&str> {
        self.custom_title.as_deref().or(self.ai_title.as_deref())
    }

    /// Cache expiry, Unix seconds.
    pub fn cache_expires(&self) -> Option<f64> {
        self.last_request_t.map(|t| t + self.cache_ttl)
    }

    pub fn apply(&mut self, v: &Value) {
        let kind = util::s(v, "type").unwrap_or("");
        let t = util::s(v, "timestamp").and_then(util::parse_iso);
        if self.session_id.is_none() {
            self.session_id = util::s(v, "sessionId").map(str::to_string);
        }
        match kind {
            "custom-title" => self.custom_title = util::s(v, "customTitle").map(|x| short(x, 120)),
            "ai-title" => self.ai_title = util::s(v, "aiTitle").map(|x| short(x, 120)),
            "cost-state" => self.cost_usd = util::f(v, "totalCostUSD"),
            "permission-mode" => self.permission_mode = util::s(v, "permissionMode").map(str::to_string),
            _ => {}
        }
        if kind != "user" && kind != "assistant" {
            return;
        }
        let sidechain = v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false);
        if sidechain {
            return;
        }
        if let Some(t) = t {
            self.last_write_t = Some(self.last_write_t.map_or(t, |x| x.max(t)));
        }
        for (field, key) in [
            (&mut self.cwd, "cwd"),
            (&mut self.git_branch, "gitBranch"),
            (&mut self.entrypoint, "entrypoint"),
            (&mut self.version, "version"),
            (&mut self.permission_mode, "permissionMode"),
        ] {
            if let Some(x) = util::s(v, key) {
                *field = Some(x.to_string());
            }
        }
        let msg = v.get("message").unwrap_or(&Value::Null);
        if kind == "user" {
            self.apply_user(v, msg, t);
        } else {
            self.apply_assistant(v, msg, t);
        }
    }

    fn apply_user(&mut self, v: &Value, msg: &Value, t: Option<f64>) {
        let meta = v.get("isMeta").and_then(Value::as_bool).unwrap_or(false);
        let mut text = String::new();
        let mut tool_results = false;
        match msg.get("content") {
            Some(Value::String(s)) => text.push_str(s),
            Some(Value::Array(blocks)) => {
                for b in blocks {
                    match util::s(b, "type") {
                        Some("tool_result") => {
                            tool_results = true;
                            if let Some(id) = util::s(b, "tool_use_id") {
                                self.open.retain(|o| o.id != id);
                            }
                            if let Some(Value::Array(inner)) = b.get("content") {
                                for x in inner {
                                    if let Some(s) = util::s(x, "text") {
                                        text.push_str(s);
                                    }
                                }
                            } else if let Some(s) = util::s(b, "content") {
                                text.push_str(s);
                            }
                        }
                        Some("text") => text.push_str(util::s(b, "text").unwrap_or("")),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        let Some(t) = t else { return };
        if text.contains("[Request interrupted by user") {
            self.open.clear();
            self.set(t, State::Done, Some("interrupted"), false);
            return;
        }
        if tool_results {
            self.set(t, State::Working, None, false);
            return;
        }
        let trimmed = text.trim_start();
        let is_prompt = !meta
            && !trimmed.is_empty()
            && !trimmed.starts_with("<command-")
            && !trimmed.starts_with("<local-command")
            && !trimmed.starts_with("Caveat:");
        if is_prompt {
            let p = short(trimmed, 200);
            if self.first_prompt.is_none() {
                self.first_prompt = Some(short(trimmed, 60));
            }
            self.last_prompt = Some(p);
            self.activity = None;
            self.set(t, State::Working, None, false);
        }
    }

    fn apply_assistant(&mut self, v: &Value, msg: &Value, t: Option<f64>) {
        let model = util::s(msg, "model").filter(|m| !m.starts_with('<'));
        if let Some(m) = model {
            self.model = Some(m.to_string());
        }
        if let Some(e) = util::s(v, "effort") {
            self.effort = Some(e.to_string());
        }
        if let (Some(usage), Some(_)) = (msg.get("usage").filter(|u| u.is_object()), model) {
            let input = util::u(usage, "input_tokens").unwrap_or(0);
            let read = util::u(usage, "cache_read_input_tokens").unwrap_or(0);
            let created = util::u(usage, "cache_creation_input_tokens").unwrap_or(0);
            if input + read + created > 0 {
                self.context_tokens = Some(input + read + created);
                if let Some(t) = t {
                    self.last_request_t = Some(self.last_request_t.map_or(t, |x| x.max(t)));
                }
            }
            if created > 0 {
                let one_hour = usage
                    .get("cache_creation")
                    .and_then(|c| util::u(c, "ephemeral_1h_input_tokens"))
                    .unwrap_or(0);
                self.cache_ttl = if one_hour > 0 { CACHE_TTL_1H } else { CACHE_TTL_5M };
            }
        }
        if let Some(Value::Array(blocks)) = msg.get("content") {
            for b in blocks {
                if util::s(b, "type") == Some("tool_use") {
                    let name = util::s(b, "name").unwrap_or("tool").to_string();
                    let summary = tool_summary(&name, b.get("input").unwrap_or(&Value::Null));
                    self.activity = Some(summary.clone());
                    if let Some(id) = util::s(b, "id") {
                        if !self.open.iter().any(|o| o.id == id) {
                            self.open.push(OpenTool { id: id.to_string(), name, summary });
                        }
                    }
                }
            }
        }
        let Some(t) = t else { return };
        let stop = util::s(msg, "stop_reason");
        if matches!(stop, Some("end_turn") | Some("stop_sequence")) && self.open.is_empty() {
            self.set(t, State::Done, None, true);
        } else {
            self.set(t, State::Working, None, false);
        }
    }

    fn set(&mut self, t: f64, state: State, reason: Option<&str>, provisional: bool) {
        if self.sig.as_ref().is_some_and(|s| t < s.t) {
            return;
        }
        self.sig = Some(Sig { t, state, reason: reason.map(str::to_string), detail: None, provisional });
    }
}

/// `Bash: git status`, `Read: main.rs`, `Agent: Count files`.
pub fn tool_summary(name: &str, input: &Value) -> String {
    let arg = ["description", "command", "file_path", "path", "pattern", "url", "query", "prompt", "skill"]
        .iter()
        .find_map(|k| util::s(input, k));
    match arg {
        Some(a) => {
            let a = if a.contains(['\\', '/']) && !a.contains(' ') { util::folder_name(a) } else { a.to_string() };
            format!("{name}: {}", short(&a, 60))
        }
        None => name.to_string(),
    }
}

/// `~/.claude/sessions/<pid>.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeStatus {
    pub pid: u32,
    pub session_id: String,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub status_t: Option<f64>,
    /// Process creation time, Unix seconds (from `procStart`, FILETIME ticks).
    pub proc_start: Option<f64>,
}

impl ClaudeStatus {
    pub fn parse(text: &str) -> Option<ClaudeStatus> {
        let v: Value = serde_json::from_str(text).ok()?;
        let proc_start = match v.get("procStart") {
            Some(Value::String(s)) => s.parse::<u64>().ok().map(util::filetime_to_unix),
            Some(Value::Number(n)) => n.as_u64().map(util::filetime_to_unix),
            _ => None,
        };
        Some(ClaudeStatus {
            pid: u32::try_from(util::u(&v, "pid")?).ok()?,
            session_id: util::s(&v, "sessionId")?.to_string(),
            cwd: util::s(&v, "cwd").map(str::to_string),
            entrypoint: util::s(&v, "entrypoint").map(str::to_string),
            name: util::s(&v, "name").map(|x| short(x, 120)),
            status: util::s(&v, "status").map(str::to_string),
            status_t: util::f(&v, "statusUpdatedAt").map(|ms| ms / 1000.0),
            proc_start,
        })
    }

    /// `waiting` → Needs you, `busy` → Working, `idle` → Idle (the reducer
    /// turns an `idle` after Working or Needs you into Done).
    pub fn sig(&self) -> Option<Sig> {
        let state = match self.status.as_deref()? {
            "waiting" => State::NeedsYou,
            "busy" => State::Working,
            "idle" => State::Idle,
            _ => return None,
        };
        // No reason: `waiting` covers permission prompts and questions alike;
        // the hook line that raised it carries the details.
        Some(Sig { t: self.status_t?, state, reason: None, detail: None, provisional: false })
    }
}

/// Display label for a Claude entrypoint.
pub fn label(entrypoint: Option<&str>) -> &'static str {
    match entrypoint {
        Some("cli") => "Claude CLI",
        Some("claude-vscode") => "Claude · VS Code",
        Some("claude-desktop") => "Claude app",
        _ => "Claude",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> ClaudeLog {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let mut log = ClaudeLog::new();
        for line in text.lines() {
            log.apply_line(line);
        }
        log
    }

    #[test]
    fn fixture_01_fields() {
        let log = fixture("claude-transcript-01.jsonl");
        assert_eq!(log.session_id.as_deref(), Some("id-0001"));
        assert_eq!(log.entrypoint.as_deref(), Some("claude-desktop"));
        assert_eq!(log.cwd.as_deref(), Some("C:\\work\\project"));
        assert_eq!(log.git_branch.as_deref(), Some("feature"));
        assert_eq!(log.custom_title.as_deref(), Some("<str:31>"));
        assert_eq!(log.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(log.effort.as_deref(), Some("medium"));
        assert!(log.context_tokens.unwrap() > 70_000);
        assert_eq!(log.cache_ttl, CACHE_TTL_1H);
        assert!(log.last_request_t.is_some());
        assert!(log.sig.is_some());
        assert_eq!(label(log.entrypoint.as_deref()), "Claude app");
    }

    #[test]
    fn fixture_02_fields() {
        let log = fixture("claude-transcript-02.jsonl");
        assert_eq!(log.entrypoint.as_deref(), Some("claude-vscode"));
        assert_eq!(log.cost_usd, Some(0.0));
        assert_eq!(log.context_tokens, None);
        // A local command is not a prompt.
        assert_eq!(log.first_prompt.as_deref(), Some("<str:130>"));
    }

    fn rec(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn turn_states() {
        let mut log = ClaudeLog::new();
        log.apply(&rec(r#"{"type":"user","timestamp":"2025-11-24T16:00:00Z","message":{"role":"user","content":"Fix the build"}}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Working);
        assert_eq!(log.first_prompt.as_deref(), Some("Fix the build"));
        log.apply(&rec(r#"{"type":"assistant","timestamp":"2025-11-24T16:00:01Z","message":{"model":"claude-x","stop_reason":"tool_use","usage":{"input_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":5,"cache_creation":{"ephemeral_5m_input_tokens":5}},"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo build"}},{"type":"tool_use","id":"t2","name":"Agent","input":{"description":"Explore code"}}]}}"#));
        assert_eq!(log.context_tokens, Some(115));
        assert_eq!(log.cache_ttl, CACHE_TTL_5M);
        assert_eq!(log.subagents(), 1);
        assert_eq!(log.activity.as_deref(), Some("Agent: Explore code"));
        log.apply(&rec(r#"{"type":"user","timestamp":"2025-11-24T16:00:02Z","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"ok"}]}}"#));
        assert_eq!(log.subagents(), 0);
        assert_eq!(log.open_activity(), Some("Bash: cargo build"));
        log.apply(&rec(r#"{"type":"user","timestamp":"2025-11-24T16:00:03Z","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#));
        log.apply(&rec(r#"{"type":"assistant","timestamp":"2025-11-24T16:00:04Z","message":{"model":"claude-x","stop_reason":"end_turn","content":[{"type":"text","text":"done"}]}}"#));
        let sig = log.sig.clone().unwrap();
        assert_eq!(sig.state, State::Done);
        assert!(sig.provisional);
        // Sidechain records are ignored.
        log.apply(&rec(r#"{"type":"assistant","isSidechain":true,"timestamp":"2025-11-24T16:00:09Z","message":{"model":"claude-x","stop_reason":"tool_use"}}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Done);
        log.apply(&rec(r#"{"type":"user","timestamp":"2025-11-24T16:00:10Z","message":{"content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#));
        assert_eq!(log.sig.as_ref().unwrap().reason.as_deref(), Some("interrupted"));
    }

    #[test]
    fn status_file() {
        let st = ClaudeStatus::parse(r#"{"pid":16624,"sessionId":"c9f6","cwd":"D:\\x","procStart":"134400005932987732","entrypoint":"claude-desktop","status":"waiting","statusUpdatedAt":1795516876707}"#).unwrap();
        assert_eq!(st.pid, 16624);
        assert!((st.proc_start.unwrap() - 1795526993.3).abs() < 0.01);
        let sig = st.sig().unwrap();
        assert_eq!(sig.state, State::NeedsYou);
        assert!((sig.t - 1795516876.707).abs() < 1e-6);
        assert!(ClaudeStatus::parse("{}").is_none());
    }
}
