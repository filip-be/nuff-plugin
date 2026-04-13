---
name: nuff-workflow-refine
description: 'Orchestrate the refine phase of the nuff pipeline. Use when turning work-item or user input into an approved SPEC.md.'
---

# Refine Workflow

This skill is consulted by the `do.nuff` orchestrator during the refine phase. You ARE the orchestrator executing these steps. Do NOT delegate to a separate agent. When the workflow says to spawn a subagent, follow the **subagent model routing rule** from `PLATFORM` (see below) and consult the `nuff-subagent-execution` skill. On Copilot: NEVER invoke nuff agents directly — always use `@nuff-copilot-subagent-<alias>` wrappers.

## Available Context Variables

- `<item-name>` -- the work item name (from do.nuff context detection)
- `.nuff/<item-name>/STATE.md` -- path to the state file
- `CONFIG` -- user configuration from `.nuff/config.json`
- `CONFIG_MODELS` -- resolved model IDs from `CONFIG.models` (aliases expanded)
- `PLATFORM` -- `claude-code` or `copilot` (determines subagent spawning method)
- `user_instruction` -- plain text provided by the user as the argument to do.nuff (if any); `null` otherwise
- `autonomous_mode` -- true/false (from do.nuff invocation)

## Roles and Boundaries

- The orchestrator owns user interaction, artifact writes, STATE.md updates, and any git or ADO side effects.
- Subagents own deep repo analysis, draft generation, and gap analysis that would otherwise bloat the main workflow context.
- If a repeated mechanic is covered by a shared skill, prefer the shared skill over restating the procedure inline.

---

## Step 1: Read STATE.md

Read `.nuff/<item-name>/STATE.md` to get the current context (phase, work item ID, timestamps, existing artifacts).

Verify that the `phase` field is `refine`. If the phase is different, note this but proceed:

```
STATE.md shows phase '<current>', but refine workflow was invoked. Proceeding anyway -- STATE.md will be updated.
```

Also note any `work_item_id` field in the YAML frontmatter -- it will be needed in Step 2.

---

## Step 1.5: Check for Previous Draft

Check if `.nuff/<item-name>/SPEC.md` already exists:

1. **Committed SPEC.md exists:** This is a restart or re-run. Read it as a "previous draft" -- use it as starting context but allow the user to modify or override any part through the normal clarification flow. Print: `Found existing SPEC.md from previous session. Using as starting point.`

2. **Uncommitted SPEC.md exists:** This was interrupted mid-write. Run `git status --porcelain .nuff/<item-name>/SPEC.md` to check. If the file exists on disk but is untracked or modified, it was interrupted mid-write. Read it and note which sections are complete vs. incomplete (check for placeholder text, empty sections, or truncated content). Print: `Found a draft SPEC.md from a previous session (uncommitted). Using it as a starting point.`

3. **No SPEC.md exists:** Fresh start. Proceed normally.

If a previous draft exists (committed or uncommitted), skip input acquisition (Step 2) for any sections already complete in the draft. Jump to the clarification loop (Step 3) to let the user confirm or modify existing content.

**Autonomous mode:** If `autonomous_mode=true` and a previous draft exists, accept the existing draft and proceed directly to approval (Step 4) without clarification.

---

## Step 1.7: Load Knowledge Context

Check if `.nuff/knowledge/INDEX.md` exists using the Glob tool.

**If it exists:**

1. Read `.nuff/knowledge/INDEX.md` using the Read tool.
2. Store the content as `KNOWLEDGE_INDEX` for use in gap analysis.
3. Print: `Loaded knowledge index (<N> entries across <C> categories).` (count entries and category headings from the file).

**If it does not exist:**

- Set `KNOWLEDGE_INDEX = null`.
- This is normal for the first work item — no warning needed.

When spawning the gap analyzer in Step 3a, if `KNOWLEDGE_INDEX` is non-null, append it to the subagent input:

```
## Relevant Knowledge (from previous sessions)

<KNOWLEDGE_INDEX content>
```

