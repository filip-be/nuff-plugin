---
name: nuff-workflow-plan
description: "Orchestrate the plan phase of the nuff pipeline. Use when turning SPEC.md into an approved PLAN.md with ordered tasks and dependencies."
---

# Plan Workflow

This skill is consulted by the `do.nuff` orchestrator during the plan phase. You ARE the orchestrator executing these steps. Do NOT delegate to a separate agent. When the workflow says to spawn a subagent, follow the **subagent model routing rule** from `PLATFORM` (see below) and consult the `nuff-subagent-execution` skill. On Copilot: NEVER invoke nuff agents directly — always use `@nuff-copilot-subagent-<alias>` wrappers.

## Available Context Variables

- `<item-name>` -- the work item name (from do.nuff context detection)
- `.nuff/<item-name>/STATE.md` -- path to the state file
- `CONFIG` -- user configuration from `.nuff/config.json`
- `CONFIG_MODELS` -- resolved model IDs from `CONFIG.models` (aliases expanded)
- `PLATFORM` -- `claude-code` or `copilot` (determines subagent spawning method)
- `autonomous_mode` -- true/false (from do.nuff invocation)

## Roles and Boundaries

- The orchestrator owns user interaction, PLAN.md writes, STATE.md updates, and any git or ADO side effects.
- Subagents own deep repo analysis and plan quality review that would otherwise clutter the workflow body.
- If a repeated mechanic is covered by a shared skill, prefer the shared skill over restating the procedure inline.

---

## Step 1: Read STATE.md and Verify Phase

Read `.nuff/<item-name>/STATE.md` to get the current context (phase, work item ID, timestamps, existing artifacts).

Verify that the `phase` field is `plan`. If the phase is different, note this but proceed:

```
STATE.md shows phase '<current>', but plan workflow was invoked. Proceeding anyway -- STATE.md will be updated.
```

---

## Step 1.5: Check for Previous Plan

Check if `.nuff/<item-name>/PLAN.md` already exists:

1. **Committed PLAN.md exists:** This is likely a re-run. Print: `Found existing PLAN.md. This will be overwritten with a fresh plan.` Ask the user (consult the `ask-user` skill):
   - **Regenerate** -- create a fresh plan from SPEC.md
   - **Keep and approve** -- proceed to approval gate with existing plan

2. **Uncommitted PLAN.md exists:** Interrupted mid-write. Run `git status --porcelain .nuff/<item-name>/PLAN.md` to check. Read it. If it has a complete task list (has `## Execution Order` section), ask the user:
   - **Continue** -- approve the existing plan as-is and proceed to approval gate
   - **Restart** -- regenerate the plan from SPEC.md

   If incomplete (missing Execution Order section), proceed to regenerate but use existing content as reference for task structure.

3. **No PLAN.md exists:** Fresh start. Proceed normally.

**Autonomous mode:** If `autonomous_mode=true` and an existing PLAN.md looks complete (has Execution Order section), auto-approve it and skip regeneration -- proceed directly to Step 5 (Approval Gate, which auto-approves in autonomous mode).

---

## Step 1.7: Load Knowledge Context

Check if `.nuff/knowledge/INDEX.md` exists using the Glob tool.

**If it exists:**

1. Read `.nuff/knowledge/INDEX.md` using the Read tool.
2. Store the content as `KNOWLEDGE_INDEX` for use in codebase analysis and gap analysis.
3. Print: `Loaded knowledge index (<N> entries across <C> categories).`

**If it does not exist:**

- Set `KNOWLEDGE_INDEX = null`.
- This is normal for the first work item — no warning needed.

Pass `KNOWLEDGE_INDEX` to sub-agents in later steps:

- **Step 3 (codebase analyzer):** Append `KNOWLEDGE_INDEX` to the subagent input so it can use convention/pattern knowledge to inform file discovery (e.g., if knowledge says "API controllers live in Controllers/ not Api/", it adjusts its search accordingly).
- **Step 4.5 (gap analyzer):** Append `KNOWLEDGE_INDEX` to the subagent input so it can flag plan approaches that conflict with known pitfalls (e.g., if knowledge says "parallel tasks must not share files" and the plan has parallel tasks on the same file).

Format when passing to sub-agents (if `KNOWLEDGE_INDEX` is non-null):

