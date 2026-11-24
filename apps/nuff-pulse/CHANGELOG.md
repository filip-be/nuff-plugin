# Changelog

## Unreleased

- Core (no UI yet): reads Claude Code transcripts and `~/.claude/sessions/<pid>.json`, GitHub Copilot `session-state` (`events.jsonl`, `workspace.yaml`, `inuse.<pid>.hold`) and nuff-pulse hook files; reduces them to one row per session with state, title, workspace, activity, subagents, context, cache countdown, model and cost.
- `nuff-pulse --dump` / `--watch` print what the core sees.
- Widget: floating bar (pill / list, draggable, position per monitor layout, hidden behind full-screen apps), tray icon with alert badge and menu, app window with session table, detail pane and settings, global hotkey, Focus window, Start with Windows, WSL distros, single instance.