The gap analyzer should consult these entries when assessing gaps — if a known pitfall or pattern is relevant to the current work item, it should reference it in its gap report. If specific knowledge entries seem highly relevant, the gap analyzer may Read the full knowledge file (at `.nuff/knowledge/<category>/<filename>.md`) for deeper context.

---

## Step 2: Acquire Input

### 2a. Determine input mode

Check for available input. Note that `work_item_id` and `user_instruction` are **independent** — both can be set at the same time if the user provided an ID followed by instruction text.

**Work item ID** — available if any of:

1. `work_item_id` was parsed from `$ARGUMENTS` by do.nuff
2. `work_item_id` field in STATE.md has a numeric value

**User instruction** — available if `user_instruction` is non-null (set by do.nuff from argument text).

Then check ADO configuration:

- ADO is **configured** if `CONFIG` has a non-empty `ado.project` value
- ADO is **not configured** if `ado.project` is missing or empty in `CONFIG`

**Mode selection:**

- Work item ID available **AND** ADO configured -> **ADO mode** (Step 2b); `user_instruction` (if set) supplements the work item data
- Otherwise -> **Manual mode** (Step 2c)

### 2b. ADO mode -- fetch via subagent

Consult the `nuff-ado-orchestration` skill with `fetch-work-item`, using:

- `work_item_id`: the numeric work item ID
- `project`: `CONFIG.ado.project`
- model alias `CONFIG_MODELS.ado_work_item_retriever`

The helper is responsible for spawning the retriever through `nuff-subagent-execution` and returning the normalized result.

**If status is `failed`:** Print `Warning: Could not fetch work item <id> from ADO. Falling back to manual mode.` and proceed to Step 2c as if no ADO input was available.

**If status is `success`:** Read the following fields from the result and store them for use in Steps 2d and 3:

- `title`, `type`, `state`, `work_item_is_simple`
- `area_path`, `iteration_path`, `tags`, `priority`, `parent_id`
- `description` (already stripped)
- `ac` (already stripped)
- `parent_context` (parent title, description, ac — if present in result)
- `comments` (if present in result)

If the result includes a Notes field with an unknown type warning, print it: `Warning: <note>.`

**Display summary block** (no blocking confirmation -- show and proceed immediately):

```
**Work Item Summary**
--------------------
**#<id>** <title>
**Type:** <type>
**State:** <state>
**Classification:** [Simple (work directly) | Parent (will decompose into child tasks)]

**Description:**
<description>

**Acceptance Criteria:**
<ac, or "None provided">

[If parent context present in result:]
**Parent Context: [#<parent_id>] <parent title>**
<parent description>
<parent ac if present>

[If user_instruction is set:]
**User Instruction:**
<user_instruction>

Starting clarification...
```

Use `Simple (work directly)` if `work_item_is_simple` is true, `Parent (will decompose into child tasks)` if false.

### 2c. Manual mode

Three sub-modes:

**If `user_instruction` is set:** The user provided free-form guidance or requirements as their argument. Treat it as the starting seed for the clarification — it may contain feature descriptions, constraints, requirements, or directional hints about how to proceed. Display it as a summary block:

```
**Work Item Summary (User Instruction)**
-----------------------------------------
**Instruction:**
<user_instruction>

Starting clarification...
```

Use the instruction text to seed the gap analyzer in Step 3a. The gap analyzer should treat it as the initial requirements input and identify what is clear, what is ambiguous, and what is missing — just as it would for an ADO work item description. Do NOT ask the user to re-describe what they already wrote; use their instruction as the basis and ask only about gaps.

**If no input was provided and ADO is not configured:** Start guided prompts. Ask the user these questions one at a time (consult the `ask-user` skill):

1. "What should this work item accomplish? (Describe the feature, fix, or change you want to make)"
2. "Who is the primary user or system affected by this change?"
3. "What does success look like? How would you know this is done correctly?"

Collect all three answers, then display a summary block:

```
**Work Item Summary (Guided Input)**
-------------------------------------
**Goal:** <answer to question 1>
**Primary User/System:** <answer to question 2>
**Success Criteria:** <answer to question 3>

Starting clarification...
```

### 2d. Update STATE.md with work item details

