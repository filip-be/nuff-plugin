//! WSL roots: each running distro's `~/.claude`, `~/.copilot` and
//! `~/.nuff-pulse`, reached through `\\wsl.localhost\<distro>\…` and polled.
//! Only running distros are touched, so the widget never starts one.

use crate::model::Origin;
use crate::roots::{Root, RootKind};
use std::path::{Path, PathBuf};

/// How often the list of running distros is refreshed.
pub const REFRESH_EVERY: f64 = 30.0;

/// Decodes `wsl.exe` output, which is UTF-16LE (with or without a BOM) on
/// most versions and UTF-8 when `WSL_UTF8=1`.
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = bytes.len() >= 2 && (bytes.starts_with(&[0xFF, 0xFE]) || bytes.iter().skip(1).step_by(2).take(8).all(|b| *b == 0));
    if utf16 {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

/// Distro names from `wsl --list --running --quiet`.
pub fn parse_running(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_matches('\0').trim())
        .filter(|l| !l.is_empty() && !l.contains(' ') && !l.ends_with(':'))
        .map(str::to_string)
        .collect()
}

/// `\\wsl.localhost\<distro>`.
pub fn distro_root(distro: &str) -> PathBuf {
    PathBuf::from(format!(r"\\wsl.localhost\{distro}"))
}

/// Polled roots under `fs_root` (a distro's `/`): for `/root` and each
/// `/home/<user>`, the `.claude`, `.copilot` and `.nuff-pulse` folders that
/// exist.
pub fn roots_in(fs_root: &Path, distro: &str) -> Vec<Root> {
    let mut homes = vec![fs_root.join("root")];
    let mut users: Vec<PathBuf> =
        std::fs::read_dir(fs_root.join("home")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    users.sort();
    homes.extend(users);
    let mut out = Vec::new();
    for home in homes {
        for (kind, name) in [(RootKind::Claude, ".claude"), (RootKind::Copilot, ".copilot"), (RootKind::Pulse, ".nuff-pulse")] {
            let path = home.join(name);
            if path.is_dir() {
                out.push(Root { kind, path, origin: Origin::Wsl(distro.to_string()), poll: true });
            }
        }
    }
    out
}

/// True when the registry lists at least one installed distro, so machines
/// without WSL never start `wsl.exe`.
#[cfg(windows)]
pub fn any_installed() -> bool {
    use windows_sys::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryInfoKeyW, HKEY, HKEY_CURRENT_USER, KEY_READ};
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Lxss\0".encode_utf16().collect();
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return false;
        }
        let mut subkeys = 0u32;
        let ok = RegQueryInfoKeyW(
            key,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut subkeys,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0;
        RegCloseKey(key);
        ok && subkeys > 0
    }
}

#[cfg(not(windows))]
pub fn any_installed() -> bool {
    false
}

/// Running distros, from `wsl.exe --list --running --quiet` (no window).
pub fn running() -> Vec<String> {
    if !any_installed() {
        return Vec::new();
    }
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.args(["--list", "--running", "--quiet"]).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match cmd.output() {
        Ok(out) if out.status.success() => parse_running(&decode(&out.stdout)),
        _ => Vec::new(),
    }
}

/// Roots of every running distro.
pub fn discover() -> Vec<Root> {
    running().iter().flat_map(|d| roots_in(&distro_root(d), d)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_wsl_output() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "Ubuntu\r\ndocker-desktop\r\n".encode_utf16() {
            bytes.extend(u.to_le_bytes());
        }
        assert_eq!(parse_running(&decode(&bytes)), vec!["Ubuntu", "docker-desktop"]);
        let no_bom: Vec<u8> = "Debian\r\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(parse_running(&decode(&no_bom)), vec!["Debian"]);
        assert_eq!(parse_running(&decode(b"Ubuntu-24.04\n")), vec!["Ubuntu-24.04"]);
        // An error message is not a distro list.
        assert!(parse_running("There are no running distributions.").is_empty());
    }

    #[test]
    fn finds_roots_in_homes() {
        let fs = std::env::temp_dir().join(format!("nuff-pulse-wsl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&fs);
        std::fs::create_dir_all(fs.join("home/me/.claude")).unwrap();
        std::fs::create_dir_all(fs.join("home/me/.nuff-pulse")).unwrap();
        std::fs::create_dir_all(fs.join("root/.copilot")).unwrap();
        std::fs::create_dir_all(fs.join("home/other")).unwrap();
        let roots = roots_in(&fs, "Ubuntu");
        let got: Vec<(RootKind, PathBuf)> = roots.iter().map(|r| (r.kind, r.path.clone())).collect();
        assert_eq!(
            got,
            vec![
                (RootKind::Copilot, fs.join("root/.copilot")),
                (RootKind::Claude, fs.join("home/me/.claude")),
                (RootKind::Pulse, fs.join("home/me/.nuff-pulse")),
            ]
        );
        assert!(roots.iter().all(|r| r.poll && r.origin == Origin::Wsl("Ubuntu".into())));
        std::fs::remove_dir_all(&fs).unwrap();
    }
}
