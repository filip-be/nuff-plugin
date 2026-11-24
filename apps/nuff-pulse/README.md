# nuff-pulse

A small Windows widget that lists your active local agent sessions from Claude Code and GitHub Copilot, with their state (Working, Needs you, Done, Error), context size and prompt-cache countdown.

> Work in progress: the core is done, the UI (floating bar, tray, app window) is not. For now the binary prints what it sees.

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

It only reads, except for deleting hook files of sessions that ended more than 24 hours ago (or that were untouched for 7 days).

## Development

```powershell
cargo test
```

```powershell
cargo clippy --all-targets -- -D warnings
```

Test fixtures in `tests/fixtures/` are sanitized real sessions: `tools/probe.py --fixtures` (Claude) and `tools/fixtures_copilot.py` (Copilot) write them with text replaced by `<str:N>`, ids remapped and times shifted.
