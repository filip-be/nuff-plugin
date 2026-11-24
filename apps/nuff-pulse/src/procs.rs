//! Process facts for liveness and labels: Toolhelp32 and GetProcessTimes on
//! Windows, never WMI. On other systems every query answers "unknown".

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub exe: String,
}

/// Whether the owner of a Copilot `inuse.<pid>.hold` still holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Exclusive open failed with a sharing violation: someone holds it.
    Held,
    /// Exclusive open succeeded: a leftover nobody holds.
    Free,
    /// The file does not exist.
    Missing,
    /// Any other error.
    Unknown,
}

/// Queries the liveness checks need; a trait so tests can fake processes.
pub trait Probe {
    /// `Some(true)` when `pid` runs and, if given, was created at `started`
    /// (Unix seconds, ±1 s); `Some(false)` when not; `None` when unknown.
    fn alive(&self, pid: u32, started: Option<f64>) -> Option<bool>;
    fn hold(&self, path: &Path) -> Hold;
    /// Executable name of `pid` and of its parent.
    fn exe_and_parent(&self, pid: u32) -> Option<(String, Option<String>)>;
}

pub struct SystemProbe {
    procs: HashMap<u32, ProcInfo>,
}

impl SystemProbe {
    /// Takes one process snapshot; build a new probe for each check round.
    pub fn new() -> Self {
        SystemProbe { procs: snapshot() }
    }
}

impl Default for SystemProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe for SystemProbe {
    fn alive(&self, pid: u32, started: Option<f64>) -> Option<bool> {
        if cfg!(not(windows)) {
            return None;
        }
        let Some(created) = process_created(pid) else {
            return Some(false);
        };
        Some(started.is_none_or(|s| (s - created).abs() <= 1.0))
    }

    fn hold(&self, path: &Path) -> Hold {
        hold_state(path)
    }

    fn exe_and_parent(&self, pid: u32) -> Option<(String, Option<String>)> {
        let p = self.procs.get(&pid)?;
        let parent = self.procs.get(&p.ppid).map(|x| x.exe.clone());
        Some((p.exe.clone(), parent))
    }
}

pub fn hold_state(path: &Path) -> Hold {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0);
    }
    #[cfg(not(windows))]
    {
        return if path.exists() {
            Hold::Unknown
        } else {
            Hold::Missing
        };
    }
    #[allow(unreachable_code)]
    match o.open(path) {
        Ok(_) => Hold::Free,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Hold::Missing,
        Err(e) if e.raw_os_error() == Some(32) => Hold::Held, // ERROR_SHARING_VIOLATION
        Err(_) => Hold::Unknown,
    }
}

#[cfg(windows)]
pub fn snapshot() -> HashMap<u32, ProcInfo> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    let mut out = HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(e.szExeFile.len());
            let exe = String::from_utf16_lossy(&e.szExeFile[..len]);
            out.insert(
                e.th32ProcessID,
                ProcInfo {
                    pid: e.th32ProcessID,
                    ppid: e.th32ParentProcessID,
                    exe,
                },
            );
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
pub fn snapshot() -> HashMap<u32, ProcInfo> {
    HashMap::new()
}

/// Creation time (Unix seconds) of a running process; `None` when it does
/// not run or cannot be opened.
#[cfg(windows)]
pub fn process_created(pid: u32) -> Option<f64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid == 0 {
        return None;
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut code = 0u32;
        let running = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE as u32;
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0;
        CloseHandle(h);
        if !running || !ok {
            return None;
        }
        let ticks = ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64;
        Some(crate::util::filetime_to_unix(ticks))
    }
}

#[cfg(not(windows))]
pub fn process_created(_pid: u32) -> Option<f64> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive_with_matching_start() {
        let pid = std::process::id();
        let created = process_created(pid).unwrap();
        let p = SystemProbe::new();
        assert_eq!(p.alive(pid, Some(created)), Some(true));
        assert_eq!(p.alive(pid, Some(created - 100.0)), Some(false));
        assert!(
            p.exe_and_parent(pid)
                .unwrap()
                .0
                .to_ascii_lowercase()
                .ends_with(".exe")
        );
    }

    #[test]
    fn hold_states() {
        let dir = std::env::temp_dir().join(format!("nuff-pulse-hold-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("inuse.1.hold");
        assert_eq!(hold_state(&path), Hold::Missing);
        std::fs::write(&path, b"").unwrap();
        assert_eq!(hold_state(&path), Hold::Free);
        let held = std::fs::File::open(&path).unwrap();
        assert_eq!(hold_state(&path), Hold::Held);
        drop(held);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
