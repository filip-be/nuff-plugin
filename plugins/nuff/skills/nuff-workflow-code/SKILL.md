---
name: nuff-workflow-code
description: "Orchestrate the code phase of the nuff pipeline. Use when executing PLAN.md tasks, committing work, and running verification."
---

# Code Workflow

This skill is consulted by the `do.nuff` orchestrator during the code phase. You ARE the orchestrator executing these steps. Do NOT delegate to a separate agent. When the workflow says to spawn a subagent, follow the **subagent model routing rule** from `PLATFORM` (see below) and consult the `nuff-subagent-execution` skill. On Copilot: NEVER invoke nuff agents directly — always use `@nuff-copilot-subagent-<alias>` wrappers.

## Available Context Variables

- `<item-name>` -- the work item name (from do.nuff context detection)
- `.nuff/<item-name>/STATE.md` -- path to the state file
- `CONFIG` -- user configuration from `.nuff/config.json`
- `CONFIG_MODELS` -- resolved model IDs from `CONFIG.models` (aliases expanded)
- `PLATFORM` -- `claude-code` or `copilot` (determines subagent spawning method)
- `autonomous_mode` -- true/false (from do.nuff invocation)

## Roles and Boundaries

- The orchestrator owns execution order, retries, progress/state updates, git commits, user interaction, and ADO side effects.
- Subagents own implementation, verification, and gap analysis that require deeper repo context than the orchestration loop should carry.
- If a repeated mechanic is covered by a shared skill, prefer the shared skill over restating the procedure inline.

This workflow orchestrates the full implementation and verification cycle. It spawns subagents for the actual work and manages the loops, git operations, and user interaction.

---

## Step 1: Read STATE.md and Verify Phase

Read `.nuff/<item-name>/STATE.md` to get the current context.

Verify that the `phase` field is `code`. If the phase is different, note this but proceed:

```
STATE.md shows phase '<current>', but code workflow was invoked. Proceeding anyway -- STATE.md will be updated.
```

Also verify the branch:

```bash
git branch --show-current
```

If the branch is not a `nuff-*` branch, print a warning but continue. Do NOT create or switch branches.

---

## Step 1.5: Check for Partial Implementation

Check for uncommitted files from a previous interrupted session:

```bash
git status --porcelain
```

If uncommitted files match PLAN.md task file lists, ask the user (consult the `ask-user` skill):

- **Keep and continue** -- stage, commit as recovery, and resume from next task
- **Discard** -- revert uncommitted changes and start fresh

If user keeps: commit with `nuff(<item-name>): recover partial work from interrupted session`.
If user discards: `git checkout -- <files>`.

**Autonomous mode:** Auto-keep and commit without prompting.

---

## Step 2: Load Context

### 2a. Read SPEC.md

Read `.nuff/<item-name>/SPEC.md`.

**Error:** If missing: `Error: No SPEC.md found. Run /do.nuff first.` Stop.

Extract requirements, acceptance criteria, out of scope, edge cases.

### 2b. Read PLAN.md and parse tasks

Read `.nuff/<item-name>/PLAN.md`.

**Error:** If missing: `Error: No PLAN.md found. Run /do.nuff first.` Stop.
**Error:** If no `### Task N:` headings: `Error: PLAN.md has no tasks.` Stop.

Extract:

- All task sections (number, name, files, approach, dependencies, parallel, requirements)
- Execution Order section (chapter groupings)
- Verification Notes section

### 2c. Read project documentation (via Explore subagent)

**Spawn an Explore subagent** to read project documentation and return a concise summary. This keeps raw documentation file contents out of the orchestrator's context window for the entire code phase.

The Explore subagent should:

1. Read project documentation files in priority order: AGENTS.md (check repo root, `.github/AGENTS.md`, `.claude/AGENTS.md`, `docs/AGENTS.md`, and child directories like `Backend/AGENTS.md`, `Frontend/AGENTS.md`), then CLAUDE.md (if exists), then README.md. Also check for `docs/` or `documentation/` directories and read key files (architecture docs, API docs, contributing guides).
2. Extract and return a concise structured summary covering:
   - **Directory structure:** key directories, naming conventions, entry points
   - **Build/test/lint commands:** exact commands to build, run tests, and lint
   - **Architectural patterns:** layers, module boundaries, DI patterns, test file locations and conventions
