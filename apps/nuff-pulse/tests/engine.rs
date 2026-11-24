//! Engine over temporary roots: initial scan, appended lines, hook merge,
//! Copilot liveness by `.hold`, and Pulse File housekeeping.

use nuff_pulse::engine::{Engine, Options};
use nuff_pulse::model::{Display, Origin, Runtime, SessionKey};
use nuff_pulse::procs::{Hold, Probe};
use nuff_pulse::roots::{Root, RootKind};
use nuff_pulse::util::parse_iso;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const VSCODE_ID: &str = "00000000-0000-4000-8000-000000000002";

#[derive(Default, Clone)]
struct World {
    alive: HashMap<u32, bool>,
    holds: HashMap<PathBuf, Hold>,
}

struct FakeProbe(World);

impl Probe for FakeProbe {
    fn alive(&self, pid: u32, _started: Option<f64>) -> Option<bool> {
        Some(*self.0.alive.get(&pid).unwrap_or(&false))
    }
    fn hold(&self, path: &Path) -> Hold {
        *self.0.holds.get(path).unwrap_or(&Hold::Missing)
    }
    fn exe_and_parent(&self, _pid: u32) -> Option<(String, Option<String>)> {
        Some(("copilot.exe".into(), Some("pwsh.exe".into())))
    }
}

struct Setup {
    dir: PathBuf,
    world: Arc<Mutex<World>>,
}