```
## Relevant Knowledge (from previous sessions)

<KNOWLEDGE_INDEX content>
```

---

## Step 2: Load Spec

### 2a. Read SPEC.md

Read `.nuff/<item-name>/SPEC.md`.

Extract and note:

- **Requirements section** -- numbered list (REQ-1, REQ-2, ...). This is the primary input for task decomposition.
- **Acceptance Criteria** -- per-requirement criteria, for reference during approach writing.
- **Out of Scope** -- items explicitly excluded. Tasks MUST NOT cover these.
- **Edge Cases** -- specific scenarios to handle during implementation.
- **Open Questions** -- any unresolved questions.

**Error conditions (stop and report before proceeding):**

- If SPEC.md does not exist at `.nuff/<item-name>/SPEC.md`:

  ```
  Error: No SPEC.md found at .nuff/<item-name>/SPEC.md. Run /do.nuff first to produce a spec before planning.
  ```

  Stop.

- If the Requirements section exists but is empty (no numbered items):
  ```
  Error: SPEC.md Requirements section is empty. Return to refine phase to define requirements before planning.
  ```
  Stop.

**Warning condition (ask before proceeding):**

- If the Open Questions section is non-empty (contains unresolved questions, not just "None -- all questions resolved"):
  Ask the user (consult the `ask-user` skill):
  - **Proceed anyway** -- I'll note which tasks are affected by the ambiguity
  - **Go back to refine** -- resolve these questions first

  If the user chooses to go back, stop. If they choose to proceed, note the ambiguous questions in the PLAN.md Summary.

---

## Step 3: Analyze Codebase (via subagent)

**Spawn the `nuff-codebase-analyzer` subagent consulting the `nuff-subagent-execution` skill.** This keeps all the Glob/Grep/Read operations for file discovery out of the main context window.

Spawn the subagent passing:

- The SPEC.md content (requirements, out of scope, edge cases)
- The item name and `.nuff/<item-name>/` path

**Model selection:** Use `CONFIG_MODELS.codebase_analyzer.plan` as the model for this subagent.

The subagent will:

1. Read project docs (AGENTS.md, README.md, architecture docs)
2. Perform per-requirement Glob/Grep file discovery
3. Determine file operation types (Modify/Create/Delete) for each requirement
4. Identify shared files across requirements (dependency implications)
5. Return a structured **File Target Map**

Wait for the subagent to return the file target map before proceeding.

### Step 3.5: Collect codebase analyzer reflections

Consult the `nuff-reflection-capture` skill with:

