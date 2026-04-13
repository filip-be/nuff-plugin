---
name: nuff-progress-sync
description: Synchronize runtime progress visualization (tasks/todos) with the current STATE.md content. Call this skill after every STATE.md update.
---

# Nuff Progress Sync

This skill reads the `## Tasks` section from STATE.md and synchronizes the runtime's task/todo list to match. It is the **single place** where progress visualization items are created or updated -- no other workflow code should call `TaskCreate`/`TaskUpdate` (Claude Code), `manage_todo_list` (VS Code Copilot Chat), or `update_todo` (Copilot CLI) directly.

## When to call this skill

Consult this skill **every time** STATE.md's `## Tasks` section is modified. Typical trigger points:

- **do.nuff orchestrator:** After initializing STATE.md (Step 2) and after each phase transition (Step 4)
- **refine workflow:** After updating STATE.md at the end (Step 5b)
- **plan workflow:** After updating STATE.md at the end (Step 6b) -- this is where Code sub-tasks first appear
- **code workflow:** After every task completion (Step 3h), after verification status changes (Step 4b), after user approval status changes (Step 6), and after the final STATE.md update (Step 8)
- **improve workflow:** After the final STATE.md update (Step 8)

## How it works

### 1. Read the Tasks section from STATE.md

Read `.nuff/<item-name>/STATE.md` and extract the `## Tasks` section. This section contains a nested checklist like:

```markdown
## Tasks

- [x] **Refine** -- SPEC.md complete
- [x] **Plan** -- PLAN.md complete (5 tasks across 3 chapters)
- [-] **Code** -- 2/5 tasks complete
  - [x] Chapter 1 (parallel)
    - [x] Task 1: add-enum (abc1234)
    - [x] Task 2: update-model (def5678)
  - [-] Chapter 2 (sequential)
    - [-] Task 3: wire-controller
    - [ ] Task 4: add-validation
  - [ ] Chapter 3 (parallel)
    - [ ] Task 5: update-ui
  - [ ] Verification
  - [ ] User approval
- [ ] **Improve** -- apply lessons learned to AGENTS.md and knowledge base
```

### 2. Parse checklist markers

Map the checklist markers to progress statuses:

| Marker | Status       |
| ------ | ------------ |
| `[x]`  | `completed`  |
| `[-]`  | `in_progress`|
| `[ ]`  | `pending`    |

### 3. Sync to runtime task/todo list

**Platform detection:** Determine the runtime platform:

- **Claude Code:** `TaskCreate` / `TaskUpdate` tools are available
- **VS Code (Copilot Chat):** `manage_todo_list` tool is available
- **GitHub Copilot CLI:** `update_todo` tool is available

### 3a. First invocation (no existing progress items)

If this is the first sync (no progress items exist yet), create all items from the Tasks section:

**Phase-level items** (Refine, Plan, Code, Improve):

- **Subject:** `<Phase>: <item-name>` (e.g., `Refine: wi-12345`)
- **Description:** The text after the `--` dash (e.g., `Clarify requirements and produce SPEC.md`)
- **ActiveForm (Claude Code):** Present continuous form: `Refining requirements`, `Planning tasks`, `Coding implementation`, `Applying improvements`
- **Status:** Derived from the checklist marker

**Code sub-items** (chapters, tasks, verification, user approval) -- only if the Code phase has sub-items in the Tasks section:

**Per chapter:**

- **Subject:** `Chapter N: <task-names>` (comma-separated task names from the chapter)
- **Description:** `<parallel|sequential> -- <count> task(s)`
- **ActiveForm:** `Chapter N: <task-names>`
- **Status:** Derived from the checklist marker

**Per task within chapter:**

- **Subject:** `Task N: <task-name>`
- **Description:** Derived from the task text in the checklist
- **ActiveForm:** `Implementing: <task-name>`
- **Status:** Derived from the checklist marker

**Verification and User approval:**

- **Subject:** `Verify: <item-name>` / `User approval: <item-name>`
- **Description:** `Run technical checks and spec compliance review` / `Present results to user for final review`
- **ActiveForm:** `Verifying implementation` / `Awaiting user approval`
- **Status:** Derived from the checklist marker

Set dependencies so items execute in order:

- Plan is blocked by Refine
- Code is blocked by Plan
- Improve is blocked by Code
- Within Code: chapters are sequential (each blocked by the previous), tasks within a chapter have no mutual dependencies, Verification is blocked by last chapter, User approval is blocked by Verification

Store the mapping of subject -> task ID so subsequent syncs can update existing items.

### 3b. Subsequent invocations (progress items already exist)

Compare the current Tasks section against the existing progress items:

1. **New items** (present in Tasks section but no matching progress item): Create them.
2. **Status changes** (progress item exists but status differs from checklist marker): Update the status.
3. **No change** (status matches): Skip -- do not update.

This means the skill is idempotent: calling it multiple times with the same STATE.md content produces no extra updates.

### 3c. Platform-specific implementation

**Claude Code:**

```
TaskCreate(subject: "<subject>", description: "<description>", activeForm: "<activeForm>")
TaskUpdate(id: "<task-id>", status: "completed" | "in_progress" | "pending")
```

Use TaskList to check for existing items before creating duplicates.

**VS Code (Copilot Chat):**

```
manage_todo_list(action: "add", subject: "<subject>", description: "<description>", status: "<status>")
manage_todo_list(action: "update", subject: "<subject>", status: "<status>")
```

**GitHub Copilot CLI:**

```
update_todo(action: "add", subject: "<subject>", description: "<description>", status: "<status>")
update_todo(action: "update", subject: "<subject>", status: "<status>")
```

## Important notes

- **STATE.md is the source of truth.** This skill only reads from STATE.md and writes to the runtime task/todo list. It never modifies STATE.md.
- **Idempotent.** Safe to call multiple times -- it only creates/updates items when there is a difference.
- **Lightweight.** This skill does not spawn subagents. It reads one file section and makes API calls.
- **No workflow logic.** This skill does not decide what status items should have -- it reflects whatever STATE.md says.
