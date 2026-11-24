//! Small helpers: time parsing, text shortening, JSON accessors.

use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current time, Unix seconds.
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Unix seconds of a `SystemTime` (file modification times).
pub fn unix(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Parses an RFC 3339 / ISO 8601 timestamp such as `2026-11-24T09:52:21.190Z`
/// or `2026-11-24T10:52:21+01:00` to Unix seconds.
pub fn parse_iso(s: &str) -> Option<f64> {
    let b = s.trim().as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        std::str::from_utf8(&b[r]).ok()?.parse::<i64>().ok()
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if b[13] != b':' || b[16] != b':' || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut i = 19;
    let mut frac = 0.0;
    if i < b.len() && b[i] == b'.' {
        let start = i + 1;
        i = start;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let digits = std::str::from_utf8(&b[start..i]).ok()?;
        if !digits.is_empty() {
            frac = format!("0.{digits}").parse::<f64>().ok()?;
        }
    }
    let mut offset = 0i64;
    if i < b.len() {
        match b[i] {
            b'Z' | b'z' => {}
            b'+' | b'-' if b.len() >= i + 6 => {
                let sign = if b[i] == b'-' { -1 } else { 1 };
                let oh = num(i + 1..i + 3)?;
                let om = num(i + 4..i + 6)?;
                offset = sign * (oh * 3600 + om * 60);
            }
            _ => return None,
        }
    }
    let days = days_from_civil(y, mo, d);
    Some((days * 86400 + h * 3600 + mi * 60 + se - offset) as f64 + frac)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Windows FILETIME ticks (100 ns since 1601) to Unix seconds.
pub fn filetime_to_unix(ticks: u64) -> f64 {
    ticks as f64 / 1e7 - 11_644_473_600.0
}

/// Unix seconds of the most recent local midnight.
pub fn start_of_today(now: f64) -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut st) };
        let since = st.wHour as f64 * 3600.0
            + st.wMinute as f64 * 60.0
            + st.wSecond as f64
            + st.wMilliseconds as f64 / 1000.0;
        now - since
    }
    #[cfg(not(windows))]
    {
        now - now.rem_euclid(86400.0)
    }
}

/// First line of `s`, trimmed, at most `max` characters (with `…` when cut).
pub fn short(s: &str, max: usize) -> String {
    let line = s.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() <= max {
        line.to_string()
    } else {
        let mut out: String = line.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// `v[key]` as a non-empty string.
pub fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|x| !x.is_empty())
}

/// `v[key]` as a number (integer or float).
pub fn f(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

/// `v[key]` as a non-negative integer.
pub fn u(v: &Value, key: &str) -> Option<u64> {
    let x = v.get(key)?;
    x.as_u64()
        .or_else(|| x.as_f64().filter(|n| *n >= 0.0).map(|n| n as u64))
}

/// Last path component of a Windows or POSIX path.
pub fn folder_name(path: &str) -> String {
    path.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_parsing() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(parse_iso("2025-11-24T16:00:00Z"), Some(1764000000.0));
        let t = parse_iso("2025-11-24T16:00:00.250Z").unwrap();
        assert!((t - 1764000000.25).abs() < 1e-6);
        assert_eq!(parse_iso("2025-11-24T17:00:00+01:00"), Some(1764000000.0));
        assert_eq!(
            parse_iso("2025-11-24T16:00:00.914000Z").map(|t| (t * 1000.0).round()),
            Some(1764000000914.0)
        );
        assert_eq!(parse_iso("nope"), None);
        assert_eq!(parse_iso("2025-13-01T00:00:00Z"), None);
    }

    #[test]
    fn filetime() {
        // 134400005932987732 is a procStart seen in ~/.claude/sessions
        let t = filetime_to_unix(134400005932987732);
        assert!((t - 1795526993.2987732).abs() < 1e-3);
    }

    #[test]
    fn shortening() {
        assert_eq!(short("  hello\nworld", 10), "hello");
        assert_eq!(short("abcdefghij", 5), "abcd…");
        assert_eq!(folder_name("D:\\work\\shop\\"), "shop");
        assert_eq!(folder_name("/home/me/proj"), "proj");
    }
}
