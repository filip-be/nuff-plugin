//! nuff-pulse core: reads Claude and GitHub Copilot session logs, the
//! nuff-pulse hook files and Claude's session status files, and reduces them
//! to one row per session. The widget UI lives in the binary (`main.rs`,
//! `ui/`); the Windows shell parts it drives (tray, hotkey, focus) are here.

pub mod claude;
pub mod copilot;
#[cfg(windows)]
pub mod desktop;
pub mod engine;
pub mod fmt;
pub mod focus;
pub mod icon;
pub mod model;
pub mod procs;
pub mod pulse;
pub mod roots;
pub mod settings;
#[cfg(windows)]
pub mod shell;
pub mod tailer;
pub mod util;
pub mod wsl;
