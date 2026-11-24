//! Keeps the model in sync with the files: change notifications as hints,
//! a size check every 2 s, polling for polled roots, liveness every 30 s and
//! housekeeping every hour.

use crate::claude::{ClaudeLog, ClaudeStatus};
use crate::copilot::{CopilotLog, LockProcess, Workspace};
use crate::model::{Liveness, Model, Origin, Runtime, SessionKey, State, ACTIVE_WINDOW};
use crate::procs::{Hold, Probe, SystemProbe};
use crate::pulse::{self, HookState};
use crate::roots::{self, FileRef, Root, RootKind};
use crate::tailer::{Chunk, Tailer};
use crate::util;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

pub const SIZE_CHECK_EVERY: f64 = 2.0;
pub const POLL_EVERY: f64 = 5.0;
pub const LIVENESS_EVERY: f64 = 30.0;
pub const RESCAN_EVERY: f64 = 60.0;
pub const HOUSEKEEPING_EVERY: f64 = 3600.0;
/// Pulse Files of sessions that ended this long ago are deleted.
pub const PULSE_ENDED_TTL: f64 = 24.0 * 3600.0;
/// A whole-read file whose mtime is younger than this can still change
/// without changing its stamp (same size, same coarse mtime tick), so its
/// stamp is not trusted yet and the next check reads it again.
pub const STAMP_SETTLE: f64 = 2.0;
/// Pulse Files untouched this long are deleted whatever they say.
pub const PULSE_STALE_TTL: f64 = 7.0 * 24.0 * 3600.0;

pub struct Options {
    /// Use change notifications (off in tests: size checks and rescans only).
    pub watch: bool,
    /// Creates a process probe for each liveness round.
    pub probe: Box<dyn Fn() -> Box<dyn Probe>>,
}

impl Default for Options {
    fn default() -> Self {
        Options { watch: true, probe: Box::new(|| Box::new(SystemProbe::new())) }
    }
}

struct Tracked {
    root: usize,
    fref: FileRef,
    tailer: Option<Tailer>,
    /// Size and mtime of a whole-read file when last read.
    stamp: Option<(u64, f64)>,
}

pub struct Engine {
    pub model: Model,
    roots: Vec<Root>,
    files: HashMap<PathBuf, Tracked>,
    opts: Options,
    watcher: Option<notify::RecommendedWatcher>,
    watched: HashSet<PathBuf>,
    rx: Option<Receiver<notify::Result<notify::Event>>>,
    next_size_check: f64,
    next_poll: f64,
    next_live: f64,
    next_rescan: f64,
    next_housekeeping: f64,
    live_soon: bool,
    generation: u64,
}

impl Engine {
    pub fn new(roots: Vec<Root>, opts: Options) -> Engine {
        Engine {
            model: Model::default(),
            roots,
            files: HashMap::new(),
            opts,
            watcher: None,
            watched: HashSet::new(),
            rx: None,
            next_size_check: 0.0,
            next_poll: 0.0,
            next_live: 0.0,
            next_rescan: 0.0,
            next_housekeeping: 0.0,
            live_soon: false,
            generation: 0,
        }
    }

    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    /// Bumped whenever the model may have changed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Initial scan: reads the files of sessions active today (head and tail
    /// of large logs) and starts watching.
    pub fn start(&mut self, now: f64) {
        if self.opts.watch {
            let (tx, rx) = channel();
            self.watcher = notify::recommended_watcher(move |res| {
                let _ = tx.send(res);
            })
            .ok();
            self.rx = Some(rx);
        }
        self.rescan(now, None);
        self.check_liveness(now);
        self.housekeeping(now);
        self.next_size_check = now + SIZE_CHECK_EVERY;
        self.next_poll = now + POLL_EVERY;
        self.next_live = now + LIVENESS_EVERY;
        self.next_rescan = now + RESCAN_EVERY;
        self.next_housekeeping = now + HOUSEKEEPING_EVERY;
    }

