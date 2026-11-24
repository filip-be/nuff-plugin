//! "Focus window": finds the window that hosts a session and brings it to
//! the front. From the agent process (Claude `agent_pid`, Copilot lock pid)
//! walk up the parents (Toolhelp32; a parent must be created before its
//! child) to the first process with a visible top-level window, skipping
//! shells and console hosts. Only known hosts are focused; any other chain
//! focuses nothing rather than a wrong window.

use crate::procs::ProcInfo;
use std::collections::HashMap;

/// Processes passed over on the walk up: their windows are never the host.
pub const SKIP: &[&str] = &["pwsh.exe", "powershell.exe", "cmd.exe", "bash.exe", "wsl.exe", "conhost.exe", "openconsole.exe"];

/// Hosts that may be focused, and whether their window must have the
/// workspace folder name in its title.
const HOSTS: &[(&str, bool)] = &[
    ("windowsterminal.exe", false),
    ("code.exe", true),
    ("code - insiders.exe", true),
    ("github.exe", false),
    ("claude.exe", false),
];

/// A visible, unowned top-level window.
#[derive(Debug, Clone, PartialEq)]
pub struct Win {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
}

fn host(exe: &str) -> Option<bool> {
    let e = exe.to_ascii_lowercase();
    HOSTS.iter().find(|(h, _)| *h == e).map(|(_, needs_title)| *needs_title)
}

fn title_has(title: &str, folder: &str) -> bool {
    !folder.is_empty() && title.to_lowercase().contains(&folder.to_lowercase())
}

/// The window to focus for a session started by `start_pid`.
/// `created` gives a process's creation time (Unix seconds).
pub fn target(
    procs: &HashMap<u32, ProcInfo>,
    created: &dyn Fn(u32) -> Option<f64>,
    windows: &[Win],
    start_pid: u32,
    start_created: Option<f64>,
    folder: &str,
) -> Option<isize> {
    let mut pid = start_pid;
    let mut child_created = created(pid)?;
    if start_created.is_some_and(|s| (s - child_created).abs() > 1.0) {
        return None; // the agent ended and its pid now belongs to another process
    }
    for _ in 0..32 {
        let p = procs.get(&pid)?;
        let exe = p.exe.to_ascii_lowercase();
        if !SKIP.contains(&exe.as_str()) {
            let own: Vec<&Win> = windows.iter().filter(|w| w.pid == pid).collect();
            if !own.is_empty() {
                let needs_title = host(&exe)?;
                let titled = own.iter().find(|w| title_has(&w.title, folder));
                return match (titled, needs_title) {
                    (Some(w), _) => Some(w.hwnd),
                    (None, false) => Some(own[0].hwnd),
                    (None, true) => None,
                };
            }
        }
        let parent = p.ppid;
        if parent == 0 || parent == pid {
            return None;
        }
        let parent_created = created(parent)?;
        if parent_created > child_created {
            return None; // the parent pid was reused by a newer process
        }
        pid = parent;
        child_created = parent_created;
    }
    None
}

/// WSL sessions: no Windows pid, so the window of a known host whose title
/// contains the workspace folder name.
pub fn target_by_title(procs: &HashMap<u32, ProcInfo>, windows: &[Win], folder: &str) -> Option<isize> {
    windows
        .iter()
        .filter(|w| procs.get(&w.pid).is_some_and(|p| host(&p.exe).is_some()))
        .find(|w| title_has(&w.title, folder))
        .map(|w| w.hwnd)
}

/// Visible, unowned top-level windows with a title.
#[cfg(windows)]
pub fn windows() -> Vec<Win> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };
    unsafe extern "system" fn each(hwnd: HWND, lp: LPARAM) -> i32 {
        let out = &mut *(lp as *mut Vec<Win>);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return 1;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        out.push(Win { hwnd: hwnd as isize, pid, title: String::from_utf16_lossy(&buf[..n.max(0) as usize]) });
        1
    }
    let mut out: Vec<Win> = Vec::new();
    unsafe { EnumWindows(Some(each), &mut out as *mut Vec<Win> as LPARAM) };
    out
}

