---
name: nuff-state-update
description: "Apply structured STATE.md mutations for nuff workflows. Use when initializing STATE.md, updating frontmatter, appending session log entries, recording ADO metadata, or transitioning between phases."
---

# Nuff State Update

This skill centralizes recurring `STATE.md` mutations so workflow skills can stay focused on orchestration instead of repeating edit recipes.

## Why this skill exists

`STATE.md` is the source of truth for nuff phase tracking, artifacts, progress, and session history. Multiple workflows need to update the same structures:

- YAML frontmatter
- `## Current Phase`
- `## Tasks`
- `## Phase History`
- `## Artifacts`
- `## Session Log`

Those mutations must stay consistent across `do.nuff`, `refine`, `plan`, `code`, and `improve`.

## When to use

Consult this skill whenever a workflow or command needs to mutate `STATE.md`. Typical operations include:

- Create the initial `STATE.md`
- Update or add frontmatter keys without rewriting the whole file
- Append session log lines
- Record ADO metadata (`ado_process_template`, `ado_tasks`, `ado_state`)
- Apply end-of-phase transitions
- Update code-phase task, verification, or user-approval checklist markers

## Core rules

- Prefer targeted edits. Preserve existing frontmatter fields and file structure whenever practical.
- Do not rewrite unrelated sections when only one section changes.
- Treat `STATE.md` as the single source of truth. If the `## Tasks` section changes, consult `nuff-progress-sync` after the edit.
- Keep session log entries append-only.

## Operations

| Operation                    | Typical callers             | Purpose                                                                     |
| ---------------------------- | --------------------------- | --------------------------------------------------------------------------- |
| `initialize-state`           | `do.nuff`                   | Create the initial `.nuff/<item-name>/STATE.md` from the canonical template |
| `update-frontmatter`         | `do.nuff`, `refine`, `code` | Add or replace one or more frontmatter keys                                 |
| `append-session-log`         | `do.nuff`, all workflows    | Append one or more lines to the `## Session Log` section                    |
| `store-ado-process-template` | `do.nuff`                   | Persist the detected `ado_process_template` block                           |
| `store-ado-tasks`            | `plan`                      | Persist or replace the `ado_tasks` array                                    |
| `set-ado-state`              | `code`                      | Set the top-level `ado_state` field in frontmatter                          |
| `set-ado-child-task-state`   | `code`                      | Update `ado_tasks[].ado_state` for a specific tracked child task            |
| `complete-refine`            | `refine`                    | Transition STATE from `refine` to `plan`                                    |
| `complete-plan`              | `plan`                      | Transition STATE from `plan` to `code` and scaffold code sub-tasks          |
| `mark-code-task-active`      | `code`                      | Mark one task and its chapter as in progress                                |
| `mark-code-task-complete`    | `code`                      | Mark one task complete and update the code progress counter                 |
| `set-verification-status`    | `code`                      | Mark Verification as pending, in progress, or complete                      |
| `set-user-approval-status`   | `code`                      | Mark User approval as pending, in progress, or complete                     |
| `complete-code`              | `code`                      | Transition STATE from `code` to `improve`                                   |
| `complete-improve`           | `improve`                   | Transition STATE from `improve` to `complete`                               |
| `capture-decision`           | `refine`, `plan`, `code`    | Record a structured decision in the `## Key Decisions` section              |
| `log-skip`                   | all workflows               | Log a skipped operation with reason to session log                          |

## Operation guidance

### `initialize-state`

Create `.nuff/<item-name>/STATE.md` using the canonical initial structure already defined by `do.nuff`:

- frontmatter with `item`, `phase`, work item fields, `created`, and `updated`
- `## Current Phase`
- `## Tasks`
- `## Phase History`
- `## Artifacts`
- `## Key Decisions`
- `## Session Log`

Use the actual runtime values for timestamps and work item identifiers.

### `capture-decision`

Append a structured decision entry to the `## Key Decisions` section. Each entry uses this format:

```markdown
### D-N: <short title> [<class>]
- **Class:** Architecture | Logic | Scope
- **Decision:** <what was decided>
- **Rationale:** <why this option was chosen>
- **Alternatives:** <what was considered and rejected>
- **Source:** <refine | plan | code> — <context>
```

Auto-increment `D-N` based on existing entries in the section (D-1, D-2, ...). If no entries exist yet, start with D-1.

Also append a session log entry via `append-session-log`: `decision-captured | D-N: <short title> [<class>]`.

### `update-frontmatter`

Use targeted edits to add or replace specific frontmatter keys. Preserve existing key order when practical, and always update `updated:` when the caller requests it.

### `append-session-log`

Ensure `## Session Log` exists, then append new entries using this format:

```markdown
- <ISO timestamp> | <event-type> | <details>
```

For standard event types, consult [references/event-taxonomy.md](./references/event-taxonomy.md).

### `log-skip`

Shorthand for logging a skipped operation. Appends:

```markdown
- <ISO timestamp> | skip | <operation_name> — <reason>
```

### `store-ado-process-template`

Persist the `ado_process_template` block in YAML frontmatter exactly once after detection succeeds.

### `store-ado-tasks`

Write or replace the full `ado_tasks` array in YAML frontmatter. Preserve unrelated frontmatter fields.

### `set-ado-state`

Set or replace the top-level `ado_state` frontmatter field.

### `set-ado-child-task-state`

Update the matching entry in `ado_tasks` by `plan_task` or `ado_id`, changing only its `ado_state` field.

### `complete-refine`

Apply the standard refine completion mutation set:

- `phase: plan`
- update `updated:`
- `## Current Phase: plan`
- mark Refine complete and Plan in progress in `## Tasks`
- add `SPEC.md` to `## Artifacts`
- close the `refine` row and add a `plan` row in `## Phase History`

### `complete-plan`

Apply the standard plan completion mutation set:

- `phase: code`
- update `updated:`
- `## Current Phase: code`
- mark Plan complete and expand the Code checklist from PLAN.md execution order
- add `PLAN.md` to `## Artifacts`
- close the `plan` row and add a `code` row in `## Phase History`

### `mark-code-task-active`

Mark the target task as `[-]` and ensure its parent chapter is also `[-]` if it was pending.

### `mark-code-task-complete`

Mark the target task as `[x]`, include the commit hash in the task line, increment the Code progress counter, and mark the parent chapter complete if all tasks in that chapter are complete.

### `set-verification-status`

Update the `Verification` checklist item to the requested status.

### `set-user-approval-status`

Update the `User approval` checklist item to the requested status.

### `complete-code`

Apply the standard code completion mutation set:

- mark `User approval` complete and `Code` complete in `## Tasks`
- `phase: improve`
- update `updated:`
- `## Current Phase: improve`
- close the `code` row and add an `improve` row in `## Phase History`

### `complete-improve`

Apply the standard improve completion mutation set:

- mark Improve complete in `## Tasks`
- `phase: complete`
- update `updated:`
- `## Current Phase: complete`
- close the `improve` row in `## Phase History`

## Important notes

- This skill defines mutation patterns. It does not replace workflow decisions.
- Callers still decide when a phase is complete or what values should be written.
- After any `## Tasks` mutation, consult `nuff-progress-sync`.