3. Keep the summary to 10-20 bullet points. Do not reproduce full file contents.

Wait for the Explore subagent to return. Store the result as `PROJECT_DOC_SUMMARY` to pass to implementer and verifier subagents.

**Parallelism note:** Steps 2a, 2b, and 2c can run in parallel. Steps 2a and 2b are direct file reads from `.nuff/<item-name>/`. Step 2c is an Explore subagent scanning project-level documentation. All three must complete before Step 3.

### 2d. Check for resumption

Check PLAN.md for `[done: ...]` markers on task headings. As fallback:

```bash
git log --oneline | grep "nuff(<item-name>): task"
```

Count completed tasks. If resuming, print: `Resuming: <done>/<total> tasks complete.`

### 2e. Load ADO context

Read from STATE.md: `ado_process_template`, `ado_tasks`, `work_item_is_simple`, `work_item_type`, `work_item_id`. If missing, ADO transitions will be skipped.

---

## Step 2.5: Initialize Task Progress Visualization

Consult the `nuff-progress-sync` skill to ensure the runtime task list reflects the current STATE.md. If the plan workflow already synced (which creates the chapter/task sub-items), this will be a no-op. If resuming, this will restore the progress items to match current STATE.md status.

---

## Step 2.6: Transition Work Item to In Progress

Skip this step if ANY of these conditions are true:

- STATE.md has no `work_item_id`
- STATE.md has no `ado_process_template` block
- STATE.md already has `ado_state: "<any value>"` in its frontmatter (already transitioned in a previous session)

Consult the `nuff-ado-orchestration` skill with `transition-parent-work-item`, using:

- `state: <ado_process_template.inprogress_state from STATE.md>`
- `assignTo:` the email returned by `git config user.email` (include only if non-empty)
- `message: Transitioned to '<inprogress_state>' — coding has started.`

The helper is responsible for calling `nuff-ado-update` and, on success, consulting `nuff-state-update` with `set-ado-state`.

This step applies to both simple items (Bug/Task) and parent items (User Story/PBI). It runs once at the start of the code phase, regardless of whether child tasks exist.

---

## Step 3: Execute Tasks (Implementation Loop)

Execute tasks in chapter order from PLAN.md's `## Execution Order` section.

### Chapter execution

For each chapter:

1. **Update chapter progress:** Set the chapter's progress item to `in_progress`.
2. **Collect tasks** in the chapter. Skip any with `[done:]` markers.
3. **Check file overlaps** within the chapter -- if any file appears in 2+ tasks, run those sequentially.
4. **Determine parallelism:** If 2+ independent tasks remain in the chapter, run them in parallel via subagents. Otherwise run sequentially.
5. **After all tasks in a chapter complete:** Set the chapter's progress item to `completed`. Write a 1-2 sentence chapter summary (tasks completed, commit hashes, any deviations from plan). Detailed subagent results from this chapter are now in committed code — reference PLAN.md `[done: hash]` markers going forward.

### Per-task execution (3a-3i)

**3a. ADO child task transition (if applicable):**

If this task has a matching `ado_tasks` entry in STATE.md (i.e., `ado_tasks[].plan_task == N`):

- Consult the `nuff-ado-orchestration` skill with `transition-child-task`, passing the child task reference, target state, and optional `assignTo:` email.
- All failures are non-blocking -- warn and continue.

Note: The parent work item transition to In Progress is handled in Step 2.6, not here.

**3b. Print task header and update progress:**

```
--- Task N/<total>: <task-name> ---
```

Consult the `nuff-state-update` skill with `mark-code-task-active` for the current task and chapter.

Then consult the `nuff-progress-sync` skill to sync the status change to the runtime task list.

**3c. Spawn nuff-implementer subagent:**

**Model selection:** Use `CONFIG_MODELS.implementer` as the model for this subagent.