After acquiring input (from either ADO or manual mode), consult the `nuff-state-update` skill with `update-frontmatter` so STATE.md reflects the current work item context even if the session is interrupted before refine completes.

**ADO mode updates** (all values from the subagent result):

- `work_item_id:` -> the fetched work item ID
- `work_item_url:` -> construct as `https://dev.azure.com/<organization-name>/<project>/_workitems/edit/<id>` (use `CONFIG.ado.project`)
- `work_item_title:` -> `title` from subagent result
- `work_item_type:` -> `type` from subagent result
- `work_item_is_simple:` -> `work_item_is_simple` from subagent result
- `area_path:` -> `area_path` from subagent result
- `iteration_path:` -> `iteration_path` from subagent result

**Manual mode updates:**

- `work_item_title:` -> derive a short title from the user instruction or guided input answers (first ~60 chars, or a concise summary). Add this field if it doesn't exist.
- `user_instruction:` -> store the `user_instruction` value if set (truncate to 200 chars if very long). Omit the field if null.
- Leave `work_item_id` and `work_item_url` empty if no ADO ID was provided.

Also refresh the `updated:` timestamp as part of the same frontmatter update.

---

## Step 3: Clarification Loop

### 3a. Gap analysis (via subagent)

**Spawn the `nuff-gap-analyzer` subagent consulting the `nuff-subagent-execution` skill.** This keeps the repo scanning context (AGENTS.md, README.md, project structure, source files) out of the main context window.

Spawn the subagent passing:

- The work item summary block (from Step 2b or 2c), which already includes the `user_instruction` section if set
- The item name and `.nuff/<item-name>/` path
- If `user_instruction` is set: explicitly note it as **User Instruction** so the gap analyzer treats it as additional requirements/constraints that must be respected alongside the work item content

**Model selection:** Use `CONFIG_MODELS.gap_analyzer.refine` as the model for this subagent.

The subagent will:

1. Read repo docs (AGENTS.md, README.md, project structure, relevant source files)
2. Analyze the work item input against the gap taxonomy (scope boundaries, AC completeness, edge cases, ambiguous terms, user roles, error handling, data shapes)
3. Check for pushback triggers (scope too broad, contradictions, vague terms, missing edge cases)
4. Return a structured **Gap Report** with identified gaps and triggered pushbacks

Wait for the subagent to return the gap report before proceeding.

### 3a.5. Collect gap analyzer reflections

Consult the `nuff-reflection-capture` skill with:

- The full gap analyzer result text
- Source label: `Refine gap analysis (gap-analyzer)`
- The current item name and timestamp

If the result has no usable `### Reflections:` section, the reflection skill will skip silently.

### 3b. Process gap report and ask questions

Use the gap report returned by the subagent to drive the clarification loop. For each identified gap or pushback trigger, ask the user (consult the `ask-user` skill).

**Pushback handling:** Present pushback triggers before or alongside related clarifying questions. Use collaborative, suggestive tone -- not assertive.

**Override handling:** If the user says "accept it anyway", "that's fine", "proceed as-is", or similar:

- Note the risk in your working context as: `Acknowledged Risk: [description]`
- Do NOT raise the same concern again
- Move on immediately

### 3b.5. Capture scope decisions inline

After the user resolves a gap or pushback trigger that involves a scope, architecture, or logic choice (not just providing clarifying information), consult the `nuff-state-update` skill with `capture-decision`.

**When to capture:** The user chose between alternatives ("use X instead of Y"), accepted a risk ("proceed without Z"), excluded something ("that's out of scope"), or made an edge case handling choice. Informational answers ("the field is a string") are NOT decisions.

**What to capture:**

