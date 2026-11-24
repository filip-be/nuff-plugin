//! nuff-pulse. The UI comes later; for now the binary prints what the core
//! sees, which is how the core is checked against real sessions.
//!
//!   nuff-pulse --dump     print today's sessions once
//!   nuff-pulse --watch    print them again whenever something changes

use nuff_pulse::engine::{Engine, Options};
use nuff_pulse::model::{Level, Row};
use nuff_pulse::{roots, util};
use std::time::Duration;

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

fn fmt_left(secs: f64) -> String {
    if secs <= 0.0 {
        "cold".into()
    } else if secs >= 3600.0 {
        format!("{}h{:02}m", (secs / 3600.0) as u64, ((secs % 3600.0) / 60.0) as u64)
    } else {
        format!("{}:{:02}", (secs / 60.0) as u64, (secs % 60.0) as u64)
    }
}

fn level(l: Level) -> &'static str {
    match l {
        Level::Ok => "",
        Level::Amber => "!",
        Level::Red => "!!",
    }
}

fn print_rows(rows: &[Row], now: f64) {
    println!("{}", "-".repeat(110));
    for r in rows {
        let ctx = match (r.context, r.context_window) {
            (Some(c), Some(w)) => format!("{}{}/{}{}", if r.context_approx { "~" } else { "" }, fmt_tokens(c), fmt_tokens(w), level(r.context_level)),
            (Some(c), None) => format!("{}{}{}", if r.context_approx { "~" } else { "" }, fmt_tokens(c), level(r.context_level)),
            _ => "-".into(),
        };
        let cache = r.cache_expires.map_or("-".into(), |e| fmt_left(e - now));
        let state = format!("{}{}", r.state.label(), if r.alert { " *" } else { "" });
        let wsl = match &r.key.origin {
            nuff_pulse::model::Origin::Wsl(d) => format!(" [WSL {d}]"),
            _ => String::new(),
        };
        println!(
            "{:<11} {:<40} {:>14} {:>7}  {}{} · {}",
            state,
            util::short(&r.title, 40),
            ctx,
            cache,
            r.label,
            wsl,
            r.workspace
        );
        let mut extra = Vec::new();
        if let Some(a) = &r.activity {
            extra.push(a.clone());
        }
        if let Some(d) = r.detail.as_ref().or(r.reason.as_ref()) {
            extra.push(d.clone());
        }
        if r.subagents > 0 {
            extra.push(format!("{} subagent(s)", r.subagents));
        }
        if let Some(m) = &r.model {
            extra.push(m.clone());
        }
        if r.cache_expiring {
            extra.push("cache expiring".into());
        }
        if !extra.is_empty() {
            println!("{:<11} {}", "", util::short(&extra.join(" · "), 98));
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let watch = match args.first().map(String::as_str) {
        Some("--dump") => false,
        Some("--watch") | None => true,
        _ => {
            eprintln!("usage: nuff-pulse [--dump | --watch]");
            std::process::exit(2);
        }
    };
    let mut engine = Engine::new(roots::default_roots(), Options { watch, ..Options::default() });
    let now = util::now();
    engine.start(now);
    let today = util::start_of_today(now);
    print_rows(&engine.model.rows(now, today), now);
    if !watch {
        return;
    }
    let mut last_print = now;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let now = util::now();
        if engine.poll(now) || now - last_print >= 60.0 {
            print_rows(&engine.model.rows(now, util::start_of_today(now)), now);
            last_print = now;
        }
    }
}