- The full codebase analyzer result text
- Source label: `Plan codebase analysis (codebase-analyzer)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

---

## Step 4: Decompose into Tasks and Generate PLAN.md

Using the file target map from the subagent, map requirements to implementation tasks and write PLAN.md.

### 4a. Task decomposition rules

Apply these rules when deciding how to group requirements into tasks:

- **One task = one logical unit** resulting in approximately one commit. Changes that are tightly coupled in the same file or the same concern belong in one task.
- **Tests are part of the task, not separate.** Every task that introduces or changes behavior MUST include its test file(s) in the Files list. Tests are written within the same task as the implementation — never in a separate task or a later chapter. Do NOT create standalone "write tests for X" tasks.
- **Task sizing:** Split a task when it crosses a logical or vertical boundary — e.g., it mixes unrelated concerns, spans multiple independent features, or combines a data-layer change with a UI change that could be developed and reviewed separately. Do NOT split based on file count alone.
- **Trivial changes:** If a task is a single-line change in one file, combine it with a related task in the same file or layer.
- **No verification tasks:** Do NOT create implementation tasks for pure verification requirements (e.g., "no functional regression", "no type errors"). These go in the Verification Notes section.
- **Respect Out of Scope:** Do NOT create tasks for items in SPEC.md's Out of Scope section. If an implementation task would naturally touch out-of-scope items, note the boundary in the Approach field.
- **Shared file conflict:** If two tasks would modify the same file and you mark them Parallel, you MUST add a dependency between them instead (or merge them into one task).

### 4b. Per-task fields

Each task record must contain these fields:

- **Task N: [Short descriptive name]** -- heading (2-6 words)
- **Files:** `path/to/file.ext` [Create | Modify | Delete] -- specific paths from the file target map. Multiple files on separate lines if needed.
- **Approach:** 2-5 sentences describing WHAT to change and WHY, including the test strategy. Describe the implementation change first, then how tests cover it. Right level of detail:
  - Too vague: "Update the frontend component and add tests"
  - Too detailed: "On line 47, change X to Y, then in test line 12 assert Z"
  - Right level: "Add a `filterByName` function to ProjectList.tsx that accepts a search string and filters the project array using case-insensitive substring matching. Wire it to the existing toolbar's search input. Write unit tests in ProjectList.test.tsx covering exact match, case-insensitive match, and empty string (returns all)."
- **Dependencies:** `None` (for Chapter 1 tasks) or `Task N, Task M` (for dependent tasks)
- **Parallel:** `Yes` (if task shares its chapter with 1 or more other tasks) or `No` (if it is the sole task in its chapter)
- **Requirements:** `REQ-N, REQ-M` references from SPEC.md (traceability; do not duplicate AC text)

### 4c. Dependency analysis

Identify dependencies between tasks using these dependency types:

- **File dependency:** Task B modifies a file that Task A creates -> B depends on A
- **Type dependency:** Task B uses a type, interface, or function defined in Task A -> B depends on A
- **Integration dependency:** Task B wires together outputs from Task A and Task C -> B depends on both A and C
- **Audit dependency:** A final audit task that verifies changes across all modified files -> depends on all individual fix tasks
- **Shared file conflict:** If Task A and Task B would both modify the same file, one must depend on the other (or they must be merged)

After listing all tasks, cross-check: for every pair of tasks that touch the same file and are marked Parallel, add a dependency between them.

### 4d. Chapter assignment

Assign chapters using this algorithm:

1. Tasks with no dependencies -> **Chapter 1**
2. Tasks whose dependencies are ALL in Chapter 1 or earlier -> **Chapter 2**
3. Tasks whose dependencies are ALL in Chapter 2 or earlier -> **Chapter 3**
4. Continue until all tasks are assigned a chapter
5. Mark each task **Parallel: Yes** if its chapter contains 2 or more tasks; **Parallel: No** if it is the sole task in its chapter (sequential bottleneck)

**Complexity flag:** If you detect more than 3 levels of dependency depth (Chapter 4 or deeper), add this note to the PLAN.md Summary section:

```
Note: This work item has 4+ levels of task dependency depth, which may indicate it is larger than ideal for a single planning unit. Consider splitting into two work items after implementation if the plan proves difficult to execute.
```

After assigning chapters, update the per-task `Dependencies` field references: tasks in Chapter 1 have `Dependencies: None`; tasks in later chapters list their prerequisite task numbers.

### 4d.5. Capture architectural decisions inline

After completing task decomposition and chapter assignment, identify any architectural decisions made during planning. These are decisions about HOW to implement, not WHAT to implement (the "what" is in SPEC.md).

**When to capture:** The plan chose a specific approach over alternatives (e.g., "modify existing service vs. create new one"), made a dependency ordering choice with trade-offs, or resolved an ambiguity in SPEC.md through an implementation choice.

For each decision, consult the `nuff-state-update` skill with `capture-decision`:

- **Class:** typically Architecture (component boundaries, API design, data flow) or Logic (validation approach, error handling strategy)
- **Decision:** What the plan chose
- **Rationale:** Why this approach (from codebase analysis findings, existing patterns, or complexity trade-offs)
- **Alternatives:** What other approach was considered
- **Source:** `plan — task decomposition`

These decisions are also reflected in PLAN.md's Summary section as prose. The structured entry is the machine-readable complement.

### 4e. Write PLAN.md to disk

Write the complete PLAN.md to `.nuff/<item-name>/PLAN.md` using the Write tool.

Use the canonical template in [references/plan-template.md](./references/plan-template.md). Keep the section order and headings from that reference, but replace every placeholder with the actual task names, files, approaches, dependencies, chapter assignments, and verification notes derived from Steps 4a-4d.

---

## Step 4.5: Self-Review via Gap Analyzer

After writing PLAN.md to disk, **spawn the `nuff-gap-analyzer` subagent consulting the `nuff-subagent-execution` skill in plan review mode** to check the plan quality before presenting it to the user.

Pass to the subagent:

- The PLAN.md content (just written in Step 4e)
- The SPEC.md content (loaded in Step 2a)
- The item name and `.nuff/<item-name>/` path

**Model selection:** Use `CONFIG_MODELS.gap_analyzer.plan` as the model for this subagent.

The gap analyzer will check:

- Requirement coverage (every REQ-N mapped to at least one task)
- Out-of-scope creep
- Approach vagueness
- Dependency correctness and shared file conflicts
- Missing edge case handling
- Standard pushback triggers (scope, contradictions, vague terms)

**Processing the gap report:**

**CRITICAL: You MUST run the gap analyzer and present its findings to the user.** Do not skip the gap analyzer. Do not silently resolve issues. The gap analyzer exists to catch problems that the coder workflow will not be able to resolve later. Every finding is a potential misalignment between the plan and user expectations.

1. **Structural fixes only** (missing dependency link, shared file conflict where the resolution is mechanically obvious): Apply the fix to PLAN.md on disk using the Edit tool. Print what was fixed: `Self-review: fixed [description].` These are limited to dependency/ordering corrections -- NOT scope, requirement coverage, or approach changes.

2. **All scope and content issues MUST go to the user** (uncovered requirements, scope concerns, contradictions, ambiguous approaches, missing edge case handling, requirement coverage gaps): Collect ALL of these as discussion points for the approval gate. Present each issue to the user and get input before proceeding (consult the `ask-user` skill). **Never silently add tasks for uncovered requirements, remove tasks, or change task scope without asking the user.**

   If the user resolves a gap finding that involves choosing between approaches, excluding scope, or making an architectural trade-off, consult `nuff-state-update` with `capture-decision` (same format as Step 4d.5, source: `plan — gap resolution`).

3. **Knowledge conflicts:** If the gap report includes a `### Knowledge Conflicts` section with conflicts found, present each conflict to the user before asking for plan approval. The user should explicitly decide whether to follow existing knowledge or override it. If overriding, capture the override as a decision via `nuff-state-update` `capture-decision` with class matching the knowledge category and source `plan — knowledge override`.