#[cfg(not(windows))]
pub fn windows() -> Vec<Win> {
    Vec::new()
}

/// Restores and activates a window.
#[cfg(windows)]
pub fn activate(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
    let h = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h) != 0
    }
}

#[cfg(not(windows))]
pub fn activate(_hwnd: isize) -> bool {
    false
}

/// Focuses the window of a session: by process chain when `pid` is known
/// (Windows sessions; `started` is its creation time when known), else by
/// title (WSL). Returns false when nothing was focused.
pub fn focus(pid: Option<u32>, started: Option<f64>, folder: &str) -> bool {
    let procs = crate::procs::snapshot();
    let wins = windows();
    let hwnd = match pid {
        Some(pid) => target(&procs, &crate::procs::process_created, &wins, pid, started, folder),
        None => target_by_title(&procs, &wins, folder),
    };
    hwnd.is_some_and(activate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn procs(list: &[(u32, u32, &str)]) -> HashMap<u32, ProcInfo> {
        list.iter().map(|(pid, ppid, exe)| (*pid, ProcInfo { pid: *pid, ppid: *ppid, exe: exe.to_string() })).collect()
    }

    fn win(hwnd: isize, pid: u32, title: &str) -> Win {
        Win { hwnd, pid, title: title.into() }
    }

    /// Creation time = pid, so parents (lower pids) are older.
    fn by_pid(pid: u32) -> Option<f64> {
        Some(pid as f64)
    }

    #[test]
    fn terminal_skips_the_shell() {
        let p = procs(&[(10, 1, "WindowsTerminal.exe"), (20, 10, "pwsh.exe"), (30, 20, "claude.exe")]);
        let w = [win(1, 20, "PowerShell"), win(2, 10, "Windows PowerShell")];
        assert_eq!(target(&p, &by_pid, &w, 30, None, "shop"), Some(2));
        assert_eq!(target(&p, &by_pid, &w, 30, Some(30.5), "shop"), Some(2));
        // The agent's pid was reused by a process started later.
        assert_eq!(target(&p, &by_pid, &w, 30, Some(12.0), "shop"), None);
    }

    #[test]
    fn vscode_needs_the_workspace_title() {
        let p = procs(&[(10, 1, "Code.exe"), (20, 10, "Code.exe"), (30, 20, "claude.exe")]);
        let w = [win(1, 10, "other - Visual Studio Code"), win(2, 10, "main.rs - shop - Visual Studio Code")];
        assert_eq!(target(&p, &by_pid, &w, 30, None, "Shop"), Some(2));
        assert_eq!(target(&p, &by_pid, &w, 30, None, "nope"), None);
    }

    #[test]
    fn copilot_app_and_unknown_hosts() {
        let p = procs(&[(10, 1, "GitHub.exe"), (20, 10, "copilot-runtime.exe")]);
        assert_eq!(target(&p, &by_pid, &[win(5, 10, "GitHub")], 20, None, ""), Some(5));
        // An unknown visible window stops the walk: focus nothing.
        let p = procs(&[(10, 1, "WindowsTerminal.exe"), (15, 10, "notepad.exe"), (20, 15, "copilot.exe")]);
        let w = [win(1, 15, "Notepad"), win(2, 10, "Terminal")];
        assert_eq!(target(&p, &by_pid, &w, 20, None, ""), None);
    }

    #[test]
    fn reused_parent_pid_stops_the_walk() {
        let p = procs(&[(40, 1, "WindowsTerminal.exe"), (30, 40, "claude.exe")]);
        assert_eq!(target(&p, &by_pid, &[win(1, 40, "t")], 30, None, ""), None);
    }

    #[test]
    fn wsl_by_title() {
        let p = procs(&[(10, 1, "WindowsTerminal.exe"), (11, 1, "notepad.exe")]);
        let w = [win(1, 11, "shop notes"), win(2, 10, "me@box: ~/shop")];
        assert_eq!(target_by_title(&p, &w, "shop"), Some(2));
        assert_eq!(target_by_title(&p, &w, ""), None);
    }
}