Spawn the `nuff-implementer` subagent consulting the `nuff-subagent-execution` skill with:

- Item name and `.nuff/<item-name>/` path
- Task number, name, files list, and approach text
- Project documentation summary (from Step 2c)

**For parallel chapters:** Spawn all independent tasks simultaneously. Wait for ALL to complete before proceeding.

**For sequential tasks:** Spawn one at a time, wait for completion.

**3d. Process implementer result:**

Read the subagent's result. If status is `failure`:

- Print the error details.
- Ask the user: **Retry this task** / **Skip and continue** / **Stop implementation**
- If retry: re-spawn the implementer for this task (max 2 retries).
- If skip: note the skip and continue to next task.
- If stop: halt execution, leave STATE.md unchanged.

**3d.5. Collect implementer reflections:**

Consult the `nuff-reflection-capture` skill with:

- The full implementer result text
- Source label: `Task N: <task-name> (implementer)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

**3e. Per-task gap check (via nuff-gap-analyzer):**

**Configurable:** If `CONFIG.behavior.task_gap_check` is `"never"`, skip this step entirely and log `skip | task-N-gap-check — disabled by config`. If `"complex-only"`, skip if the task has <= 2 files AND the approach is <= 5 lines. See [references/ablation-guidance.md](./references/ablation-guidance.md) for rationale.

**Model selection:** Use `CONFIG_MODELS.gap_analyzer.code` as the model for this subagent.

After successful implementation, spawn the `nuff-gap-analyzer` subagent consulting the `nuff-subagent-execution` skill to check the task's output:

- Pass: the task's approach, requirements, and files changed
- Pass: the SPEC.md content for the requirements this task covers
- Mode: use as a lightweight check -- "does this task's implementation match its approach and requirements?"

As part of this check, the gap analyzer MUST also verify:

- If the task's Files list included test files: confirm those test files were written and contain tests covering the behavior described in the approach.
- If test files were expected but are missing or empty: treat this as a gap (same as a missing requirement).

Process the gap report:

- **No gaps:** Log `info | task-N gap-check — passed, no findings`. Continue to commit.
- **Gaps found:** Attempt to fix by re-spawning the nuff-implementer with the gap details appended to the approach. Maximum 3 re-implementation attempt per task.
- **Still gaps after retry:** Note the gaps, commit anyway, and flag for the bulk verification to catch.

**3e.5. Collect gap analyzer reflections:**

Consult the `nuff-reflection-capture` skill with:

- The full gap analyzer result text
- Source label: `Task N gap check (gap-analyzer)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

**3f. Verify staged changes:**

```bash
git status --short
```

Confirm only expected files changed. Note any deviations.

**3g. Stage and commit:**

```bash
git add path/to/file1.ts path/to/file2.ts
git commit -m "nuff(<item-name>): task N - <short-description>"
```

NEVER use `git add .` or `git add -A`. Stage files explicitly. For audit tasks with no changes, use `--allow-empty`.

After committing, validate per [nuff-session-management references/git-validation.md](../nuff-session-management/references/git-validation.md).

**3h. Mark task done in PLAN.md and update STATE.md Tasks section:**

Update the task heading in PLAN.md:

```
### Task N: <task-name> [done: <commit-hash>]
```

Consult the `nuff-state-update` skill with `mark-code-task-complete`, passing the task number, task name, parent chapter, current totals, and commit hash.

Amend the commit to include both PLAN.md and STATE.md updates:

```bash
git add .nuff/<item-name>/PLAN.md .nuff/<item-name>/STATE.md
git commit --amend --no-edit
```

**3i. Print completion and sync progress:**

```
Task N/<total> complete (<commit-hash>)
```

Consult the `nuff-progress-sync` skill to sync the task and chapter status changes to the runtime task list.

---

## Step 4: Bulk Verification

After ALL tasks are complete (or skipped), run bulk verification.

Consult the `nuff-state-update` skill with `set-verification-status` to mark Verification in progress.

Then consult the `nuff-progress-sync` skill to sync the status change.

Initialize: `verification_iteration = 1`, `max_iterations = 3`.