    /// Processes pending hints and due periodic checks. Returns true when the
    /// model may have changed.
    pub fn poll(&mut self, now: f64) -> bool {
        let before = self.generation;
        let mut hinted: Vec<PathBuf> = Vec::new();
        let mut rescan = false;
        if let Some(rx) = &self.rx {
            while let Ok(res) = rx.try_recv() {
                match res {
                    Ok(ev) => {
                        if ev.need_rescan() {
                            rescan = true;
                        }
                        hinted.extend(ev.paths);
                    }
                    Err(_) => rescan = true,
                }
            }
        }
        hinted.sort();
        hinted.dedup();
        for p in hinted {
            self.hint(&p, now);
        }
        if rescan || now >= self.next_rescan {
            self.rescan(now, None);
            self.next_rescan = now + RESCAN_EVERY;
        }
        if now >= self.next_poll {
            let polled: Vec<usize> = (0..self.roots.len()).filter(|&i| self.roots[i].poll).collect();
            for i in polled {
                self.rescan(now, Some(i));
            }
            self.next_poll = now + POLL_EVERY;
        }
        if now >= self.next_size_check {
            self.size_check(now);
            self.next_size_check = now + SIZE_CHECK_EVERY;
        }
        if self.live_soon || now >= self.next_live {
            self.check_liveness(now);
            self.live_soon = false;
            self.next_live = now + LIVENESS_EVERY;
        }
        if now >= self.next_housekeeping {
            self.housekeeping(now);
            self.next_housekeeping = now + HOUSEKEEPING_EVERY;
        }
        self.revive(now);
        self.generation != before
    }

    /// Replaces the roots (settings changed, a WSL distro started or
    /// stopped). Files of roots that stay keep their read offsets. Sessions
    /// no longer read from any root end; they come back when their files are
    /// read again (the distro runs again, the root is added back).
    pub fn set_roots(&mut self, roots: Vec<Root>, now: f64) {
        if roots == self.roots {
            return;
        }
        let map: Vec<Option<usize>> = self.roots.iter().map(|r| roots.iter().position(|n| n == r)).collect();
        let dropped: HashSet<SessionKey> = self
            .files
            .values()
            .filter(|t| map[t.root].is_none())
            .filter_map(|t| file_key(&t.fref, &self.roots[t.root].origin))
            .collect();
        self.files.retain(|_, t| map[t.root].is_some());
        for t in self.files.values_mut() {
            t.root = map[t.root].unwrap_or(t.root);
        }
        let keep: HashSet<PathBuf> =
            roots.iter().filter(|r| !r.poll).flat_map(|r| roots::watch_dirs(r).into_iter().map(|(d, _)| d)).collect();
        if let Some(w) = self.watcher.as_mut() {
            use notify::Watcher;
            for d in self.watched.iter().filter(|d| !keep.contains(*d)) {
                let _ = w.unwatch(d);
            }
        }
        self.watched.retain(|d| keep.contains(d));
        self.roots = roots;
        let read = self.read_keys();
        for key in dropped.iter().filter(|k| !read.contains(*k)) {
            if let Some(s) = self.model.sessions.get_mut(key) {
                s.root_gone = true;
                s.live = Liveness::NotLive;
                s.live_t = now;
                s.note_state();
            }
        }
        self.rescan(now, None);
        self.revive(now);
        self.generation += 1;
    }

    /// Sessions marked `root_gone` whose files are read again (now, or on a
    /// later poll once they show up) are no longer ended by that mark.
    fn revive(&mut self, now: f64) {
        if !self.model.sessions.values().any(|s| s.root_gone) {
            return;
        }
        let read = self.read_keys();
        for (key, s) in self.model.sessions.iter_mut() {
            if s.root_gone && read.contains(key) {
                s.root_gone = false;
                s.live = Liveness::Unknown;
                s.live_t = now;
                s.note_state();
                self.live_soon = true;
                self.generation += 1;
            }
        }
    }

