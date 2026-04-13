---
name: nuff-ado-orchestration
description: "Coordinate nuff Azure DevOps side effects. Use when fetching work-item context, detecting process templates, creating child tasks, posting approved artifacts, or recording ADO state transitions in STATE.md."
---

# Nuff ADO Orchestration

This skill is a thin orchestration layer over ADO-related work. It keeps the workflows focused on sequencing while reusing:

- `nuff-subagent-execution` for ADO retriever subagents
- `nuff-ado-update` for work item state transitions and comments
- `nuff-state-update` for persisting ADO metadata into `STATE.md`

## Why this skill exists

ADO behavior was previously spread across `do.nuff`, `refine`, `plan`, and `code`. The same concepts kept repeating:

- fetch work item context
- detect process template
- create and track child tasks
- post approved artifacts to ADO
- transition parent or child work items and record the result in `STATE.md`

This skill provides one place to describe those flows.

## When to use

Consult this skill whenever a command or workflow needs an ADO side effect or needs to keep ADO-related `STATE.md` fields synchronized.

## Operations

| Operation                     | Typical callers  | Purpose                                                                        |
| ----------------------------- | ---------------- | ------------------------------------------------------------------------------ |
| `fetch-work-item`             | `refine`         | Retrieve and normalize the current work item context                           |
| `detect-process-template`     | `do.nuff`        | Retrieve process-template state metadata and persist it in `STATE.md`          |
| `create-child-tasks`          | `plan`           | Create plan-derived child tasks, link them, and store `ado_tasks`              |
| `post-message`                | `plan`           | Add a nuff message comment without changing state                              |
| `post-artifact-approval`      | `refine`, `plan` | Post an approved artifact file to ADO                                          |
| `transition-parent-work-item` | `code`           | Transition the tracked parent/simple item and persist `ado_state` on success   |
| `transition-child-task`       | `code`           | Transition one tracked child task and persist its `ado_state` on success       |
| `transition-review-state`     | `code`           | Apply review-state transitions for the parent/simple item and tracked children |

## Shared rules

- If ADO is not configured (`CONFIG.ado.project` empty), skip and log via `nuff-state-update` `log-skip` with the operation name and reason `ADO not configured`.
- If there is no `work_item_id`, skip work-item-specific operations and log via `nuff-state-update` `log-skip` with the operation name and reason `no work_item_id`.
- All failures are non-blocking unless the caller declares the step blocking.
- Persist successful ADO state changes through `nuff-state-update` instead of inlining STATE.md edit recipes.

## Operation guidance

### `fetch-work-item`

Consult `nuff-subagent-execution` to run `nuff-ado-work-item-retriever` with the configured `CONFIG_MODELS.ado_work_item_retriever` alias. Return the normalized result to the caller. The caller decides how to use the fetched fields, but should consult `nuff-state-update` to persist work item metadata in `STATE.md`.

### `detect-process-template`

Consult `nuff-subagent-execution` to run `nuff-ado-process-retriever` with the configured `CONFIG_MODELS.ado_process_retriever` alias. On success, consult `nuff-state-update` with `store-ado-process-template` to persist the returned template block in frontmatter.

### `create-child-tasks`

Use MCP-first with CLI fallback to create each child task, link it to the parent, and collect the results as `ado_tasks` entries:

- success entry: `{plan_task: N, ado_id: <created_id>, ado_state: "New"}`
- failure entry: `{plan_task: N, ado_id: null, ado_state: "creation_failed"}`

After the creation loop, consult `nuff-state-update` with `store-ado-tasks` to persist the collected array. If the caller wants a summary comment in ADO, follow with `post-message`.

### `post-message`

Consult `nuff-ado-update` with `comment_type: message` and the supplied message.

### `post-artifact-approval`

Consult `nuff-ado-update` with `comment_type: artifact`, `artifact_label`, and `artifact_file`.

### `transition-parent-work-item`

For state transitions, follow the [references/idempotency-protocol.md](./references/idempotency-protocol.md). Consult `nuff-ado-update` with the requested `state`, optional `assignTo`, and message. If the update succeeds, consult `nuff-state-update` with `set-ado-state`.

### `transition-child-task`

For state transitions, follow the [references/idempotency-protocol.md](./references/idempotency-protocol.md). For a tracked child task, update the ADO work item directly using MCP-first with CLI fallback. Child task transitions do not need the artifact/comment behavior from `nuff-ado-update` unless the caller explicitly wants a comment. On success, consult `nuff-state-update` with `set-ado-child-task-state`.

### `transition-review-state`

For state transitions, follow the [references/idempotency-protocol.md](./references/idempotency-protocol.md). Resolve the correct review state from `ado_process_template.review_states`.

- For simple items: transition the tracked work item and persist top-level `ado_state` on success.
- For parent items: transition tracked child tasks first, persist each child `ado_state`, then transition the parent and persist top-level `ado_state` on success.

If `review_state_closes` is true, print the warning before transitioning.

## Important notes

- This skill coordinates ADO behavior; it does not replace the retriever agents or `nuff-ado-update`.
- `nuff-ado-update` remains the low-level primitive for comment-capable work item updates.
- `nuff-state-update` remains the source of truth for how ADO metadata is written back to `STATE.md`.