### 4a. Spawn nuff-verifier subagent

**Model selection:** Use `CONFIG_MODELS.verifier` as the model for this subagent.

Spawn the `nuff-verifier` subagent consulting the `nuff-subagent-execution` skill with:

- Item name and `.nuff/<item-name>/` path
- SPEC.md content
- PLAN.md content
- Project documentation summary (including build/test/lint commands)
- Iteration number
- Previous issues (if iteration 2+)
- `visual_verification`: value of `CONFIG.behavior.visual_verification` (pass `true` or `false`)

Wait for the verifier to return its report.

### 4b. Process verification report

If **all checks passed and all requirements satisfied:**

- Print: `Verification passed. All requirements satisfied.`
- Consult the `nuff-state-update` skill with `set-verification-status` to mark Verification complete.
- Consult the `nuff-progress-sync` skill to sync the status change.
- Proceed to Step 5.

**4b.5. Collect verifier reflections:**

Consult the `nuff-reflection-capture` skill with:

- The full verifier report text
- Source label: `Verification iteration N (verifier)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

If **issues found:**

- Print issue summary: `Verification found <N> issues.`
- Present issues to the user (consult the `ask-user` skill):
  - **Apply all fixes** -- approve all proposed fixes
  - **Select fixes** -- I'll choose which to apply
  - **Skip fixes** -- proceed without fixing

If user selects specific fixes, ask the user with multiSelect listing each issue.

### 4c. Apply approved fixes via implementer subagent

For each approved fix, treat it as a fix task and run through the same implementation loop as Step 3:

**Model selection:** Use `CONFIG_MODELS.implementer` as the model.

Spawn the `nuff-implementer` subagent (consult the `nuff-subagent-execution` skill) with the list of approved issues to fix, passing for each issue:

- Item name and `.nuff/<item-name>/` path
- The issue description and proposed fix as the task approach
- The affected files list
- Project documentation summary (from Step 2c)

After each implementer completes, run the per-task gap check (Step 3e):

**Model selection:** Use `CONFIG_MODELS.gap_analyzer.code` as the model.

Spawn the `nuff-gap-analyzer` subagent (consult the `nuff-subagent-execution` skill) to verify the fix matches its issue description and does not introduce new problems.

- **No gaps:** Proceed to commit.
- **Gaps found:** Attempt one re-implementation with gap details appended. If still failing, commit and note for the next verification iteration.

After all fixes applied, commit:

```bash
git add <fixed-files...>
git commit -m "nuff(<item-name>): verification fixes (iteration <N>)"
```

### 4d. Check loop termination

1. **All issues skipped** (no fixes applied): proceed to Step 5 with current status.
2. **Iteration = max_iterations** (3): proceed to Step 5; do not loop again.
3. **Fixes applied AND iteration < max_iterations**: increment iteration, return to Step 4a with the remaining/new issues as context.

Print: `Verification iteration <N> complete. <M> issues fixed, <K> remaining.`

When the verification loop exits (all issues resolved, all skipped, or max iterations reached):

- Consult the `nuff-state-update` skill with `set-verification-status` to mark Verification complete.
- Consult the `nuff-progress-sync` skill to sync the status change.

---

## Step 5: Mark Requirements and Acceptance Criteria in SPEC.md

After verification completes (regardless of final status), update SPEC.md to mark all requirements and acceptance criteria.

For each requirement (REQ-N) and its acceptance criteria:

- If SATISFIED: check the checkbox (`- [ ]` → `- [x]`) and add `[SATISFIED]` after the requirement/AC text.
- If NOT_SATISFIED: add `[NOT SATISFIED]` after the requirement/AC text (leave checkbox unchecked).

Use the Edit tool for each change. Then commit:

```bash
git add .nuff/<item-name>/SPEC.md
git commit -m "nuff(<item-name>): mark requirement status in SPEC.md"
```

---

## Step 6: User Approval Gate

Consult the `nuff-state-update` skill with `set-user-approval-status` to mark User approval in progress.

Then consult the `nuff-progress-sync` skill to sync the status changes.

Present the final status to the user and ask for approval.

Print a summary:

```
**Code phase summary for: <item-name>**