4. **No issues found**: Print `Self-review: plan looks clean.` and proceed.

After processing, if any fixes were applied, re-read the updated PLAN.md to confirm consistency.

### Step 4.6: Collect gap analyzer reflections

Consult the `nuff-reflection-capture` skill with:

- The full gap analyzer result text
- Source label: `Plan gap analysis (gap-analyzer)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

---

## Step 5: Approval Gate

### 5a. Announce file location

After writing PLAN.md to disk (and applying any self-review fixes), print:

```
PLAN.md written to .nuff/<item-name>/PLAN.md -- review it in your editor.
```

If the gap analyzer flagged issues requiring user input, you MUST present them now and resolve them before asking for plan approval:

```
**Plan review notes:**
- [Issue 1: description and question for user]
- [Issue 2: description and question for user]
```

Address **each** flagged issue individually with the user before proceeding to approval (consult the `ask-user` skill). Do NOT bundle gap findings into the approval question -- resolve them first, apply any resulting changes to PLAN.md, then ask for approval of the final plan.

### 5b. Ask for explicit approval

**Autonomous mode check:** If `autonomous_mode=true`:

- Print: `Autonomous mode: PLAN.md auto-approved -- proceeding to ADO tasks and commit.`
- Skip the approval gate entirely and proceed directly to Step 6.

If autonomous_mode is NOT set, ask the user (consult the `ask-user` skill):

- **Approved -- commit the plan** -- Commit PLAN.md and proceed to ADO task creation
- **Needs changes -- I'll describe** -- Tell me what to revise and I'll update the plan

### 5c. Revision loop

- If the user requests changes (selects "Needs changes" or describes revisions): apply the changes to `.nuff/<item-name>/PLAN.md` on disk using the Edit tool, then re-ask for approval (return to 5b).
- Repeat until the user selects the structured approval option from the `ask-user` skill.
- If the user sends freeform feedback instead of choosing an option, treat concrete revision requests as change input. Otherwise, re-ask with the structured approval options instead of inferring approval from phrases.
- **Never commit before receiving an explicit approval signal.**

---

## Step 6: ADO Child Tasks and Commit

Only proceed here after the user has explicitly approved PLAN.md.

### 6a. Create ADO child tasks (if applicable)

This step creates ADO Task work items for each task in PLAN.md. Skip this entire step if ANY of these conditions are true:

- STATE.md does not have a `work_item_id` field (no ADO work item associated)
- STATE.md `work_item_is_simple` is `true` (Task/Bug items do not get children)
- ADO is not configured (no `ado.project` in `CONFIG`)
- STATE.md does not have an `ado_process_template` block (process template detection failed)

If skipping, print: "Skipping ADO child task creation: [reason]." and proceed to 6b.

Consult the `nuff-ado-orchestration` skill with `create-child-tasks`, passing:

- the parsed PLAN task list and execution order
- the project from `CONFIG.ado.project`
- the parent work item ID from STATE.md
- the task type from `ado_process_template`
- `area_path` and `iteration_path` from STATE.md

The helper is responsible for creating and linking child tasks, recording the `ado_tasks` array through `nuff-state-update`, printing the success summary, and optionally posting the summary comment via `post-message`.

### 6b. Update STATE.md

Consult the `nuff-state-update` skill with `complete-plan`, passing:

- the current timestamp
- the parsed PLAN execution-order structure
- the actual task count and chapter count
- the fact that `PLAN.md` is now complete

### 6b.1. Sync progress visualization

Consult the `nuff-progress-sync` skill to update the runtime task list. This is an important sync point because the Code phase sub-tasks (chapters, tasks, verification, user approval) now appear in STATE.md for the first time.

### 6c. Git commit

Run these commands using the Bash tool:

```bash
git add .nuff/<item-name>/
git commit -m "nuff(<item-name>): plan complete -- PLAN.md approved"
```

The directory-level `git add` ensures both PLAN.md and the updated STATE.md are staged together. After committing, validate per [nuff-session-management references/git-validation.md](../nuff-session-management/references/git-validation.md).

After a successful commit, consult the `nuff-state-update` skill with `append-session-log` for:

- `phase-complete | plan`
- `artifact-committed | PLAN.md`

Then stage and amend the commit to include the session log update:

```bash
git add .nuff/<item-name>/STATE.md
git commit --amend --no-edit
```

### 6c.1. Post PLAN.md to ADO

Consult the `nuff-ado-orchestration` skill with `post-artifact-approval`, using:

- `artifact_label: Plan`
- `artifact_file: .nuff/<item-name>/PLAN.md`

### 6d. Report completion

Print:

```
Plan complete. PLAN.md committed at .nuff/<item-name>/PLAN.md. Ready for code phase.
```

---

## Important Constraints

- **Never commit before approval:** PLAN.md must be written to disk and explicitly approved by the user before running any `git` commands or creating ADO tasks.
- **ADO child task creation runs after plan approval and before git commit.** Failures are non-blocking -- log errors and continue. The ado_tasks array in STATE.md is consumed by the code workflow for state transitions.
- **Directory-level git add:** Always use `git add .nuff/<item-name>/` to capture both PLAN.md and STATE.md in one commit.
- **Codebase exploration is mandatory:** Every task's Files field must contain specific, verified file paths from the codebase analyzer subagent -- either confirmed by Glob/Grep (existing files) or derived from project conventions (new files, marked [Create]). Never write vague file targets.
- **Respect Out of Scope:** Do not create tasks for items listed in SPEC.md's Out of Scope section. If an implementation naturally touches an out-of-scope area, note the boundary in the Approach field.
- **No verification tasks:** Requirements that are pure verification (e.g., "no functional regression") go in Verification Notes, not as implementation tasks.
- **Tests belong inside the task that introduces the behavior.** Never create a separate task or chapter for tests. Every behavior-changing task lists both its source file(s) and its test file(s) in Files, and its Approach describes both the implementation and the test coverage.
- **Shared file conflict rule:** If two tasks would both modify the same file and you marked them Parallel: Yes, you MUST add a dependency between them (making one Parallel: No) or merge them.
- **Area path and iteration path for child tasks are inherited from the parent work item** (read from STATE.md area_path and iteration_path fields set by the refine workflow).
- **CRITICAL: Always run the gap analyzer and always ask the user about findings.** The gap analyzer (Step 4.5) is NOT optional. Its findings MUST be presented to the user (consult the `ask-user` skill) before plan approval. Never silently resolve requirement coverage gaps, scope questions, or approach ambiguities. These are scope decisions that belong to the user -- making them autonomously leads to plans that don't match user expectations, which wastes the code phase entirely.
