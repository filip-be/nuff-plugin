//! `%APPDATA%\nuff-pulse\settings.toml`: bar position per monitor layout,
//! hotkey, zoom, extra roots and context window overrides. Unknown or broken
//! values fall back to defaults; the file is only written by the app.

use crate::model::{Config, Origin};
use crate::roots::{Root, RootKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_HOTKEY: &str = "Win+Alt+P";
/// Zoom limits: text and windows from half to three times their size.
pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 3.0;

/// A zoom within the limits, rounded to whole percents; 1 when not a number.
pub fn clamp_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    (z.clamp(ZOOM_MIN, ZOOM_MAX) * 100.0).round() / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarMode {
    Pill,
    #[default]
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraRoot {
    /// `claude`, `copilot` or `pulse`.
    pub kind: String,
    pub path: String,
    /// Poll every 5 s instead of watching (network or WSL paths).
    #[serde(default)]
    pub poll: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey: String,
    pub bar_visible: bool,
    pub bar_mode: BarMode,
    /// Size of text and windows (1 = 100 %); Ctrl+mouse wheel changes it.
    pub zoom: f32,
    /// Monitor layout key → bar position (physical pixels, top-left).
    pub bar_positions: BTreeMap<String, [i32; 2]>,
    pub extra_roots: Vec<ExtraRoot>,
    /// Model-name prefix → context window in tokens; the longest prefix wins.
    pub context_windows: BTreeMap<String, u64>,
    /// A broken settings file could not be kept aside: never overwrite it.
    #[serde(skip)]
    pub save_blocked: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey: DEFAULT_HOTKEY.into(),
            bar_visible: true,
            bar_mode: BarMode::List,
            zoom: 1.0,
            bar_positions: BTreeMap::new(),
            extra_roots: Vec::new(),
            context_windows: BTreeMap::new(),
            save_blocked: false,
        }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Settings {
        Settings::try_parse(text).unwrap_or_default()
    }

    pub fn try_parse(text: &str) -> Result<Settings, String> {
        let mut s: Settings = toml::from_str(text).map_err(|e| e.to_string())?;
        s.zoom = clamp_zoom(s.zoom);
        Ok(s)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// `%APPDATA%\nuff-pulse\settings.toml` (`~/.config/nuff-pulse` elsewhere).
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("nuff-pulse").join("settings.toml"))
    }

    /// The settings in `path`; defaults when it is missing. A file that
    /// does not parse is copied to `settings.toml.bad` first, so saving the
    /// defaults later does not lose what the user wrote.
    pub fn load_from(path: &Path) -> Settings {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            // Exists but unreadable: use defaults and leave the file alone.
            Err(_) => {
                return Settings {
                    save_blocked: true,
                    ..Default::default()
                };
            }
        };
        let parsed = std::str::from_utf8(&bytes)
            .map_err(|e| e.to_string())
            .and_then(Settings::try_parse);
        match parsed {
            Ok(s) => s,
            Err(_) => {
                let kept = std::fs::copy(path, path.with_extension("toml.bad")).is_ok();
                Settings {
                    save_blocked: !kept,
                    ..Default::default()
                }
            }
        }
    }

    pub fn load() -> Settings {
        Settings::path()
            .map(|p| Settings::load_from(&p))
            .unwrap_or_default()
    }

    /// Writes through a temporary file so a crash never leaves half a file.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if self.save_blocked {
            return Err(std::io::Error::other(
                "the settings file could not be read or backed up; not overwriting it",
            ));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.to_toml())?;
        std::fs::rename(&tmp, path)
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Settings::path() {
            Some(p) => self.save_to(&p),
            None => Err(std::io::Error::other("no settings folder")),
        }
    }

    /// Extra roots that name a known kind and a non-empty path.
    pub fn roots(&self) -> Vec<Root> {
        self.extra_roots
            .iter()
            .filter(|r| !r.path.trim().is_empty())
            .filter_map(|r| {
                let kind = parse_kind(&r.kind)?;
                Some(Root {
                    kind,
                    path: PathBuf::from(r.path.trim()),
                    origin: Origin::Windows,
                    poll: r.poll,
                })
            })
            .collect()
    }

    pub fn config(&self) -> Config {
        Config {
            context_windows: self
                .context_windows
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        }
    }
}

pub fn parse_kind(s: &str) -> Option<RootKind> {
    match s.trim().to_ascii_lowercase().as_str() {
        "claude" => Some(RootKind::Claude),
        "copilot" => Some(RootKind::Copilot),
        "pulse" | "nuff-pulse" => Some(RootKind::Pulse),
        _ => None,
    }
}

/// Monitor rectangles (left, top, right, bottom; physical pixels) to a key
/// that names the layout regardless of enumeration order.
pub fn layout_key(monitors: &[[i32; 4]]) -> String {
    let mut m: Vec<[i32; 4]> = monitors.to_vec();
    m.sort();
    m.iter()
        .map(|[l, t, r, b]| format!("{l},{t},{}x{}", r - l, b - t))
        .collect::<Vec<_>>()
        .join(";")
}

/// Modifier flags of a hotkey (`MOD_*` values of `RegisterHotKey`).
pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