Tasks completed: N/M
Verification: [passed | passed_with_fixes | issues_remaining]
Requirements: X/Y satisfied

SPEC.md: .nuff/<item-name>/SPEC.md
PLAN.md: .nuff/<item-name>/PLAN.md
```

Ask the user (consult the `ask-user` skill):

- **Approved -- all looks good** -- proceed to commit and complete
- **Issues found -- I'll describe** -- tell me what needs fixing

### 6a. User fix loop

If the user reports issues:

1. Collect the user's feedback (what's wrong, which files, what's expected).
2. Determine which tasks/files need re-implementation.
3. Spawn nuff-implementer subagents for the fixes (consult the `nuff-subagent-execution` skill).
4. After fixes: spawn nuff-gap-analyzer to check (consult the `nuff-subagent-execution` skill).
5. Commit fixes: `nuff(<item-name>): user-requested fix (<brief description>)`
6. Re-run bulk verification (Step 4) to confirm all requirements still pass.
7. Update SPEC.md requirement markers (Step 5).
8. Re-present to user (return to Step 6).

Repeat until user approves.

---

## Step 6a.5: Back-propagate Approved Deviations to SPEC.md

After the user approval loop completes (Step 6/6a), check whether the user approved any behavior that differs from the original SPEC.md requirements. This keeps SPEC.md as a living specification that reflects the actual agreed-upon behavior.

**When to run:** Only if ANY of these occurred:

- The user reported issues in Step 6 that resulted in implementation changes
- The verifier found deviations that the user approved via "Skip fixes" or during fix selection
- Requirements were marked `[NOT SATISFIED]` in Step 5 but the user approved anyway

**When NOT to run:** If the user approved without any deviations (all requirements satisfied as-is), skip this step.

**What to do:**

1. Re-read `.nuff/<item-name>/SPEC.md`.
2. For each user-approved deviation (behavior that differs from a specific requirement or AC):
   - If the requirement needs rewording: Edit the requirement text to match the approved behavior. Append `[updated during code phase]` marker.
   - If the deviation is a scope reduction (requirement partially satisfied by design): Add a note under the requirement: `> Deviation: <description>. Approved by user during code phase.`
   - If an AC needs updating: Edit the AC to match the approved behavior.
3. For each deviation, consult `nuff-state-update` with `capture-decision`:
   - **Class:** Scope (accepted trade-off) or Logic (changed behavior)
   - **Decision:** What the approved deviation is
   - **Rationale:** Why the user approved the deviation
   - **Alternatives:** The original requirement as written
   - **Source:** `code — user-approved deviation`
4. Commit the updated SPEC.md and STATE.md:
   ```bash
   git add .nuff/<item-name>/SPEC.md .nuff/<item-name>/STATE.md
   git commit -m "nuff(<item-name>): update SPEC.md with approved deviations"
   ```
5. Log via `nuff-state-update` `append-session-log`: `spec-deviation | <N> requirements updated to match approved implementation`

---

## Step 7: ADO Review Transitions

This step runs after the user approves the implementation (Step 6). The user's approval is the authority — if they approved despite remaining issues, the work item should still transition to review.

Skip this step if:

- STATE.md has no `ado_process_template` block
- STATE.md has no `work_item_id`

Read `ado_process_template.review_states` from STATE.md. Determine the work item type from STATE.md (`work_item_type` field, or from the fetched work item — default to `"Task"` if not recorded).

Look up the entry for the actual work item type in `review_states`. If the type is not found in `review_states`, fall back to the `Task` entry. If neither is found, skip this step.

If the resolved `review_state` is `null`, skip this step.

If `review_state_closes` is `true`, print a note before transitioning:
`Note: Transitioning to '<review_state>' will close the work item (no Resolved-category state available).`

Consult the `nuff-ado-orchestration` skill with `transition-review-state`, passing:

- `review_states` from `ado_process_template`
- whether the item is simple or parent
- the tracked `ado_tasks` entries
- the message `Transitioned to '<review_state>' — coding completed and verified.`

The helper is responsible for applying child-task transitions when needed, transitioning the parent/simple item, and persisting successful `ado_state` changes through `nuff-state-update`. All failures remain non-blocking.

---

## Step 8: Update STATE.md and Final Commit

Consult the `nuff-state-update` skill with `complete-code`, passing the current timestamp and the fact that the user-approval gate is complete.

Then consult the `nuff-state-update` skill with `append-session-log` for `phase-complete | code`.

Consult the `nuff-progress-sync` skill to sync the final status changes (Code complete, Improve pending).

Commit:

```bash
git add .nuff/<item-name>/
git commit -m "nuff(<item-name>): code complete"
```

After committing, validate per [nuff-session-management references/git-validation.md](../nuff-session-management/references/git-validation.md).

---

## Step 9: Report Completion

Print:

```
**Code complete for: <item-name>**

