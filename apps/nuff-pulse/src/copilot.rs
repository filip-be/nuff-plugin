//! GitHub Copilot: `~/.copilot/session-state/<id>/events.jsonl` and
//! `workspace.yaml`. Only the event envelope `id, timestamp, parentId, type,
//! data` is assumed; everything else is read tolerantly.
//!
//! Events with an envelope `agentId` or a `data.parentToolCallId` belong to a
//! subagent: they only feed the subagent count, except permission and
//! `ask_user` prompts, which wait for the user whoever asked.

use crate::model::{Sig, State};
use crate::util::{self, short};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Re-check delay before a provisional (disk-only) Done raises its alert.
pub const PROVISIONAL_DONE_DELAY: f64 = 3.0;

#[derive(Debug, Clone, Default)]
pub struct CacheEntry {
    pub expires: Option<f64>,
    pub ttl: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct CopilotLog {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub repository: Option<String>,
    pub producer: Option<String>,
    pub version: Option<String>,
    pub first_prompt: Option<String>,
    pub last_prompt: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    /// input + cache read + cache write of the last main-thread request.
    pub context_tokens: Option<u64>,
    /// True when `context_tokens` comes from `session.shutdown currentTokens`.
    pub context_approx: bool,
    /// `maxPromptTokens` of the last main-thread request.
    pub max_prompt_tokens: Option<u64>,
    pub last_request_t: Option<f64>,
    /// `usage.cacheExpiresAt` of the last main-thread request.
    pub request_cache_expires: Option<f64>,
    /// Model of the last main-thread request.
    pub last_request_model: Option<String>,
    pub checkpoint_t: Option<f64>,
    /// Per model, from the newest `session.usage_checkpoint`.
    pub cache_state: HashMap<String, CacheEntry>,
    /// GitHub AI credits used so far, in billionths (`totalNanoAiu`).
    pub nano_aiu: Option<u64>,
    pub last_write_t: Option<f64>,
    pub sig: Option<Sig>,
    open_tools: Vec<(String, String)>,
    open_permissions: HashSet<String>,
    open_asks: HashSet<String>,
    subagents: HashSet<String>,
    turn_requested_tools: bool,
    /// The current turn wrote an `assistant.message`.
    turn_has_message: bool,
}

fn is_subagent(v: &Value, data: &Value) -> bool {
    util::s(v, "agentId").is_some() || util::s(data, "parentToolCallId").is_some()
}

impl CopilotLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_line(&mut self, line: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            self.apply(&v);
        }
    }

    pub fn subagents(&self) -> usize {
        self.subagents.len()
    }

    /// The most recent main-thread tool still running: its `toolTitle`, else
    /// its `toolName`.
    pub fn activity(&self) -> Option<&str> {
        self.open_tools.last().map(|(_, label)| label.as_str())
    }

    /// Drops everything that pairs a start with a later completion (open
    /// tools, prompts, subagents) and the derived state. Called between the
    /// head and the tail of a log whose middle was skipped: a start in the
    /// head may have completed in the middle.
    pub fn forget_open(&mut self) {
        self.open_tools.clear();
        self.open_permissions.clear();
        self.open_asks.clear();
        self.subagents.clear();
        self.turn_requested_tools = false;
        self.turn_has_message = false;
        self.sig = None;
    }

    fn blocked(&self) -> bool {
        !self.open_permissions.is_empty() || !self.open_asks.is_empty()
    }

    /// Cache lifetime of the current model, seconds.
    pub fn cache_ttl(&self) -> Option<f64> {
        let model = self.model.as_deref()?;
        Some(self.cache_state.get(model).and_then(|e| e.ttl).unwrap_or_else(|| default_ttl(model)))
    }

    /// Cache expiry of the current model, Unix seconds.
    pub fn cache_expires(&self) -> Option<f64> {
        let model = self.model.as_deref();
        let entry = model.and_then(|m| self.cache_state.get(m));
        let checkpoint_newer = match (self.checkpoint_t, self.last_request_t) {
            (Some(c), Some(r)) => c >= r,
            (Some(_), None) => true,
            _ => false,
        };
        if checkpoint_newer {
            if let Some(exp) = entry.and_then(|e| e.expires) {
                return Some(exp);
            }
        }
        // The last request only tells about its own model's cache; after a
        // model switch the new model has no cache yet.
        if self.last_request_t.is_some() && self.last_request_model.as_deref() != model {
            return None;
        }
        if let Some(exp) = self.request_cache_expires {
            return Some(exp);
        }
        let t = self.last_request_t?;
        let ttl = entry.and_then(|e| e.ttl).unwrap_or_else(|| default_ttl(model.unwrap_or("")));
        Some(t + ttl)
    }

    pub fn apply(&mut self, v: &Value) {
        let Some(kind) = util::s(v, "type") else { return };
        let data = v.get("data").unwrap_or(&Value::Null);
        let t = util::s(v, "timestamp").and_then(util::parse_iso);
        if let Some(t) = t {
            self.last_write_t = Some(self.last_write_t.map_or(t, |x| x.max(t)));
        }
        let sub = is_subagent(v, data);

        // Thread-independent: subagent lifecycle and user prompts.
        match kind {
            "subagent.started" => {
                if let Some(id) = util::s(data, "toolCallId") {
                    self.subagents.insert(id.to_string());
                }
                return;
            }
            "subagent.completed" | "subagent.failed" => {
                if let Some(id) = util::s(data, "toolCallId") {
                    self.subagents.remove(id);
                }
                return;
            }
            "permission.requested" => {
                if let Some(id) = util::s(data, "requestId") {
                    self.open_permissions.insert(id.to_string());
                }
                if let Some(m) = util::s(data, "permissionMode") {
                    self.permission_mode = Some(m.to_string());
                }
                if let Some(t) = t {
                    self.set(t, State::NeedsYou, Some("permission_prompt"), false);
                }
                return;
            }
            "permission.completed" => {
                if let Some(id) = util::s(data, "requestId") {
                    self.open_permissions.remove(id);
                }
                if let Some(t) = t {
                    self.unblock(t);
                }
                return;
            }
            _ => {}
        }
        if kind == "tool.execution_start" && util::s(data, "toolName") == Some("ask_user") {
            if let Some(id) = util::s(data, "toolCallId") {
                self.open_asks.insert(id.to_string());
            }
            if let Some(t) = t {
                self.set(t, State::NeedsYou, Some("question"), false);
            }
        }
        if kind == "tool.execution_complete" {
            if let Some(id) = util::s(data, "toolCallId") {
                if self.open_asks.remove(id) {
                    if let Some(t) = t {
                        self.unblock(t);
                    }
                }
            }
        }
        if sub {
            return;
        }

        match kind {
            "session.start" | "session.resume" => {
                if let Some(ctx) = data.get("context") {
                    self.cwd = util::s(ctx, "cwd").map(str::to_string).or(self.cwd.take());
                    self.branch = util::s(ctx, "branch").map(str::to_string);
                    self.repository = util::s(ctx, "repository").map(str::to_string);
                }
                if let Some(id) = util::s(data, "sessionId") {
                    self.session_id = Some(id.to_string());
                }
                if let Some(p) = util::s(data, "producer") {
                    self.producer = Some(p.to_string());
                }
                if let Some(p) = util::s(data, "copilotVersion") {
                    self.version = Some(p.to_string());
                }
                if let Some(m) = util::s(data, "selectedModel") {
                    self.model = Some(m.to_string());
                }
                if let Some(e) = util::s(data, "reasoningEffort") {
                    self.effort = Some(e.to_string());
                }
                // A new process: nothing from before is still open.
                self.open_tools.clear();
                self.open_permissions.clear();
                self.open_asks.clear();
                self.subagents.clear();
                if let Some(t) = t {
                    self.set(t, State::Idle, None, false);
                }
            }
            "session.model_change" => {
                if let Some(m) = util::s(data, "newModel") {
                    self.model = Some(m.to_string());
                }
                if let Some(e) = util::s(data, "reasoningEffort") {
                    self.effort = Some(e.to_string());
                }
            }
            "user.message" => {
                let content = util::s(data, "content").unwrap_or("").trim();
                let from_user = matches!(util::s(data, "source"), None | Some("user"));
                if from_user && !content.is_empty() {
                    if self.first_prompt.is_none() {
                        self.first_prompt = Some(short(content, 60));
                    }
                    self.last_prompt = Some(short(content, 200));
                    if let Some(t) = t {
                        self.set(t, State::Working, None, false);
                    }
                }
            }
            "assistant.turn_start" => {
                self.turn_requested_tools = false;
                self.turn_has_message = false;
                if let Some(m) = util::s(data, "model") {
                    self.model = Some(m.to_string());
                }
                if let Some(t) = t {
                    self.set_unless_blocked(t, State::Working);
                }
            }
            "assistant.message" => {
                if let Some(m) = util::s(data, "model") {
                    self.model = Some(m.to_string());
                }
                let requests = data.get("toolRequests").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                self.turn_requested_tools = requests;
                self.turn_has_message = true;
                if let Some(t) = t {
                    self.set_unless_blocked(t, State::Working);
                }
            }
            "assistant.turn_end" => {
                let idle = self.turn_has_message
                    && !self.turn_requested_tools
                    && self.open_tools.is_empty()
                    && !self.blocked()
                    && self.subagents.is_empty();
                if let (true, Some(t)) = (idle, t) {
                    self.set(t, State::Done, None, true);
                }
            }
            "tool.execution_start" => {
                if let Some(id) = util::s(data, "toolCallId") {
                    let label = util::s(data, "toolTitle").or(util::s(data, "toolName")).unwrap_or("tool");
                    self.open_tools.push((id.to_string(), short(label, 60)));
                }
                if let Some(t) = t {
                    self.set_unless_blocked(t, State::Working);
                }
            }
            "tool.execution_complete" => {
                if let Some(id) = util::s(data, "toolCallId") {
                    self.open_tools.retain(|(x, _)| x != id);
                }
                if let Some(t) = t {
                    self.set_unless_blocked(t, State::Working);
                }
            }
            "session.usage_record" => {
                let usage = data.get("usage").unwrap_or(&Value::Null);
                let input = util::u(usage, "inputTokens").unwrap_or(0);
                let read = util::u(usage, "cacheReadTokens").unwrap_or(0);
                let write = util::u(usage, "cacheWriteTokens").unwrap_or(0);
                if input + read + write > 0 {
                    self.context_tokens = Some(input + read + write);
                    self.context_approx = false;
                }
                if let Some(m) = util::s(usage, "model") {
                    self.model = Some(m.to_string());
                }
                // Without `usage.model` the request ran on the selected model.
                self.last_request_model = self.model.clone();
                if let Some(mp) = util::u(usage, "maxPromptTokens") {
                    self.max_prompt_tokens = Some(mp);
                }
                self.request_cache_expires = util::s(usage, "cacheExpiresAt").and_then(util::parse_iso);
                if let Some(t) = t {
                    self.last_request_t = Some(t);
                }
            }
            "session.usage_checkpoint" => {
                self.checkpoint_t = t.or(self.checkpoint_t);
                self.read_cost(data);
                if let Some(arr) = data.get("modelCacheState").and_then(Value::as_array) {
                    for e in arr {
                        if let Some(id) = util::s(e, "modelId") {
                            self.cache_state.insert(
                                id.to_string(),
                                CacheEntry {
                                    expires: util::s(e, "cacheExpiresAt").and_then(util::parse_iso),
                                    ttl: util::f(e, "cacheTtlSeconds"),
                                },
                            );
                        }
                    }
                }
            }
            "session.error" => {
                if let Some(t) = t {
                    let reason = util::s(data, "errorType").unwrap_or("error").to_string();
                    let detail = util::s(data, "message").map(|m| short(m, 200));
                    if self.sig.as_ref().is_none_or(|s| t >= s.t) {
                        self.sig = Some(Sig { t, state: State::Error, reason: Some(reason), detail, provisional: false });
                    }
                }
            }
            "abort" => {
                self.open_tools.clear();
                self.open_asks.clear();
                self.open_permissions.clear();
                if let Some(t) = t {
                    self.set(t, State::Done, Some("aborted"), false);
                }
            }
            "session.shutdown" => {
                self.read_cost(data);
                if self.context_tokens.is_none() {
                    if let Some(c) = util::u(data, "currentTokens") {
                        self.context_tokens = Some(c);
                        self.context_approx = true;
                    }
                }
                if let Some(m) = util::s(data, "currentModel") {
                    self.model = Some(m.to_string());
                }
                if let Some(t) = t {
                    self.set(t, State::Ended, util::s(data, "shutdownType"), false);
                }
            }
            _ => {}
        }
    }

    fn read_cost(&mut self, data: &Value) {
        if let Some(n) = util::u(data, "totalNanoAiu") {
            self.nano_aiu = Some(n);
        }
    }

    /// A blocker was answered: back to Working when nothing else waits.
    fn unblock(&mut self, t: f64) {
        if !self.blocked() && self.sig.as_ref().is_some_and(|s| s.state == State::NeedsYou) {
            self.set(t, State::Working, None, false);
        }
    }

    fn set_unless_blocked(&mut self, t: f64, state: State) {
        if self.blocked() {
            return;
        }
        self.set(t, state, None, false);
    }

    fn set(&mut self, t: f64, state: State, reason: Option<&str>, provisional: bool) {
        if self.sig.as_ref().is_some_and(|s| t < s.t) {
            return;
        }
        self.sig = Some(Sig { t, state, reason: reason.map(str::to_string), detail: None, provisional });
    }
}

