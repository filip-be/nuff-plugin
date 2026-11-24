# Changelog

## Unreleased

- Core (no UI yet): reads Claude Code transcripts and `~/.claude/sessions/<pid>.json`, GitHub Copilot `session-state` (`events.jsonl`, `workspace.yaml`, `inuse.<pid>.hold`) and nuff-pulse hook files; reduces them to one row per session with state, title, workspace, activity, subagents, context, cache countdown, model and cost.
- `nuff-pulse --dump` / `--watch` print what the core sees.
