//! Reads only the bytes appended to a line-oriented log since the last read.
//!
//! A tailer keeps a byte offset and the partial last line. On first open of a
//! large file it reads only the head (the first 64 KB: session start, first
//! prompt) and the tail (the last ~256 KB: current state), never the middle.
//! Files are opened with share read|write|delete so writers are never blocked.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const HEAD_BYTES: u64 = 64 * 1024;
pub const TAIL_BYTES: u64 = 256 * 1024;
/// The tail window grows (×4) up to this size until it holds an anchor line.
pub const MAX_TAIL_BYTES: u64 = 16 * 1024 * 1024;
const CHUNK: usize = 1024 * 1024;

pub fn open_shared(path: &Path) -> io::Result<File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        o.share_mode(0x1 | 0x2 | 0x4);
    }
    o.open(path)
}

/// Lines read from a file in one call.
#[derive(Debug, Default)]
pub struct Chunk {
    /// Complete lines from the file's head (first open of a large file only).
    pub head: Vec<String>,
    /// Complete lines in file order (after `head`).
    pub lines: Vec<String>,
    /// True when the file shrank or was replaced and was re-read from the start.
    pub reset: bool,
    /// True when bytes between `head` and `lines` were not read.
    pub skipped: bool,
}

#[derive(Debug)]
pub struct Tailer {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    opened: bool,
    anchors: &'static [&'static str],
}

impl Tailer {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Tailer { path: path.into(), offset: 0, partial: Vec::new(), opened: false, anchors: &[] }
    }

    /// On first open of a large file, the tail window grows until one of its
    /// lines contains one of `anchors` (substrings of the records that carry
    /// state), up to `MAX_TAIL_BYTES` or the head.
    pub fn with_anchors(path: impl Into<PathBuf>, anchors: &'static [&'static str]) -> Self {
        Tailer { anchors, ..Tailer::new(path) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes consumed so far (including the pending partial line).
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Reads what is new. The first call on a file larger than head + tail
    /// returns the head lines and the tail lines, skipping the middle.
    pub fn read(&mut self) -> io::Result<Chunk> {
        let mut file = open_shared(&self.path)?;
        let len = file.metadata()?.len();
        let mut chunk = Chunk::default();
        if !self.opened {
            self.opened = true;
            if len > HEAD_BYTES + TAIL_BYTES {
                let mut head = vec![0u8; HEAD_BYTES as usize];
                file.read_exact(&mut head)?;
                let cut = head.iter().rposition(|&c| c == b'\n').map_or(0, |p| p + 1) as u64;
                chunk.head = split_lines(&head[..cut as usize]);
                let mut window = TAIL_BYTES;
                loop {
                    let start = len.saturating_sub(window).max(cut);
                    // Drop the line the window starts inside of, unless it
                    // starts exactly at a line start.
                    let mut at_line_start = start == cut;
                    if !at_line_start {
                        file.seek(SeekFrom::Start(start - 1))?;
                        let mut prev = [0u8; 1];
                        file.read_exact(&mut prev)?;
                        at_line_start = prev[0] == b'\n';
                    }
                    file.seek(SeekFrom::Start(start))?;
                    self.offset = start;
                    self.partial.clear();
                    let mut lines = self.read_from(&mut file, len)?;
                    if !at_line_start && !lines.is_empty() {
                        lines.remove(0);
                    }
                    let anchored = self.anchors.is_empty()
                        || lines.iter().any(|l| self.anchors.iter().any(|a| l.contains(a)));
                    if anchored || start == cut || window >= MAX_TAIL_BYTES {
                        chunk.skipped = start > cut;
                        chunk.lines = lines;
                        return Ok(chunk);
                    }
                    window *= 4;
                }
            }
        }
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
            chunk.reset = true;
        }
        if len == self.offset {
            return Ok(chunk);
        }
        file.seek(SeekFrom::Start(self.offset))?;
        chunk.lines = self.read_from(&mut file, len)?;
        Ok(chunk)
    }

    fn read_from(&mut self, file: &mut File, len: u64) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        let mut buf = vec![0u8; CHUNK.min((len - self.offset) as usize).max(1)];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            self.offset += n as u64;
            let data = &buf[..n];
            let mut start = 0;
            for (i, &c) in data.iter().enumerate() {
                if c == b'\n' {
                    self.partial.extend_from_slice(&data[start..i]);
                    push_line(&mut out, &self.partial);
                    self.partial.clear();
                    start = i + 1;
                }
            }
            self.partial.extend_from_slice(&data[start..]);
        }
        Ok(out)
    }
}