    /// Sessions that have at least one tracked file.
    fn read_keys(&self) -> HashSet<SessionKey> {
        self.files.values().filter_map(|t| file_key(&t.fref, &self.roots[t.root].origin)).collect()
    }

    /// Replaces the context window overrides.
    pub fn set_config(&mut self, config: crate::model::Config) {
        self.model.config = config;
        self.generation += 1;
    }

    /// Clears a session's alert (row clicked).
    pub fn acknowledge(&mut self, key: &SessionKey, now: f64) {
        self.model.acknowledge(key, now);
        self.generation += 1;
    }

    /// Hides a session from the bar until its next activity.
    pub fn hide(&mut self, key: &SessionKey, now: f64) {
        self.model.hide(key, now);
        self.generation += 1;
    }

    /// Shows a hidden session on the bar again.
    pub fn unhide(&mut self, key: &SessionKey) {
        self.model.unhide(key);
        self.generation += 1;
    }

    fn keep_since(now: f64) -> f64 {
        util::start_of_today(now).min(now - ACTIVE_WINDOW)
    }

    fn ensure_watches(&mut self) {
        let Some(w) = self.watcher.as_mut() else { return };
        use notify::{RecursiveMode, Watcher};
        for root in self.roots.iter().filter(|r| !r.poll) {
            for (dir, recursive) in roots::watch_dirs(root) {
                if self.watched.contains(&dir) || !dir.is_dir() {
                    continue;
                }
                let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
                if w.watch(&dir, mode).is_ok() {
                    self.watched.insert(dir);
                }
            }
        }
    }

    /// Lists the roots (or one root) and reads new or changed files.
    fn rescan(&mut self, now: f64, only: Option<usize>) {
        self.ensure_watches();
        let keep_since = Self::keep_since(now);
        let indices: Vec<usize> = match only {
            Some(i) => vec![i],
            None => (0..self.roots.len()).collect(),
        };
        for i in indices {
            for (path, fref, mtime) in roots::list(&self.roots[i]) {
                let tracked = self.files.contains_key(&path);
                // Status files describe running processes: always read.
                let recent = mtime >= keep_since || matches!(fref, FileRef::ClaudeStatus(_));
                if tracked || recent {
                    self.process(i, &path, fref, now);
                }
            }
        }
    }

    fn hint(&mut self, path: &Path, now: f64) {
        if let Some(t) = self.files.get(path) {
            let (i, fref) = (t.root, t.fref.clone());
            self.process(i, path, fref, now);
            return;
        }
        for i in 0..self.roots.len() {
            if let Some(fref) = roots::classify(&self.roots[i], path) {
                if path.exists() {
                    self.process(i, path, fref, now);
                }
                return;
            }
        }
    }

    fn size_check(&mut self, now: f64) {
        let due: Vec<(PathBuf, usize, FileRef)> = self
            .files
            .iter()
            // Polled roots (WSL) are read on their own 5 s round.
            .filter(|(_, t)| t.tailer.is_some() && !self.roots[t.root].poll)
            .filter(|(p, t)| {
                let len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                len != t.tailer.as_ref().map_or(0, Tailer::offset)
            })
            .map(|(p, t)| (p.clone(), t.root, t.fref.clone()))
            .collect();
        for (p, i, fref) in due {
            self.process(i, &p, fref, now);
        }
    }

    fn key(&self, root: usize, runtime: Runtime, id: &str) -> SessionKey {
        SessionKey { runtime, origin: self.roots[root].origin.clone(), id: id.to_string() }
    }

