---
name: nuff-ado-update
description: Update ADO work item state and/or add a nuff-prefixed comment. Use this skill whenever a nuff workflow transitions state, creates child tasks, approves artifacts, or reaches a milestone on a tracked work item.
---

# Nuff ADO Update

This skill handles ADO work item updates — state transitions, comments, or both — in a single call. It provides a consistent format and MCP/CLI routing so workflows only need to specify what happened.

## When to call this skill

| Event | Workflow | Step | State | Comment type |
| --- | --- | --- | --- | --- |
| SPEC.md approved | refine | 5c | — | artifact |
| Child tasks created | plan | 6a | — | message |
| PLAN.md approved | plan | 6c | — | artifact |
| Code phase started | code | 2.6 | `<inprogress_state>` | message |
| Code phase review | code | 7 | `<review_state>` | message |

## Prerequisites

Skip and log via `nuff-state-update` `log-skip` if ANY of these are true:

- STATE.md has no `work_item_id` — reason: `no work_item_id`
- ADO is not configured (`CONFIG.ado.project` is empty) — reason: `ADO not configured`

## How to use

When a workflow says "consult the `nuff-ado-update` skill", provide:

- **work_item_id** — the numeric ADO work item ID from STATE.md
- **project** — the ADO project from `CONFIG.ado.project`
- **state** (optional) — the target state to transition to. Omit for comment-only events.
- **assignTo** (optional) — ADO user email to assign the work item to (resolved via `git config user.email`). Applied together with the state transition. Omit if not needed.
- **comment_type** — either `message` or `artifact`

**For `message` type:**
- **message** — the event-specific text (e.g., `Transitioned to 'Active' — coding has started.`)

**For `artifact` type:**
- **artifact_label** — display label (e.g., `Specification`, `Plan`)
- **artifact_file** — path to the file (e.g., `.nuff/<item-name>/SPEC.md`)

## Implementation

### Step 1: State transition and/or assignment (if state or assignTo is provided)

**MCP path (preferred):**

```
Tool: wit_update_work_item
Parameters:
  id: <work_item_id>
  project: <project>
  state: <state>           # include only if state is provided
  assignedTo: <assignTo>   # include only if assignTo is provided
```

**CLI fallback (if MCP tool unavailable):**

```bash
az boards work-item update --id <work_item_id> --state "<state>" --assigned-to "<assignTo>" --project <project> --org https://dev.azure.com/<organization-name> --output json
```

Omit `--state` or `--assigned-to` flags if the corresponding parameter was not provided.

On success: print `ADO #<work_item_id> → <state>` (include ` (assigned to <assignTo>)` if assignTo was provided)
On failure: print `Warning: Could not update ADO #<work_item_id>.`

**Return the transition result** to the calling workflow: `{ success: true/false, new_state: "<state>", current_state: "<actual ADO state or unknown>" }`. The caller uses `current_state` to keep STATE.md accurate even on failure. The skill does NOT update STATE.md itself.

### Step 2: Add comment (always)

Regardless of whether the state transition succeeded or was skipped, add the comment.

#### Message comment format

```
**Nuff:** <message>
```

#### Artifact comment format

Read the artifact file content from disk. Build the comment as markdown with a collapsible `<details>` block:

```markdown
**Nuff:** <artifact_label> has been approved.
<details>
<summary><artifact_file basename></summary>

<artifact file content>
</details>
```

Example for SPEC.md:

```markdown
**Nuff:** Specification has been approved.
<details>
<summary>SPEC.md</summary>

# SPEC.md
**Item:** wi-12345
...full SPEC.md content...
</details>
```

#### MCP call

```
Tool: wit_add_work_item_comment
Parameters:
  project: <project>
  workItemId: <work_item_id>
  comment: <formatted comment>
  format: "markdown"
```

#### CLI fallback

```bash
az boards work-item update --id <work_item_id> --discussion "<formatted comment>" --project <project> --org https://dev.azure.com/<organization-name> --output json
```

On failure: print `Warning: Could not add comment to ADO #<work_item_id>.` and continue.

## Error handling

All failures are **non-blocking**. State transition failure does NOT prevent the comment from being attempted. Comment failure does NOT affect the workflow.