/// Copilot cache TTL by model family when no checkpoint names it:
/// Claude 300 s, GPT 1800 s (observed), otherwise 300 s.
pub fn default_ttl(model: &str) -> f64 {
    if model.starts_with("gpt") || model.starts_with("o") && model[1..].starts_with(|c: char| c.is_ascii_digit()) {
        1800.0
    } else {
        300.0
    }
}

/// `workspace.yaml`: flat `key: value` lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Workspace {
    pub id: Option<String>,
    pub cwd: Option<String>,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub client_name: Option<String>,
    pub name: Option<String>,
    pub user_named: bool,
}

impl Workspace {
    pub fn parse(text: &str) -> Workspace {
        let mut w = Workspace::default();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with([' ', '\t', '#', '-']) {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else { continue };
            let raw = value.trim();
            let value = if raw.starts_with(['|', '>']) {
                // Block scalar: the indented lines that follow, joined.
                let block: Vec<&str> = lines[i + 1..]
                    .iter()
                    .take_while(|l| l.starts_with([' ', '\t']) || l.trim().is_empty())
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect();
                block.join(" ")
            } else {
                unquote(raw)
            };
            if value.is_empty() {
                continue;
            }
            let value = Some(value);
            match key.trim() {
                "id" => w.id = value,
                "cwd" => w.cwd = value,
                "repository" => w.repository = value,
                "branch" => w.branch = value,
                "client_name" => w.client_name = value,
                "name" => w.name = value.map(|n| short(&n, 120)),
                "user_named" => w.user_named = value.as_deref() == Some("true"),
                _ => {}
            }
        }
        w
    }
}