impl Setup {
    fn new(name: &str) -> Setup {
        let dir = std::env::temp_dir().join(format!("nuff-pulse-engine-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Setup { dir, world: Arc::new(Mutex::new(World::default())) }
    }

    fn roots(&self) -> Vec<Root> {
        let r = |kind, sub: &str| Root { kind, path: self.dir.join(sub), origin: Origin::Windows, poll: false };
        vec![r(RootKind::Claude, "claude"), r(RootKind::Copilot, "copilot"), r(RootKind::Pulse, "pulse")]
    }

    fn engine(&self) -> Engine {
        let world = self.world.clone();
        let opts = Options {
            watch: false,
            probe: Box::new(move || Box::new(FakeProbe(world.lock().unwrap().clone()))),
        };
        Engine::new(self.roots(), opts)
    }

    fn copilot_dir(&self, id: &str) -> PathBuf {
        self.dir.join("copilot").join("session-state").join(id)
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn append(p: &Path, s: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
    f.write_all(s.as_bytes()).unwrap();
}

fn copy_copilot_fixture(setup: &Setup, name: &str, id: &str, keep_lines: Option<usize>) -> PathBuf {
    let dir = setup.copilot_dir(id);
    std::fs::create_dir_all(&dir).unwrap();
    let events = std::fs::read_to_string(format!("{FIX}/copilot/{name}/events.jsonl")).unwrap();
    let kept: Vec<&str> = events.lines().take(keep_lines.unwrap_or(usize::MAX)).collect();
    std::fs::write(dir.join("events.jsonl"), kept.join("\n") + "\n").unwrap();
    std::fs::copy(format!("{FIX}/copilot/{name}/workspace.yaml"), dir.join("workspace.yaml")).unwrap();
    dir.join("events.jsonl")
}

fn key(runtime: Runtime, id: &str) -> SessionKey {
    SessionKey { runtime, origin: Origin::Windows, id: id.into() }
}

#[test]
fn copilot_session_follows_appends_and_liveness() {
    let setup = Setup::new("copilot");
    // Up to (excluding) the ask_user tool start of the VS Code fixture.
    let all = std::fs::read_to_string(format!("{FIX}/copilot/vscode/events.jsonl")).unwrap();
    let lines: Vec<&str> = all.lines().collect();
    let ask = lines.iter().position(|l| l.contains("\"toolName\":\"ask_user\"") && l.contains("tool.execution_start")).unwrap();
    let events = copy_copilot_fixture(&setup, "vscode", VSCODE_ID, Some(ask));
    let dir = setup.copilot_dir(VSCODE_ID);
    std::fs::write(dir.join("inuse.4242.lock"), "4242").unwrap();
    std::fs::write(dir.join("inuse.4242.hold"), "").unwrap();
    {
        let mut w = setup.world.lock().unwrap();
        w.alive.insert(4242, true);
        w.holds.insert(dir.join("inuse.4242.hold"), Hold::Held);
    }
    let t_ask = parse_iso("2025-11-24T16:00:51Z").unwrap();
    let mut e = setup.engine();
    e.start(t_ask);
    let k = key(Runtime::Copilot, VSCODE_ID);
    let row = e.model.row(&k, &e.model.sessions[&k], t_ask);
    assert_eq!(row.state, Display::Working);
    assert_eq!(row.label, "Copilot · VS Code");
    assert_eq!(row.title, "Test prompt session");
    assert_eq!(row.workspace, "project");
    assert_eq!(row.focus_pid, Some(4242));

    // The question is asked: Needs you, raised by the next size check.
    append(&events, &format!("{}\n", lines[ask]));
    assert!(e.poll(t_ask + 3.0));
    let row = e.model.row(&k, &e.model.sessions[&k], t_ask + 3.0);
    assert_eq!((row.state, row.alert), (Display::NeedsYou, true));
    assert_eq!(row.reason.as_deref(), Some("question"));
    assert_eq!(row.activity.as_deref(), Some("Asking user"));

    // The rest of the session, minus the shutdown: provisional Done.
    let rest: Vec<&str> = lines[ask + 1..].iter().copied().filter(|l| !l.contains("session.shutdown")).collect();
    append(&events, &(rest.join("\n") + "\n"));
    // The final turn_end is at 16:01:10.532.
    let t_done = parse_iso("2025-11-24T16:01:12Z").unwrap();
    e.poll(t_done);
    let row = e.model.row(&k, &e.model.sessions[&k], t_done);
    assert_eq!((row.state, row.alert), (Display::Done, false), "provisional Done waits 3 s");
    let row = e.model.row(&k, &e.model.sessions[&k], t_done + 2.0);
    assert_eq!((row.state, row.alert), (Display::Done, true));
    assert!(row.cache_level.is_some());

    // A hook agentStop line arrives; then the user clicks the row.
    let pulse = setup.dir.join("pulse/sessions").join(format!("copilot-{VSCODE_ID}.hooks.jsonl"));
    append(&pulse, &format!(r#"{{"v":1,"t":{},"runtime":"copilot","session_id":"{VSCODE_ID}","event":"agentStop","state":"done","reason":"end_turn"}}{}"#, t_done + 1.0, "\n"));
    e.poll(t_done + 61.0); // rescan finds the new Pulse File
    assert!(e.model.sessions[&k].hooks.sig.is_some());
    e.acknowledge(&k, t_done + 62.0);
    let row = e.model.row(&k, &e.model.sessions[&k], t_done + 62.0);
    assert_eq!((row.state, row.alert), (Display::Idle, false));

    // The process exits and removes its lock files: Ended at the next check.
    std::fs::remove_file(dir.join("inuse.4242.hold")).unwrap();
    std::fs::remove_file(dir.join("inuse.4242.lock")).unwrap();
    setup.world.lock().unwrap().alive.insert(4242, false);
    e.poll(t_done + 100.0);
    let row = e.model.row(&k, &e.model.sessions[&k], t_done + 100.0);
    assert_eq!(row.state, Display::Ended);
    assert!(!row.active);
}

#[test]
fn leftover_hold_after_a_hard_kill_is_not_live() {
    let setup = Setup::new("leftover");
    copy_copilot_fixture(&setup, "cli", "00000000-0000-4000-8000-000000000003", Some(20));
    let dir = setup.copilot_dir("00000000-0000-4000-8000-000000000003");
    std::fs::write(dir.join("inuse.77.hold"), "").unwrap();
    setup.world.lock().unwrap().holds.insert(dir.join("inuse.77.hold"), Hold::Free);
    let now = parse_iso("2025-11-24T16:00:30Z").unwrap();
    let mut e = setup.engine();
    e.start(now);
    let k = key(Runtime::Copilot, "00000000-0000-4000-8000-000000000003");
    let row = e.model.row(&k, &e.model.sessions[&k], now);
    assert_eq!(row.state, Display::Ended);
    assert_eq!(row.reason.as_deref(), Some("not_live"));
}

#[test]
fn claude_transcript_hooks_and_status() {
    let setup = Setup::new("claude");
    let id = "11111111-2222-3333-4444-555555555555";
    let transcript = setup.dir.join("claude/projects/C--work-project").join(format!("{id}.jsonl"));
    let fixture = std::fs::read_to_string(format!("{FIX}/claude-transcript-01.jsonl")).unwrap();
    append(&transcript, &fixture);
    let pulse = setup.dir.join("pulse/sessions").join(format!("claude-{id}.hooks.jsonl"));
    let last = fixture.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("timestamp").and_then(|t| t.as_str()).and_then(parse_iso))
        .fold(0.0, f64::max);
    let line = |t: f64, state: &str, reason: &str| {
        format!(r#"{{"v":1,"t":{t},"runtime":"claude","session_id":"{id}","event":"x","state":"{state}","reason":"{reason}","detail":"Allow Bash: cargo test","host":{{"os":"windows","agent_pid":900,"agent_started":1.0}}}}{}"#, "\n")
    };
    append(&pulse, &line(last + 1.0, "needs_you", "permission_prompt"));
    setup.world.lock().unwrap().alive.insert(900, true);

    let mut e = setup.engine();
    e.start(last + 2.0);
    let k = key(Runtime::Claude, id);
    let row = e.model.row(&k, &e.model.sessions[&k], last + 2.0);
    assert_eq!((row.state, row.alert), (Display::NeedsYou, true));
    assert_eq!(row.detail.as_deref(), Some("Allow Bash: cargo test"));
    assert_eq!(row.label, "Claude app");
    assert_eq!(row.focus_pid, Some(900));
    assert_eq!(row.model.as_deref(), Some("claude-opus-5-5"));
    assert!(row.context.is_some());
    assert_eq!(row.resume_command, format!("claude --resume {id}"));

    // The user approves: Claude's status file turns busy.
    let status = setup.dir.join("claude/sessions/900.json");
    append(&status, &format!(r#"{{"pid":900,"sessionId":"{id}","status":"busy","statusUpdatedAt":{}}}"#, ((last + 3.0) * 1000.0) as u64));
    e.poll(last + 63.0);
    let row = e.model.row(&k, &e.model.sessions[&k], last + 63.0);
    assert_eq!(row.state, Display::Working);

    // `/clear`: the same process now runs another session.
    let other = "99999999-2222-3333-4444-555555555555";
    std::fs::write(&status, format!(r#"{{"pid":900,"sessionId":"{other}","status":"idle","statusUpdatedAt":{}}}"#, ((last + 4.0) * 1000.0) as u64)).unwrap();
    e.poll(last + 130.0); // next rescan re-reads the status file
    assert!(e.model.sessions[&k].status.is_none());
    assert_eq!(e.model.sessions[&key(Runtime::Claude, other)].status.as_ref().unwrap().pid, 900);
    let row = e.model.row(&k, &e.model.sessions[&k], last + 130.0);
    assert_eq!((row.state, row.alert, row.active), (Display::Ended, false, false));
    assert_eq!(row.reason.as_deref(), Some("cleared"));

}

#[test]
fn housekeeping_deletes_old_ended_pulse_files_only() {
    let setup = Setup::new("housekeeping");
    let sessions = setup.dir.join("pulse/sessions");
    let mk = |name: &str, state: &str, age_h: u64| {
        let p = sessions.join(name);
        append(&p, &format!(r#"{{"v":1,"t":1,"runtime":"claude","session_id":"a","state":"{state}"}}{}"#, "\n"));
        let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_h * 3600)).unwrap();
        p
    };
    let old_ended = mk("claude-a.hooks.jsonl", "ended", 25);
    let old_done = mk("claude-b.hooks.jsonl", "done", 25);
    let new_ended = mk("claude-c.hooks.jsonl", "ended", 1);
    let ancient = mk("claude-d.hooks.jsonl", "done", 24 * 8);
    let other = mk("notes.txt", "ended", 25 * 8);
    let mut e = setup.engine();
    e.housekeeping(nuff_pulse::util::now());
    assert!(!old_ended.exists());
    assert!(old_done.exists());
    assert!(new_ended.exists());
    assert!(!ancient.exists());
    assert!(other.exists());
}

#[test]
fn claude_liveness_by_agent_pid_or_status_pid() {
    use nuff_pulse::claude::ClaudeStatus;
    use nuff_pulse::engine::claude_liveness;
    use nuff_pulse::model::Liveness;
    use nuff_pulse::pulse::{parse_line, HookState};
    let mut hooks = HookState::default();
    hooks.apply(&parse_line(r#"{"v":1,"t":1,"runtime":"claude","session_id":"a","host":{"os":"windows","agent_pid":900,"agent_started":1.0}}"#).unwrap());
    let status = ClaudeStatus::parse(r#"{"pid":901,"sessionId":"a","status":"busy","statusUpdatedAt":1000}"#).unwrap();
    let probe = |alive: &[(u32, bool)]| FakeProbe(World { alive: alive.iter().copied().collect(), holds: HashMap::new() });
    assert_eq!(claude_liveness(&probe(&[(900, true)]), &hooks, Some(&status)), Liveness::Live);
    assert_eq!(claude_liveness(&probe(&[(901, true)]), &hooks, Some(&status)), Liveness::Live);
    assert_eq!(claude_liveness(&probe(&[]), &hooks, Some(&status)), Liveness::NotLive);
    assert_eq!(claude_liveness(&probe(&[]), &HookState::default(), None), Liveness::Unknown);
    // Linux pids from WSL hook lines are never looked up among Windows processes.
    let mut wsl = HookState::default();
    wsl.apply(&parse_line(r#"{"v":1,"t":1,"runtime":"claude","session_id":"a","host":{"os":"wsl","agent_pid":900}}"#).unwrap());
    assert_eq!(claude_liveness(&probe(&[(900, true)]), &wsl, None), Liveness::Unknown);
}

#[test]
fn roots_change_keeps_offsets_and_ends_stopped_wsl_sessions() {
    let setup = Setup::new("roots");
    let events = copy_copilot_fixture(&setup, "vscode", VSCODE_ID, None);
    // The same folder, seen as a WSL distro's root.
    let wsl_dir = setup.dir.join("wsl-copilot");
    let wsl_session = wsl_dir.join("session-state").join("wsl-1");
    std::fs::create_dir_all(&wsl_session).unwrap();
    std::fs::copy(&events, wsl_session.join("events.jsonl")).unwrap();
    let wsl_root = Root { kind: RootKind::Copilot, path: wsl_dir, origin: Origin::Wsl("Ubuntu".into()), poll: true };

    let t = parse_iso("2025-11-24T16:01:00Z").unwrap();
    let mut e = setup.engine();
    e.start(t);
    let windows = key(Runtime::Copilot, VSCODE_ID);
    let wsl = SessionKey { runtime: Runtime::Copilot, origin: Origin::Wsl("Ubuntu".into()), id: "wsl-1".into() };
    assert!(!e.model.sessions.contains_key(&wsl));

    // The distro starts: its root is added and read.
    let mut with_wsl = setup.roots();
    with_wsl.push(wsl_root);
    e.set_roots(with_wsl.clone(), t);
    assert!(e.model.sessions.contains_key(&wsl));
    let before = e.model.sessions[&windows].copilot.as_ref().unwrap().last_write_t;

    // It stops: its sessions end; the Windows log is not read again.
    e.set_roots(setup.roots(), t + 1.0);
    let row = e.model.row(&wsl, &e.model.sessions[&wsl], t + 1.0);
    assert_eq!(row.state, Display::Ended);
    assert_eq!(e.model.sessions[&windows].copilot.as_ref().unwrap().last_write_t, before);
    // Only the WSL session was marked not live by the change.
    assert_eq!(e.model.sessions[&wsl].live_t, t + 1.0);
    assert_ne!(e.model.sessions[&windows].live_t, t + 1.0);
    assert!(!e.poll(t + 1.5), "nothing new to read");

    // The distro runs again: its session is read again and no longer ended
    // by the earlier stop (the fixture's own shutdown still ends it, so
    // check the flags).
    e.set_roots(with_wsl, t + 2.0);
    let s = &e.model.sessions[&wsl];
    assert!(!s.root_gone);
    assert_eq!(s.live, nuff_pulse::model::Liveness::Unknown);
}
