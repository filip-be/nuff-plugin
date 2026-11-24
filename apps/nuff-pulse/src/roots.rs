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
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        if let Some(fr) = classify(root, &p) {
            let mtime = std::fs::metadata(&p).and_then(|m| m.modified()).map(crate::util::unix).unwrap_or(0.0);
            out.push((p, fr, mtime));
        }
    };
    let dir = |p: &Path| std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path());
    match root.kind {
        RootKind::Claude => {
            for project in dir(&root.path.join("projects")) {
                if project.is_dir() {
                    dir(&project).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).for_each(&mut push);
                }
            }
            dir(&root.path.join("sessions")).for_each(&mut push);
        }
        RootKind::Copilot => {
            for session in dir(&root.path.join("session-state")) {
                if session.is_dir() {
                    for name in ["events.jsonl", "workspace.yaml"] {
                        let p = session.join(name);
                        if p.exists() {
                            push(p);
                        }
                    }
                }
            }
        }
        RootKind::Pulse => dir(&root.path.join("sessions")).for_each(&mut push),
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
}
