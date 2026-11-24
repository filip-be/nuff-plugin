# nuff-pulse

A small Windows widget that lists your active local agent sessions from Claude Code and GitHub Copilot, with their state (Working, Needs you, Done, Error), context size and prompt-cache countdown.

> Work in progress: install steps and a release come with the CI workflow.

```powershell
cargo run --release
```

## Using it

- **Floating bar**: borderless, always on top, drag it anywhere; its position is remembered per monitor layout. It hides while a full-screen app is in front. Two modes: a **pill** (counts and the most urgent cache countdown) and a **list** (one row per active session: state ring, title, state · activity, context, cache countdown, runtime and workspace). Click the pill to expand it; `–` in the list collapses it. **Win+Alt+P** (configurable) toggles pill ↔ list.
- **Click a row** to clear its alert and bring its window to the front: Windows Terminal, VS Code (the window whose title has the workspace folder), the GitHub Copilot app or the Claude app. Any other host is left alone rather than focusing a wrong window. WSL sessions are matched by window title.
- **Tray icon**: a badge with the number of alerts, blinking while a session needs you. Left click shows or hides the bar; the menu has Show bar, Open app, Settings, Start with Windows and Quit.
- **App window** (`☰` on the bar, or the tray): today's sessions with state, title, runtime, context, cache, model and cost, and a detail pane (workspace, branch, model, context meter, cache expiry, subagents, permission mode, last prompt, recent state changes, Focus window, Copy resume command).
- **Settings** (`%APPDATA%\nuff-pulse\settings.toml`, edited in the app window): hotkey, extra roots (folders read besides the defaults, optionally polled), context window per model-name prefix. Start with Windows adds or removes an `HKCU\…\Run` entry; it is off by default.
- **WSL**: the `~/.claude`, `~/.copilot` and `~/.nuff-pulse` folders of each *running* distro are read through `\\wsl.localhost\<distro>\` and polled every 5 s; the list of running distros is refreshed every 30 s. Stopped distros are never started.

Only one widget runs per Windows session; starting it again shows the bar.

For checking the core against real sessions, the binary can also print what it sees:

```powershell
cargo run --release -- --dump
```

```powershell
cargo run --release -- --watch
```

## What it reads

| Source | Files |
| ------ | ----- |
| Claude Code | `~/.claude/projects/<enc-cwd>/<id>.jsonl` (transcripts), `~/.claude/sessions/<pid>.json` (status) — `CLAUDE_CONFIG_DIR` is honoured |
| GitHub Copilot (CLI, app, VS Code Agent Host) | `~/.copilot/session-state/<id>/events.jsonl`, `workspace.yaml`, `inuse.<pid>.hold` — `COPILOT_HOME` is honoured |
| Hook plugins | `~/.nuff-pulse/sessions/<runtime>-<id>.hooks.jsonl`, written by the `nuff-pulse-cc` and `nuff-pulse-copilot` plugins, which document the format |

The widget works without the hook plugins, with less precise state: Done is then inferred from the log and its alert waits 3 s for further activity.

It only reads session files, except for deleting hook files of sessions that ended more than 24 hours ago (or that were untouched for 7 days).

## Development

```powershell
cargo test
```

```powershell
cargo clippy --all-targets -- -D warnings
```

Test fixtures in `tests/fixtures/` are sanitized real sessions: `tools/probe.py --fixtures` (Claude) and `tools/fixtures_copilot.py` (Copilot) write them with text replaced by `<str:N>`, ids remapped and times shifted.