    /// Reads a file and applies it to its session.
    fn process(&mut self, root: usize, path: &Path, fref: FileRef, now: f64) {
        let changed = match &fref {
            FileRef::ClaudeTranscript(id) => {
                let key = self.key(root, Runtime::Claude, id);
                self.tail(root, path, &fref).is_some_and(|(chunk, mtime)| {
                    let s = self.model.session(key);
                    if chunk.reset {
                        s.claude = None;
                    }
                    let log = s.claude.get_or_insert_with(ClaudeLog::new);
                    chunk.head.iter().for_each(|l| log.apply_line(l));
                    if chunk.skipped {
                        log.forget_open();
                    }
                    chunk.lines.iter().for_each(|l| log.apply_line(l));
                    s.log_mtime = Some(mtime);
                    s.note_state();
                    true
                })
            }
            FileRef::CopilotEvents(id) => {
                let first = !self.files.contains_key(path);
                let key = self.key(root, Runtime::Copilot, id);
                let changed = self.tail(root, path, &fref).is_some_and(|(chunk, mtime)| {
                    let s = self.model.session(key);
                    if chunk.reset {
                        s.copilot = None;
                    }
                    let log = s.copilot.get_or_insert_with(CopilotLog::new);
                    chunk.head.iter().for_each(|l| log.apply_line(l));
                    if chunk.skipped {
                        log.forget_open();
                    }
                    chunk.lines.iter().for_each(|l| log.apply_line(l));
                    s.log_mtime = Some(mtime);
                    s.note_state();
                    true
                });
                if first {
                    let ws = roots::copilot_dir(&self.roots[root], id).join("workspace.yaml");
                    if ws.exists() {
                        self.process(root, &ws, FileRef::CopilotWorkspace(id.clone()), now);
                    }
                    self.live_soon = true;
                }
                changed
            }
            FileRef::Pulse(rt, id) => {
                let key = self.key(root, *rt, id);
                self.tail(root, path, &fref).is_some_and(|(chunk, _)| {
                    let s = self.model.session(key);
                    let mut any = false;
                    // Hook lines merge newest-wins, so head + tail is safe.
                    let lines = chunk.head.iter().chain(chunk.lines.iter());
                    for line in lines.filter_map(|l| pulse::parse_line(l)) {
                        if line.runtime == *rt && line.session_id == *id {
                            s.hooks.apply(&line);
                            any = true;
                        }
                    }
                    s.note_state();
                    any
                })
            }
            FileRef::CopilotWorkspace(id) => {
                let key = self.key(root, Runtime::Copilot, id);
                match self.read_whole(root, path, &fref) {
                    Some(text) => {
                        self.model.session(key).workspace = Some(Workspace::parse(&text));
                        true
                    }
                    None => false,
                }
            }
            FileRef::ClaudeStatus(_) => match self.read_whole(root, path, &fref).and_then(|t| ClaudeStatus::parse(&t)) {
                Some(st) => {
                    let key = self.key(root, Runtime::Claude, &st.session_id);
                    // The same process now runs another session (`/clear`):
                    // its status no longer describes the old one.
                    for (k, other) in self.model.sessions.iter_mut() {
                        if *k != key && k.origin == key.origin && other.status.as_ref().is_some_and(|o| o.pid == st.pid) {
                            other.status = None;
                            other.cleared_t = Some(st.status_t.unwrap_or(now));
                            other.note_state();
                        }
                    }
                    let s = self.model.session(key);
                    if s.status.as_ref() != Some(&st) {
                        s.status = Some(st);
                        s.note_state();
                        true
                    } else {
                        false
                    }
                }
                None => false,
            },
            FileRef::CopilotLock(_) => {
                self.live_soon = true;
                false
            }
        };
        if changed {
            self.generation += 1;
        }
    }