- **Class:** Architecture (technology/design choices), Logic (business rules, edge case handling), or Scope (exclusions, trade-offs)
- **Decision:** The choice the user made
- **Rationale:** Why (from user's words or the context of the gap)
- **Alternatives:** What was the other option(s) from the gap report
- **Source:** `refine — gap clarification`

Decisions are also still written into SPEC.md's Scope Decisions section as prose (for human readability). The structured `capture-decision` entry is the machine-readable complement.

**Knowledge conflicts:** If the gap report includes a `### Knowledge Conflicts` section with conflicts found, present each conflict to the user alongside the regular gap findings. Knowledge conflicts represent precedents from previous work items — the user should explicitly acknowledge overriding a known pattern or convention. Use the `ask-user` skill with options:

- **Follow knowledge** — adjust the spec to align with the existing knowledge entry
- **Override** — proceed as-is; the knowledge entry may need updating during improve phase
- **Discuss** — I need more context before deciding

If the user overrides a knowledge entry, capture the override as a decision via `capture-decision` with source `refine — knowledge override`.

### 3c. Adaptive question batching

Ask user questions based on the gap report (consult the `ask-user` skill). Map options to the tool's options array. Group related questions together when batching. Scope-related questions go together, error handling questions go together, etc.

### 3d. Confidence threshold

After each round of answers, self-assess with this check:

> "Can I write a complete SPEC.md with testable acceptance criteria for every stated requirement, clear scope boundaries, and at least the 2 most important edge cases handled?"

- If **yes**: Announce "I have enough to write the spec." and ask user for final confirmation (consult the `ask-user` skill), then proceed to Step 4 if user accepts.
- If the user explicitly says "that's enough", "proceed", "write the spec", "stop asking", or equivalent: **Stop immediately** and proceed to Step 4.
- If **no**: Continue with the next highest-priority gap.

**CRITICAL: You MUST run the gap analyzer and ask user questions.** Do not skip or shortcut the clarification loop. Even if the work item description seems detailed, the gap analyzer will identify gaps that are not obvious from the description alone. You MUST:

- Always spawn the gap analyzer subagent (Step 3a) -- never skip it
- Always ask the user about identified gaps (Step 3b) -- never silently resolve gaps yourself
- Never make scope decisions on the user's behalf -- if the gap report identifies scope concerns, ambiguities, or missing details, ask the user
- Never assume you know what the user wants for edge cases, error handling, or scope boundaries -- ask

**Important constraints:**

- Do not ask questions you already know the answers to from the work item content
- Do not repeat questions that have already been answered
- Stop asking when confidence threshold is met -- do not continue for completeness
- Every requirement in the final SPEC.md must be testable -- if a user's answer still produces a vague requirement, flag it once before finalizing: "I notice requirement [N] is still a bit vague. Can we make it more specific?" Then accept the user's response and proceed.

---

## Step 3.5: Documentation Impact Assessment (via Explore subagent)

After the clarification loop closes (confidence threshold met or user said proceed), assess whether this work item requires documentation changes before writing SPEC.md. **Spawn an Explore subagent** to keep documentation file contents out of the main context window.

Pass the Explore subagent:

- The work item summary block (from Step 2b or 2c)
- Key clarification decisions from the loop (Step 3)
- The item name

The Explore subagent should:

1. Find all human-facing documentation files using Glob: `README.md`, `docs/**/*.md`, `AGENTS.md`, `CONTRIBUTING.md`, `wiki/**/*.md`, and any other `.md` files outside `.nuff/` and `node_modules/`.
2. Read each file found.
3. Assess for each file: does this work item add, remove, or change user-facing behavior, APIs, configuration, or architectural concepts described in the file? Would a reader encounter stale or missing information after the work item ships?
4. Return a structured documentation impact list:
   - **Needs update:** `<file path>` — one sentence on what needs changing and why
   - **No update needed:** `<file path>` — brief reason (e.g., "internal refactor with no user-facing change")
   - **No documentation found:** note this if no doc files exist

Wait for the Explore subagent to return. Store the result as `DOC_IMPACT` — this feeds directly into the `## Documentation Impact` section of SPEC.md.

Do NOT ask the user about documentation impact unless the assessment is genuinely ambiguous — make a call and record it.

---

## Step 4: Generate and Approve SPEC.md

### 4a. Write SPEC.md draft to disk

Generate the complete SPEC.md content and **write it to disk** at `.nuff/<item-name>/SPEC.md`. This allows the user to review the file in their editor.

Use the canonical template in [references/spec-template.md](./references/spec-template.md). Keep the section order and headings from that reference, but replace every placeholder with concrete content from the current work item.

### 4b. Ask user for approval

**Autonomous mode check:** If `autonomous_mode=true`:

- Print: `Autonomous mode: SPEC.md auto-approved -- proceeding to commit.`
- Skip the approval gate entirely and proceed directly to Step 5.

If autonomous_mode is NOT set, proceed with the interactive approval flow below.

After writing SPEC.md to disk, tell the user the file path so they can review it in their editor. Then ask the user (consult the `ask-user` skill):

```
SPEC.md written to .nuff/<item-name>/SPEC.md -- review it in your editor. Ready to commit?

Options:
- Approved: "Commit the spec as-is"
- Needs changes: "I'll describe what to revise"
```

### 4c. Revision loop

- If the user requests changes (selects "Needs changes" or describes issues): apply the revisions to the file on disk using the Edit tool, then ask for approval again.
- Repeat until the user selects the structured approval option from the `ask-user` skill.
- If the user sends freeform feedback instead of choosing an option, treat concrete revision requests as change input. Otherwise, re-ask with the structured approval options instead of inferring approval from phrases.

---

## Step 5: Commit and Update State

Only proceed here after the user has explicitly approved the SPEC.md.

### 5a. SPEC.md already on disk

SPEC.md was written in Step 4a and revised in-place during 4c if needed. No additional write required here.

### 5b. Update STATE.md

Consult the `nuff-state-update` skill with `complete-refine`, passing the current timestamp and the fact that `SPEC.md` is now complete.

### 5b.1. Sync progress visualization

Consult the `nuff-progress-sync` skill to update the runtime task list with the new phase status.

### 5c. Git commit

Run these commands using the Bash tool:

```bash
git add .nuff/<item-name>/
git commit -m "nuff(<item-name>): refine complete -- SPEC.md approved"
```

The directory-level `git add` ensures both SPEC.md and the updated STATE.md are staged together. After committing, validate per [nuff-session-management references/git-validation.md](../nuff-session-management/references/git-validation.md).

After a successful commit, consult the `nuff-state-update` skill with `append-session-log` for:

- `phase-complete | refine`
- `artifact-committed | SPEC.md`

Then stage and amend the commit to include the session log update:

```bash
git add .nuff/<item-name>/STATE.md
git commit --amend --no-edit
```

### 5c.1. Post SPEC.md to ADO

Consult the `nuff-ado-orchestration` skill with `post-artifact-approval`, using:

- `artifact_label: Specification`
- `artifact_file: .nuff/<item-name>/SPEC.md`

### 5d. Report completion

Print:

```
Refine complete. SPEC.md committed at .nuff/<item-name>/SPEC.md. Ready for plan phase.
```

---

## Important Constraints

- **Always write SPEC.md to disk before asking for approval.** Write the draft to `.nuff/<item-name>/SPEC.md` first so the user can review in their editor. Revise in-place if changes requested. Only commit after explicit approval.
- **Never block on ADO summary confirmation.** Show the summary block and immediately proceed to gap analysis -- do not ask "does this look right?" before starting.
- **If pushback is overridden, do not re-raise.** Note the risk once, then move on.
- **Stop asking when confidence threshold is met.** Once you can write a complete, testable spec, announce it and proceed -- do not continue for completeness.
- **Every requirement must be testable.** Flag vague requirements once before finalizing the spec. If the user's clarification still leaves ambiguity, accept it and note it in Scope Decisions.
- **Commit both files together.** Use `git add .nuff/<item-name>/` (directory-level) to always include both SPEC.md and STATE.md in the same commit.
- **This workflow reads from ADO but does not write back.** ADO child task creation is handled by the plan workflow; state transitions are handled by the code workflow.
- **CRITICAL: Always run the gap analyzer and always ask the user about findings.** The gap analyzer (Step 3a) is NOT optional. Its findings MUST be presented to the user (consult the `ask-user` skill, Step 3b). Never silently resolve scope questions, edge case decisions, or ambiguities. These decisions belong to the user -- making them autonomously leads to specs that don't match user expectations, which cascades into wrong plans and wasted implementation effort.
