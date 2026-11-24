//! Where session files live and what each file is.

use crate::model::{Origin, Runtime};
use crate::pulse;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    /// `~/.claude`: `projects/<enc-cwd>/<id>.jsonl`, `sessions/<pid>.json`.
    Claude,
    /// `~/.copilot`: `session-state/<id>/{events.jsonl,workspace.yaml,inuse.<pid>.hold}`.
    Copilot,
    /// `~/.nuff-pulse`: `sessions/<runtime>-<id>.hooks.jsonl`.
    Pulse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub kind: RootKind,
    pub path: PathBuf,
    pub origin: Origin,
    /// Polled instead of watched (roots across the WSL boundary).
    pub poll: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileRef {
    ClaudeTranscript(String),
    ClaudeStatus(u32),
    CopilotEvents(String),
    CopilotWorkspace(String),
    /// `inuse.<pid>.lock` or `inuse.<pid>.hold` of a session.
    CopilotLock(String),
    Pulse(Runtime, String),
}

impl FileRef {
    pub fn is_log(&self) -> bool {
        matches!(self, FileRef::ClaudeTranscript(_) | FileRef::CopilotEvents(_) | FileRef::Pulse(..))
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The Windows roots: `CLAUDE_CONFIG_DIR` or `~/.claude`, `COPILOT_HOME` or
/// `~/.copilot`, and `~/.nuff-pulse`.
pub fn default_roots() -> Vec<Root> {
    let mut roots = Vec::new();
    let home = home();
    let mut add = |kind, path: Option<PathBuf>| {
        if let Some(path) = path {
            roots.push(Root { kind, path, origin: Origin::Windows, poll: false });
        }
    };
    add(RootKind::Claude, env_dir("CLAUDE_CONFIG_DIR").or(home.as_ref().map(|h| h.join(".claude"))));
    add(RootKind::Copilot, env_dir("COPILOT_HOME").or(home.as_ref().map(|h| h.join(".copilot"))));
    add(RootKind::Pulse, home.as_ref().map(|h| h.join(".nuff-pulse")));
    roots
}

/// Directories to watch for a root, with recursion.
pub fn watch_dirs(root: &Root) -> Vec<(PathBuf, bool)> {
    match root.kind {
        RootKind::Claude => vec![(root.path.join("projects"), true), (root.path.join("sessions"), false)],
        RootKind::Copilot => vec![(root.path.join("session-state"), true)],
        RootKind::Pulse => vec![(root.path.join("sessions"), false)],
    }
}

fn is_session_id(s: &str) -> bool {
    pulse::valid_id(s)
}

/// What `path` is inside `root`, if it is a file nuff-pulse reads.
pub fn classify(root: &Root, path: &Path) -> Option<FileRef> {
    let rel = path.strip_prefix(&root.path).ok()?;
    let parts: Vec<&str> = rel.iter().map(|c| c.to_str()).collect::<Option<_>>()?;
    match (root.kind, parts.as_slice()) {
        (RootKind::Claude, ["projects", _, file]) => {
            let id = file.strip_suffix(".jsonl")?;
            is_session_id(id).then(|| FileRef::ClaudeTranscript(id.to_string()))
        }
        (RootKind::Claude, ["sessions", file]) => {
            let pid = file.strip_suffix(".json")?.parse().ok()?;
            Some(FileRef::ClaudeStatus(pid))
        }
        (RootKind::Copilot, ["session-state", id, file]) if is_session_id(id) => {
            let id = id.to_string();
            match *file {
                "events.jsonl" => Some(FileRef::CopilotEvents(id)),
                "workspace.yaml" => Some(FileRef::CopilotWorkspace(id)),
                f if lock_pid(f).is_some() => Some(FileRef::CopilotLock(id)),
                _ => None,
            }
        }
        (RootKind::Pulse, ["sessions", file]) => {
            let (rt, id) = pulse::parse_file_name(file)?;
            Some(FileRef::Pulse(rt, id))
        }
        _ => None,
    }
}

/// `inuse.1234.hold` / `inuse.1234.lock` → 1234.
pub fn lock_pid(file: &str) -> Option<u32> {
    let rest = file.strip_prefix("inuse.")?;
    let num = rest.strip_suffix(".hold").or_else(|| rest.strip_suffix(".lock"))?;
    num.parse().ok()
}

/// A Copilot session's folder.
pub fn copilot_dir(root: &Root, id: &str) -> PathBuf {
    root.path.join("session-state").join(id)
}

/// Every readable file of a root with its modification time (Unix seconds).
pub fn list(root: &Root) -> Vec<(PathBuf, FileRef, f64)> {
    // Across the WSL boundary every file system call is a round trip, so a
    // polled root takes types and times from the directory listing itself.
    // Windows roots ask each file: NTFS updates a listing's times lazily.
    let poll = root.poll;
    let mtime = |e: &std::fs::DirEntry| {
        // A listing's metadata describes a link itself, not its target.
        let link = e.file_type().is_ok_and(|t| t.is_symlink());
        let m = if poll && !link { e.metadata() } else { std::fs::metadata(e.path()) };
        m.and_then(|m| m.modified()).map(crate::util::unix).unwrap_or(0.0)
    };
    let entries = |p: &Path| std::fs::read_dir(p).into_iter().flatten().flatten();
    let is_dir = |e: &std::fs::DirEntry| match e.file_type() {
        Ok(t) if t.is_symlink() => e.path().is_dir(),
        Ok(t) => t.is_dir(),
        Err(_) => false,
    };
    let mut out = Vec::new();
    let mut push = |e: &std::fs::DirEntry| {
        let p = e.path();
        if let Some(fr) = classify(root, &p) {
            let t = mtime(e);
            out.push((p, fr, t));
        }
    };
    match root.kind {
        RootKind::Claude => {
            for project in entries(&root.path.join("projects")).filter(is_dir) {
                entries(&project.path()).filter(|e| e.path().extension().is_some_and(|x| x == "jsonl")).for_each(|e| push(&e));
            }
            entries(&root.path.join("sessions")).for_each(|e| push(&e));
        }
        RootKind::Copilot => {
            for session in entries(&root.path.join("session-state")).filter(is_dir) {
                entries(&session.path())
                    .filter(|e| e.file_name() == "events.jsonl" || e.file_name() == "workspace.yaml")
                    .for_each(|e| push(&e));
            }
        }
        RootKind::Pulse => entries(&root.path.join("sessions")).for_each(|e| push(&e)),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(kind: RootKind) -> Root {
        Root { kind, path: PathBuf::from("/r"), origin: Origin::Windows, poll: false }
    }

    #[test]
    fn classifies_files() {
        let c = root(RootKind::Claude);
        assert_eq!(
            classify(&c, Path::new("/r/projects/D--x/0b1c-22.jsonl")),
            Some(FileRef::ClaudeTranscript("0b1c-22".into()))
        );
        assert_eq!(classify(&c, Path::new("/r/projects/D--x/abc/subagents/agent-1.jsonl")), None);
        assert_eq!(classify(&c, Path::new("/r/sessions/16624.json")), Some(FileRef::ClaudeStatus(16624)));
        assert_eq!(classify(&c, Path::new("/r/sessions/16624.abc.key")), None);
        let k = root(RootKind::Copilot);
        assert_eq!(classify(&k, Path::new("/r/session-state/ab-1/events.jsonl")), Some(FileRef::CopilotEvents("ab-1".into())));
        assert_eq!(
            classify(&k, Path::new("/r/session-state/ab-1/workspace.yaml")),
            Some(FileRef::CopilotWorkspace("ab-1".into()))
        );
        assert_eq!(classify(&k, Path::new("/r/session-state/ab-1/inuse.49592.hold")), Some(FileRef::CopilotLock("ab-1".into())));
        assert_eq!(classify(&k, Path::new("/r/session-state/ab-1/session.db")), None);
        let p = root(RootKind::Pulse);
        assert_eq!(
            classify(&p, Path::new("/r/sessions/copilot-ab-1.hooks.jsonl")),
            Some(FileRef::Pulse(Runtime::Copilot, "ab-1".into()))
        );
        assert_eq!(lock_pid("inuse.12.lock"), Some(12));
        assert_eq!(lock_pid("inuse.x.lock"), None);
    }

    #[test]
    fn lists_files_watched_and_polled() {
        let base = std::env::temp_dir().join(format!("nuff-pulse-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mk = |rel: &str| {
            let p = base.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "{}").unwrap();
        };
        mk("c/projects/D--x/0b1c-22.jsonl");
        mk("c/projects/D--x/notes.txt");
        mk("c/sessions/16624.json");
        mk("k/session-state/ab-1/events.jsonl");
        mk("k/session-state/ab-1/workspace.yaml");
        mk("k/session-state/ab-1/session.db");
        mk("p/sessions/copilot-ab-1.hooks.jsonl");
        for poll in [false, true] {
            let r = |kind, dir: &str| Root { kind, path: base.join(dir), origin: Origin::Windows, poll };
            let names = |root: Root| {
                let mut v: Vec<FileRef> = list(&root)
                    .into_iter()
                    .inspect(|(_, _, t)| assert!(*t > 0.0))
                    .map(|(_, f, _)| f)
                    .collect();
                v.sort_by_key(|f| format!("{f:?}"));
                v
            };
            assert_eq!(
                names(r(RootKind::Claude, "c")),
                vec![FileRef::ClaudeStatus(16624), FileRef::ClaudeTranscript("0b1c-22".into())]
            );
            assert_eq!(
                names(r(RootKind::Copilot, "k")),
                vec![FileRef::CopilotEvents("ab-1".into()), FileRef::CopilotWorkspace("ab-1".into())]
            );
            assert_eq!(names(r(RootKind::Pulse, "p")), vec![FileRef::Pulse(Runtime::Copilot, "ab-1".into())]);
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