fn push_line(out: &mut Vec<String>, bytes: &[u8]) {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    if bytes.iter().any(|c| !c.is_ascii_whitespace()) {
        out.push(String::from_utf8_lossy(bytes).into_owned());
    }
}

fn split_lines(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    for line in data.split(|&c| c == b'\n') {
        push_line(&mut out, line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nuff-pulse-tailer-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&dir);
        dir
    }

    fn append(p: &Path, s: &str) {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn reads_appended_lines_and_keeps_partial() {
        let p = tmp("append");
        append(&p, "a\nb\npar");
        let mut t = Tailer::new(&p);
        assert_eq!(t.read().unwrap().lines, vec!["a", "b"]);
        assert!(t.read().unwrap().lines.is_empty());
        append(&p, "tial\r\n\nc\n");
        assert_eq!(t.read().unwrap().lines, vec!["partial", "c"]);
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn restarts_when_truncated() {
        let p = tmp("trunc");
        append(&p, "one\ntwo\n");
        let mut t = Tailer::new(&p);
        t.read().unwrap();
        std::fs::write(&p, "x\n").unwrap();
        let c = t.read().unwrap();
        assert!(c.reset);
        assert_eq!(c.lines, vec!["x"]);
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn large_file_reads_head_and_tail_only() {
        let p = tmp("large");
        let line = format!("{}\n", "x".repeat(99)); // 100 bytes
        let total = (HEAD_BYTES + TAIL_BYTES) / 100 + 2000;
        let mut body = String::new();
        for i in 0..total {
            if i == 0 {
                body.push_str(&format!("{:<99}\n", "first"));
            } else if i == total - 1 {
                body.push_str(&format!("{:<99}\n", "last"));
            } else {
                body.push_str(&line);
            }
        }
        std::fs::write(&p, &body).unwrap();
        let mut t = Tailer::new(&p);
        let c = t.read().unwrap();
        assert_eq!(c.head[0].trim(), "first");
        assert!(c.skipped);
        assert_eq!(c.head.len() as u64, HEAD_BYTES / 100);
        assert_eq!(c.lines.last().unwrap().trim(), "last");
        // The window starts inside a line, which is dropped.
        assert_eq!(c.lines.len() as u64, TAIL_BYTES / 100);
        assert_eq!(t.offset(), body.len() as u64);
        append(&p, "next\n");
        assert_eq!(t.read().unwrap().lines, vec!["next"]);
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn tail_window_grows_until_an_anchor() {
        let p = tmp("anchor");
        let filler = format!("{}\n", "x".repeat(99));
        let mut body = String::new();
        for _ in 0..1000 {
            body.push_str(&filler);
        }
        body.push_str(&format!("{:<99}\n", "ANCHOR"));
        // ~1.2 MB after the anchor: beyond the first two windows.
        for _ in 0..12_000 {
            body.push_str(&filler);
        }
        std::fs::write(&p, &body).unwrap();
        let mut t = Tailer::with_anchors(&p, &["ANCHOR"]);
        let c = t.read().unwrap();
        assert!(c.lines.iter().any(|l| l.contains("ANCHOR")));
        assert!(!c.skipped, "the window grew back to the head");
        // Head and tail never overlap.
        assert_eq!(c.head.len() + c.lines.len(), 13_001);
        let mut plain = Tailer::new(&p);
        assert!(!plain.read().unwrap().lines.iter().any(|l| l.contains("ANCHOR")));
        std::fs::remove_file(&p).unwrap();
    }
}