    /// New complete lines of a log file and its modification time, `None`
    /// when nothing new.
    fn tail(&mut self, root: usize, path: &Path, fref: &FileRef) -> Option<(Chunk, f64)> {
        let t = self.files.entry(path.to_path_buf()).or_insert_with(|| Tracked {
            root,
            fref: fref.clone(),
            tailer: Some(Tailer::with_anchors(path, anchors(fref))),
            stamp: None,
        });
        let chunk = t.tailer.as_mut()?.read().ok()?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).map(util::unix).unwrap_or(0.0);
        (chunk.reset || !chunk.head.is_empty() || !chunk.lines.is_empty()).then_some((chunk, mtime))
    }

    /// The text of a small file when it changed since the last read.
    fn read_whole(&mut self, root: usize, path: &Path, fref: &FileRef) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().map(util::unix).unwrap_or(0.0);
        let stamp = (meta.len(), mtime);
        let t = self.files.entry(path.to_path_buf()).or_insert_with(|| Tracked {
            root,
            fref: fref.clone(),
            tailer: None,
            stamp: None,
        });
        if t.stamp == Some(stamp) || meta.len() > 1024 * 1024 {
            return None;
        }
        let text = std::fs::read_to_string(path).ok()?;
        let settled = util::unix(std::time::SystemTime::now()) - mtime >= STAMP_SETTLE;
        t.stamp = settled.then_some(stamp);
        Some(text)
    }

    /// Windows sessions: Claude by agent pid (+ creation time) or status-file
    /// pid (+ procStart); Copilot by its `inuse.<pid>.hold`. Others stay
    /// unknown and fall back to the 2 h activity window.
    pub fn check_liveness(&mut self, now: f64) {
        let probe = (self.opts.probe)();
        let copilot_roots: Vec<Root> =
            self.roots.iter().filter(|r| r.kind == RootKind::Copilot && r.origin == Origin::Windows).cloned().collect();
        let mut changed = false;
        for (key, s) in self.model.sessions.iter_mut() {
            if key.origin != Origin::Windows || s.root_gone || now - s.last_activity() > ACTIVE_WINDOW {
                continue;
            }
            let (live, lock_pid, lock) = match key.runtime {
                Runtime::Claude => (claude_liveness(probe.as_ref(), &s.hooks, s.status.as_ref()), None, None),
                Runtime::Copilot => {
                    let dir = copilot_roots.iter().map(|r| roots::copilot_dir(r, &key.id)).find(|d| d.is_dir());
                    match dir {
                        Some(d) => copilot_liveness(probe.as_ref(), &d),
                        None => (Liveness::Unknown, None, None),
                    }
                }
            };
            if s.live != live || s.lock_pid != lock_pid {
                if s.lock_pid != lock_pid {
                    // Lets Focus refuse the pid once another process reuses it.
                    s.lock_started = lock_pid.and_then(crate::procs::process_created);
                }
                s.live = live;
                s.live_t = now;
                s.lock_pid = lock_pid;
                s.lock = lock;
                s.note_state();
                changed = true;
            }
        }
        if changed {
            self.generation += 1;
        }
    }

    /// Deletes Pulse Files of sessions ended more than 24 h ago (or untouched
    /// for 7 days) and forgets sessions from before today.
    pub fn housekeeping(&mut self, now: f64) {
        for root in self.roots.iter().filter(|r| r.kind == RootKind::Pulse) {
            let dir = root.path.join("sessions");
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if pulse::parse_file_name(&entry.file_name().to_string_lossy()).is_none() {
                    continue;
                }
                let mtime = entry.metadata().and_then(|m| m.modified()).map(util::unix).unwrap_or(now);
                let age = now - mtime;
                let expired = age > PULSE_STALE_TTL || (age > PULSE_ENDED_TTL && pulse_file_ended(&path));
                if expired && std::fs::remove_file(&path).is_ok() {
                    self.files.remove(&path);
                }
            }
        }
        let keep_since = Self::keep_since(now);
        let before = self.model.sessions.len();
        self.model.prune(now, keep_since);
        if self.model.sessions.len() != before {
            let alive: HashSet<(Runtime, Origin, String)> =
                self.model.sessions.keys().map(|k| (k.runtime, k.origin.clone(), k.id.clone())).collect();
            let roots = &self.roots;
            self.files.retain(|_, t| {
                let origin = roots[t.root].origin.clone();
                match &t.fref {
                    FileRef::ClaudeTranscript(id) => alive.contains(&(Runtime::Claude, origin, id.clone())),
                    FileRef::CopilotEvents(id) | FileRef::CopilotWorkspace(id) | FileRef::CopilotLock(id) => {
                        alive.contains(&(Runtime::Copilot, origin, id.clone()))
                    }
                    FileRef::Pulse(rt, id) => alive.contains(&(*rt, origin, id.clone())),
                    FileRef::ClaudeStatus(_) => true,
                }
            });
            self.generation += 1;
        }
    }
}

