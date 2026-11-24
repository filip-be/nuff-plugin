//! nuff-pulse core: reads Claude and GitHub Copilot session logs, the
//! nuff-pulse hook files and Claude's session status files, and reduces them
//! to one row per session. No UI here; see `main.rs` for a text dump.

pub mod claude;
pub mod copilot;
pub mod engine;
pub mod model;
pub mod procs;
pub mod pulse;
pub mod roots;
pub mod tailer;
pub mod util;
