---
name: nuff-session-management
description: "Initialize, resume, checkpoint, and archive nuff sessions. Centralizes work item detection, config validation, STATE.md lifecycle, and resume logic."
---

# Nuff Session Management

This skill centralizes session lifecycle operations so `do.nuff` can focus on phase routing.

## Why this skill exists

`do.nuff` previously handled work item detection, config validation, branch creation, STATE.md initialization, resume detection, and progress presentation — all inline. This skill extracts those concerns into reusable operations with detailed logic in `references/` files (loaded only when needed).

## Operations

| Operation            | Typical callers | Purpose                                                         |
| -------------------- | --------------- | --------------------------------------------------------------- |
| `initialize-session` | `do.nuff`       | Detect work item, validate config, create STATE.md, set up branch |
| `resume-session`     | `do.nuff`       | Validate artifacts, detect partial work, present resume status  |
| `checkpoint-session` | `do.nuff`       | Save volatile state to STATE.md, commit artifacts               |
| `archive-and-restart`| `do.nuff`       | Archive vN artifacts, reset phase for re-opened work items      |

## Operation guidance

### `initialize-session`

Full initialization flow for a new or existing session. Consult [references/init-steps.md](./references/init-steps.md) for detailed steps covering:

1. **Work item detection** — parse arguments, check branch name, scan existing STATE.md files
2. **Configuration validation** — check/create `.nuff/config.json`, schema version check, alias resolution, platform detection
3. **STATE.md lifecycle** — create or read STATE.md, auto-create feature branch, commit initial state
4. **ADO process template detection** — one-time detection via `nuff-ado-orchestration`

Returns: `<item-name>`, `CONFIG`, `CONFIG_MODELS`, `PLATFORM`, `user_instruction`, `autonomous_mode`, and the active `phase`.

### `resume-session`

Runs after STATE.md is read. Validates phase claims against actual disk state. Consult [references/resume-detection.md](./references/resume-detection.md) for detailed steps covering:

1. **Artifact consistency check** — verify required files exist and are committed for the claimed phase
2. **Uncommitted artifact detection** — find in-progress work, offer commit or restart
3. **Code progress detection** — count completed vs total tasks from PLAN.md markers
4. **Branch check** — warn if not on expected nuff branch
5. **Manual change detection** — detect edits outside nuff since last commit
6. **Other in-progress items** — list other active work items
7. **Resume status presentation** — banner, progress table, confirmation prompt

Returns: resume context (progress details, uncommitted files, resume point) or signals fresh start.

### `checkpoint-session`

After a phase completes and the workflow has committed its artifacts:

1. Read STATE.md to confirm phase was updated by the workflow.
2. Log `phase-complete | <phase>` if not already logged by the workflow.
3. This is a coordination point — the workflow owns the actual commit; this operation verifies it happened.

### `archive-and-restart`

When a completed work item is re-opened for additional work (as seen in wi-1070805 v1→v2):

1. Rename current artifacts to versioned names: `SPEC.md` → `SPEC.v<N>.md`, `PLAN.md` → `PLAN.v<N>.md`
2. Update STATE.md artifacts table to mark archived files
3. Log `user-restart | re-opened for <reason>, old artifacts archived as v<N>`
4. Reset phase to `refine` (or specified phase) for the new cycle
5. Commit the archive operation

## Git validation

After any git operation within this skill, follow the [references/git-validation.md](./references/git-validation.md) protocol.

## Important notes

- This skill coordinates session lifecycle; it does not replace phase workflows.
- `nuff-state-update` remains the source of truth for STATE.md mutations.
- Config validation and platform detection results must be passed to phase workflows as context variables.