/// `Win+Alt+P` → (modifiers, virtual-key code). At least one modifier and
/// exactly one key: a letter, a digit, `F1`–`F24`, `Space` or a few others.
pub fn parse_hotkey(s: &str) -> Option<(u32, u32)> {
    let mut mods = 0;
    let mut key = None;
    for part in s.split('+').map(str::trim) {
        let p = part.to_ascii_lowercase();
        match p.as_str() {
            "win" | "windows" | "super" | "meta" => mods |= MOD_WIN,
            "alt" => mods |= MOD_ALT,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "shift" => mods |= MOD_SHIFT,
            _ => {
                if key.is_some() {
                    return None;
                }
                key = Some(vk(&p)?);
            }
        }
    }
    (mods != 0).then_some(())?;
    Some((mods, key?))
}

fn vk(p: &str) -> Option<u32> {
    let b = p.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return Some(b[0].to_ascii_uppercase() as u32);
    }
    if let Some(n) = p.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then_some(0x70 + n - 1);
    }
    Some(match p {
        "space" => 0x20,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "esc" | "escape" => 0x1B,
        "home" => 0x24,
        "end" => 0x23,
        "insert" | "ins" => 0x2D,
        "delete" | "del" => 0x2E,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "`" | "backtick" => 0xC0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let mut s = Settings::default();
        s.bar_positions.insert("0,0,2560x1440".into(), [100, -20]);
        s.extra_roots.push(ExtraRoot {
            kind: "copilot".into(),
            path: "E:\\c".into(),
            poll: true,
        });
        s.context_windows.insert("claude-opus".into(), 1_000_000);
        assert_eq!(Settings::parse(&s.to_toml()), s);
        // Missing fields take defaults; a broken file is ignored.
        let p = Settings::parse("bar_mode = \"pill\"\n");
        assert_eq!(
            (p.bar_mode, p.hotkey.as_str(), p.bar_visible),
            (BarMode::Pill, DEFAULT_HOTKEY, true)
        );
        assert_eq!(Settings::parse("hotkey = [").hotkey, DEFAULT_HOTKEY);
        assert_eq!(p.zoom, 1.0);
    }

    #[test]
    fn zoom_is_clamped() {
        assert_eq!(Settings::parse("zoom = 1.25").zoom, 1.25);
        assert_eq!(Settings::parse("zoom = 9.0").zoom, ZOOM_MAX);
        assert_eq!(Settings::parse("zoom = 0.1").zoom, ZOOM_MIN);
        assert_eq!(Settings::parse("zoom = nan").zoom, 1.0);
        assert_eq!(clamp_zoom(1.234_567), 1.23);
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("nuff-pulse-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(&path, "hotkey = [\n").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
        assert_eq!(
            std::fs::read_to_string(dir.join("settings.toml.bad")).unwrap(),
            "hotkey = [\n"
        );
        // Not UTF-8: also kept aside.
        std::fs::write(&path, [0xFFu8, 0xFE, 0x00]).unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
        assert_eq!(
            std::fs::read(dir.join("settings.toml.bad")).unwrap(),
            vec![0xFF, 0xFE, 0x00]
        );
        assert!(
            Settings {
                save_blocked: true,
                ..Default::default()
            }
            .save_to(&path)
            .is_err()
        );
        let s = Settings {
            hotkey: "Ctrl+Alt+K".into(),
            ..Default::default()
        };
        s.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), s);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn roots_and_config() {
        let mut s = Settings::default();
        s.extra_roots.push(ExtraRoot {
            kind: "Claude".into(),
            path: " D:\\x\\.claude ".into(),
            poll: false,
        });
        s.extra_roots.push(ExtraRoot {
            kind: "nope".into(),
            path: "D:\\y".into(),
            poll: false,
        });
        s.extra_roots.push(ExtraRoot {
            kind: "pulse".into(),
            path: "".into(),
            poll: false,
        });
        let r = s.roots();
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].kind, r[0].path.clone()),
            (RootKind::Claude, PathBuf::from("D:\\x\\.claude"))
        );
        s.context_windows.insert("gpt".into(), 5);
        assert_eq!(s.config().context_window(Some("gpt-6"), None), Some(5));
    }

    #[test]
    fn hotkeys() {
        assert_eq!(
            parse_hotkey("Win+Alt+P"),
            Some((MOD_WIN | MOD_ALT, 'P' as u32))
        );
        assert_eq!(
            parse_hotkey("ctrl + shift + f12"),
            Some((MOD_CONTROL | MOD_SHIFT, 0x7B))
        );
        assert_eq!(parse_hotkey("Alt+Space"), Some((MOD_ALT, 0x20)));
        assert_eq!(parse_hotkey("P"), None);
        assert_eq!(parse_hotkey("Win+P+Q"), None);
        assert_eq!(parse_hotkey("Win+F25"), None);
        assert_eq!(parse_hotkey("Win+Alt"), None);
    }

    #[test]
    fn layout_keys_ignore_order() {
        let a = layout_key(&[[0, 0, 2560, 1440], [2560, 0, 4480, 1080]]);
        let b = layout_key(&[[2560, 0, 4480, 1080], [0, 0, 2560, 1440]]);
        assert_eq!(a, b);
        assert_eq!(a, "0,0,2560x1440;2560,0,1920x1080");
    }
}