/// The session a file belongs to (status files name theirs inside).
fn file_key(fref: &FileRef, origin: &Origin) -> Option<SessionKey> {
    let (runtime, id) = match fref {
        FileRef::ClaudeTranscript(id) => (Runtime::Claude, id),
        FileRef::CopilotEvents(id) | FileRef::CopilotWorkspace(id) | FileRef::CopilotLock(id) => (Runtime::Copilot, id),
        FileRef::Pulse(rt, id) => (*rt, id),
        FileRef::ClaudeStatus(_) => return None,
    };
    Some(SessionKey { runtime, origin: origin.clone(), id: id.clone() })
}

/// Records that carry a log's current state; the first read of a large log
/// reaches back until it finds one (a few huge trailing records must not
/// hide the last turn).
fn anchors(fref: &FileRef) -> &'static [&'static str] {
    match fref {
        FileRef::ClaudeTranscript(_) => &["\"type\":\"assistant\""],
        FileRef::CopilotEvents(_) => {
            &["\"type\":\"session.usage_record\"", "\"type\":\"session.shutdown\"", "\"type\":\"abort\""]
        }
        _ => &[],
    }
}

pub fn claude_liveness(probe: &dyn Probe, hooks: &HookState, status: Option<&ClaudeStatus>) -> Liveness {
    let mut candidates: Vec<(u32, Option<f64>)> = Vec::new();
    if let Some(h) = hooks.host.as_ref().filter(|h| h.os.as_deref() == Some("windows")) {
        if let Some(pid) = h.agent_pid {
            candidates.push((pid, h.agent_started));
        }
    }
    if let Some(st) = status {
        candidates.push((st.pid, st.proc_start));
    }
    let answers: Vec<Option<bool>> = candidates.iter().map(|(p, s)| probe.alive(*p, *s)).collect();
    if answers.contains(&Some(true)) {
        Liveness::Live
    } else if !answers.is_empty() && answers.iter().all(|a| *a == Some(false)) {
        Liveness::NotLive
    } else {
        Liveness::Unknown
    }
}

/// Live when an `inuse.<pid>.hold` is held and its pid runs; a missing
/// `.hold` means ended; an openable leftover means not live.
pub fn copilot_liveness(probe: &dyn Probe, dir: &Path) -> (Liveness, Option<u32>, Option<LockProcess>) {
    let mut any_unknown = false;
    let mut any_hold = false;
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(pid) = name.ends_with(".hold").then(|| roots::lock_pid(&name)).flatten() else { continue };
        any_hold = true;
        match probe.hold(&entry.path()) {
            Hold::Held => match probe.alive(pid, None) {
                Some(true) => {
                    let lock = probe
                        .exe_and_parent(pid)
                        .map(|(exe, parent)| LockProcess { exe: Some(exe), parent_exe: parent });
                    return (Liveness::Live, Some(pid), lock);
                }
                Some(false) => {}
                None => any_unknown = true,
            },
            Hold::Free | Hold::Missing => {}
            Hold::Unknown => any_unknown = true,
        }
    }
    if any_unknown || (!any_hold && !dir.is_dir()) {
        (Liveness::Unknown, None, None)
    } else {
        (Liveness::NotLive, None, None)
    }
}

/// True when the last line of a Pulse File says `ended`.
fn pulse_file_ended(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = crate::tailer::open_shared(path) else { return false };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(8192);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return false;
    }
    let text = String::from_utf8_lossy(&buf);
    text.lines()
        .rev()
        .find_map(pulse::parse_line)
        .is_some_and(|l| l.state == Some(State::Ended))
}