fn unquote(v: &str) -> String {
    if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
        let inner = &v[1..v.len() - 1];
        if v.starts_with('\'') {
            inner.replace("''", "'")
        } else {
            inner.replace("\\\"", "\"").replace("\\\\", "\\")
        }
    } else {
        v.to_string()
    }
}

/// Facts about the process holding a session's `inuse.<pid>.lock`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LockProcess {
    pub exe: Option<String>,
    pub parent_exe: Option<String>,
}

/// Display label for a Copilot session.
pub fn label(client_name: Option<&str>, lock: Option<&LockProcess>, wsl: bool) -> &'static str {
    let client = client_name.unwrap_or("");
    if client == "vscode-agent-host" {
        return "Copilot · VS Code";
    }
    if wsl {
        return "Copilot CLI";
    }
    if client.starts_with("github") {
        if let Some(lp) = lock {
            let exe = lp.exe.as_deref().unwrap_or("").to_ascii_lowercase();
            let parent = lp.parent_exe.as_deref().unwrap_or("").to_ascii_lowercase();
            if exe == "copilot-runtime.exe" && parent == "github.exe" {
                return "Copilot app";
            }
            if exe == "copilot.exe" {
                return "Copilot CLI";
            }
        }
        if client == "github/cli" {
            return "Copilot CLI";
        }
    }
    "Copilot"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> (CopilotLog, Workspace) {
        let dir = format!("{}/tests/fixtures/copilot/{name}", env!("CARGO_MANIFEST_DIR"));
        let mut log = CopilotLog::new();
        for line in std::fs::read_to_string(format!("{dir}/events.jsonl")).unwrap().lines() {
            log.apply_line(line);
        }
        let ws = Workspace::parse(&std::fs::read_to_string(format!("{dir}/workspace.yaml")).unwrap());
        (log, ws)
    }

    #[test]
    fn vscode_session() {
        let (log, ws) = fixture("vscode");
        assert_eq!(ws.client_name.as_deref(), Some("vscode-agent-host"));
        assert_eq!(ws.name.as_deref(), Some("Test prompt session"));
        assert!(!ws.user_named);
        assert_eq!(log.session_id.as_deref(), Some("00000000-0000-4000-8000-000000000002"));
        assert_eq!(log.cwd.as_deref(), Some("C:\\work\\project"));
        assert_eq!(log.branch.as_deref(), Some("feature"));
        assert_eq!(log.repository.as_deref(), Some("owner/project"));
        // Main-thread model, not the subagent's.
        assert_eq!(log.model.as_deref(), Some("gpt-6.1-sol"));
        // Last main-thread usage record: 22474 + 22379 + 92.
        assert_eq!(log.context_tokens, Some(22474 + 22379 + 92));
        assert!(!log.context_approx);
        assert_eq!(log.subagents(), 0);
        assert_eq!(log.nano_aiu, Some(6657742000));
        // Checkpoint is newer than the last request: its expiry wins.
        let ttl = log.cache_state.get("gpt-6.1-sol").unwrap().ttl;
        assert_eq!(ttl, Some(1800.0));
        assert_eq!(log.cache_expires(), log.cache_state["gpt-6.1-sol"].expires);
        assert_eq!(log.sig.as_ref().unwrap().state, State::Ended);
        assert_eq!(label(ws.client_name.as_deref(), None, false), "Copilot · VS Code");
    }

    #[test]
    fn cli_session() {
        let (log, ws) = fixture("cli");
        assert_eq!(ws.client_name.as_deref(), Some("github/cli"));
        assert_eq!(log.model.as_deref(), Some("gemini-4-hydrogen"));
        assert_eq!(log.effort.as_deref(), Some("medium"));
        assert_eq!(log.first_prompt.as_deref().map(|p| p.starts_with("<str:")), Some(true));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Ended);
        assert_eq!(label(ws.client_name.as_deref(), None, false), "Copilot CLI");
    }

    #[test]
    fn app_session() {
        let (log, ws) = fixture("app");
        assert!(ws.user_named);
        assert_eq!(ws.name.as_deref(), Some("Copilot 1b verification"));
        assert!(log.context_tokens.unwrap() > 0);
        let lock = LockProcess { exe: Some("copilot-runtime.exe".into()), parent_exe: Some("GitHub.exe".into()) };
        assert_eq!(label(ws.client_name.as_deref(), Some(&lock), false), "Copilot app");
        assert_eq!(label(ws.client_name.as_deref(), None, true), "Copilot CLI");
        assert_eq!(label(ws.client_name.as_deref(), None, false), "Copilot");
    }

    /// Replays the VS Code fixture and records the state after each event.
    #[test]
    fn vscode_state_sequence() {
        let dir = format!("{}/tests/fixtures/copilot/vscode", env!("CARGO_MANIFEST_DIR"));
        let mut log = CopilotLog::new();
        let mut seq: Vec<State> = Vec::new();
        let mut needs_you_reasons = Vec::new();
        for line in std::fs::read_to_string(format!("{dir}/events.jsonl")).unwrap().lines() {
            log.apply_line(line);
            let st = log.sig.as_ref().map(|s| s.state).unwrap_or(State::Idle);
            if seq.last() != Some(&st) {
                seq.push(st);
                if st == State::NeedsYou {
                    needs_you_reasons.push(log.sig.as_ref().unwrap().reason.clone().unwrap());
                }
            }
        }
        // Subagent permission prompt, main-thread host-policy permission
        // (answered at once), ask_user question; then the final answer and
        // shutdown.
        assert_eq!(
            seq,
            vec![
                State::Idle,
                State::Working,
                State::NeedsYou,
                State::Working,
                State::NeedsYou,
                State::Working,
                State::NeedsYou,
                State::Working,
                State::Done,
                State::Ended
            ]
        );
        assert_eq!(needs_you_reasons, vec!["permission_prompt", "permission_prompt", "question"]);
    }

    fn ev(kind: &str, t: &str, data: &str) -> Value {
        serde_json::from_str(&format!(r#"{{"type":"{kind}","id":"x","timestamp":"2025-11-24T16:00:{t}Z","parentId":null,"data":{data}}}"#)).unwrap()
    }

    #[test]
    fn provisional_done_is_superseded() {
        let mut log = CopilotLog::new();
        log.apply(&ev("assistant.turn_start", "01", r#"{"turnId":"0"}"#));
        log.apply(&ev("assistant.message", "02", r#"{"toolRequests":[{"toolCallId":"a","name":"view"}]}"#));
        log.apply(&ev("tool.execution_start", "03", r#"{"toolCallId":"a","toolName":"view"}"#));
        assert_eq!(log.activity(), Some("view"));
        log.apply(&ev("tool.execution_complete", "04", r#"{"toolCallId":"a"}"#));
        log.apply(&ev("assistant.turn_end", "05", r#"{"turnId":"0"}"#));
        // The turn's last message requested tools: not Done.
        assert_eq!(log.sig.as_ref().unwrap().state, State::Working);
        log.apply(&ev("assistant.turn_start", "06", r#"{"turnId":"1"}"#));
        log.apply(&ev("assistant.message", "07", r#"{"toolRequests":[]}"#));
        log.apply(&ev("assistant.turn_end", "08", r#"{"turnId":"1"}"#));
        let s = log.sig.clone().unwrap();
        assert_eq!((s.state, s.provisional), (State::Done, true));
        log.apply(&ev("assistant.turn_start", "09", r#"{"turnId":"2"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Working);
        log.apply(&ev("session.error", "10", r#"{"errorType":"query","message":"boom"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Error);
        log.apply(&ev("abort", "11", r#"{"reason":"user_initiated"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Done);
    }

    #[test]
    fn cache_falls_back_to_request_time_plus_ttl() {
        let mut log = CopilotLog::new();
        log.apply(&ev("session.usage_checkpoint", "00", r#"{"modelCacheState":[{"modelId":"claude-x","cacheExpiresAt":"2025-11-24T16:05:00Z","cacheTtlSeconds":300}]}"#));
        log.apply(&ev("session.usage_record", "30", r#"{"usage":{"model":"claude-x","inputTokens":5,"cacheReadTokens":1,"cacheWriteTokens":1}}"#));
        let req = util::parse_iso("2025-11-24T16:00:30Z").unwrap();
        assert_eq!(log.cache_expires(), Some(req + 300.0));
        assert_eq!(log.context_tokens, Some(7));
        assert_eq!(default_ttl("gpt-6.1"), 1800.0);
        assert_eq!(default_ttl("claude-sonnet"), 300.0);
    }

    #[test]
    fn turn_without_a_message_is_not_done() {
        let mut log = CopilotLog::new();
        log.apply(&ev("assistant.turn_start", "01", r#"{"turnId":"0"}"#));
        log.apply(&ev("assistant.turn_end", "02", r#"{"turnId":"0"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Working);
    }

    #[test]
    fn model_switch_has_no_cache_yet() {
        let mut log = CopilotLog::new();
        log.apply(&ev("session.usage_record", "30", r#"{"usage":{"model":"gpt-a","inputTokens":5,"cacheExpiresAt":"2025-11-24T16:30:30Z"}}"#));
        assert!(log.cache_expires().is_some());
        log.apply(&ev("session.model_change", "40", r#"{"newModel":"claude-b"}"#));
        assert_eq!(log.cache_expires(), None);
        // A request of unknown model says nothing about a model selected later.
        let mut anon = CopilotLog::new();
        anon.apply(&ev("session.usage_record", "30", r#"{"usage":{"inputTokens":5,"cacheExpiresAt":"2025-11-24T16:30:30Z"}}"#));
        assert!(anon.cache_expires().is_some());
        anon.apply(&ev("session.model_change", "40", r#"{"newModel":"claude-b"}"#));
        assert_eq!(anon.cache_expires(), None);
    }

    #[test]
    fn forget_open_drops_pairs_from_a_skipped_middle() {
        let mut log = CopilotLog::new();
        log.apply(&ev("permission.requested", "01", r#"{"requestId":"p"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::NeedsYou);
        log.forget_open();
        // Tail: the last turn ends normally.
        log.apply(&ev("assistant.turn_start", "50", r#"{"turnId":"9"}"#));
        log.apply(&ev("assistant.message", "51", r#"{"toolRequests":[]}"#));
        log.apply(&ev("assistant.turn_end", "52", r#"{"turnId":"9"}"#));
        assert_eq!(log.sig.as_ref().unwrap().state, State::Done);
    }

    #[test]
    fn workspace_quoting() {
        let w = Workspace::parse("name: 'It''s: here'\nuser_named: true\nclient_name: github\n");
        assert_eq!(w.name.as_deref(), Some("It's: here"));
        assert!(w.user_named);
        let b = Workspace::parse("name: |-\n  # Review this\n\n  second line\nuser_named: false\nbranch: main\n");
        assert_eq!(b.name.as_deref(), Some("# Review this second line"));
        assert_eq!(b.branch.as_deref(), Some("main"));
    }
}