Status: [passed | passed_with_fixes | issues_remaining]
Tasks: N/M completed
Verification iterations: K
Fixes applied: F
Requirements satisfied: X/Y

All artifacts committed to .nuff/<item-name>/
```

If all requirements satisfied:

```
Code phase complete. STATE.md transitioned to phase: improve.
```

---

## Important Constraints

- **Subagents for all heavy work.** Implementation is done by nuff-implementer subagents. Verification is done by the nuff-verifier subagent. Gap checking by nuff-gap-analyzer. This workflow orchestrates, manages git, and handles user interaction.
- **Subagents must NOT run git commands.** All git operations (stage, commit, amend) are handled by this workflow. Subagents write/modify files only.
- **Ask the user at every decision point** (consult the `ask-user` skill). Keep the interaction loop unbroken. Never print a plain text menu and wait.
- **Parallel execution within chapters.** When a chapter has 2+ tasks with no file overlaps, spawn implementer subagents in parallel. Wait for all to complete before committing.
- **Per-task gap check is lightweight and configurable.** Controlled by `CONFIG.behavior.task_gap_check` (`always` | `complex-only` | `never`). It does NOT do full spec compliance -- that's the verifier's job.
- **Bulk verification runs after ALL tasks.** Do not run verification after individual tasks. The per-task gap check catches obvious misses; the verifier catches cross-requirement issues.
- **Maximum 3 verification iterations.** After 3 iterations, proceed with whatever status remains. The user approval gate is the final safety net.
- **Verification fixes use the implementer subagent.** Approved fixes from the verifier report are applied via nuff-implementer (not inline Edit), followed by a gap check, then re-verification. This keeps fix quality consistent with the initial implementation.
- **User approval is the final gate.** Even if verification passes, the user must approve. If they find issues, the big loop runs again (implement fix -> gap check -> verify -> present).
- **Mark requirements in SPEC.md.** After verification, check all satisfied checkboxes and add [SATISFIED] / [NOT SATISFIED] markers to every requirement and acceptance criterion. This is the persistent verification record -- no separate VERIFICATION.md is produced.
- **Stage files explicitly by name.** Never use `git add .` or `git add -A`. Always stage by explicit path.
- **Stay on current branch.** Do NOT create or switch branches.
- **ADO transitions are non-blocking.** All ADO operations use MCP-first with CLI fallback. Failures warn and continue. The work item transitions to In Progress at code phase start (Step 2.6) and to review state after user approval (Step 7) -- both apply to simple and parent items alike.
- **Track ADO state in STATE.md.** For simple items, `ado_state` is stored directly in STATE.md frontmatter. For parent items, `ado_tasks` entries track child task states. Use Edit tool for targeted updates -- do not rewrite the entire file.
- **Trust the Execution Order section.** Execute chapters in order from PLAN.md. Do not re-derive dependencies.
- **Keep progress visualization current.** Consult the `nuff-progress-sync` skill after every STATE.md Tasks section update. The skill reads STATE.md and syncs the runtime task list -- never call `TaskCreate`/`TaskUpdate` (Claude Code), `manage_todo_list` (VS Code Copilot Chat), or `update_todo` (Copilot CLI) directly.
