---
name: nuff-workflow-improve
description: "Orchestrate the improve phase of the nuff pipeline. Use when applying approved lessons to AGENTS.md and maintaining .nuff/knowledge/."
---

# Improve Workflow

This skill is consulted by the `do.nuff` orchestrator during the improve phase.

**Goal:** Review lessons learned (user corrections in `IMPROVEMENTS.md`), agent reflections (`REFLECTIONS.md`), and session decisions from artifacts (`SPEC.md`, `PLAN.md`, `STATE.md`). Persist approved improvements to AGENTS.md, knowledge files in `.nuff/knowledge/`, and ADRs where appropriate. Validate existing knowledge against session findings.

This workflow runs after `code` phase completes (or can be run standalone via `/improve.nuff`).

## Roles and Boundaries

- The orchestrator owns approval flows, AGENTS.md edits, knowledge file writes, STATE.md updates, and git commits.
- Shared skills own reusable mechanics such as knowledge file format rules or question handling.
- This workflow should stay focused on sequencing decisions; reusable write rules belong in helper skills.

## Workflow Shape

- Improvement path: discover AGENTS.md files, parse `IMPROVEMENTS.md`, draft rules, and apply approved updates.
- Knowledge path: parse `REFLECTIONS.md`, turn reusable learnings into knowledge candidates, validate contradictions, and regenerate the knowledge index.
- Decision path: validate pre-captured decisions from `STATE.md`'s `## Key Decisions`, catch any missed decisions from `SPEC.md`/`PLAN.md` prose, persist as `decisions` knowledge entries, and propose ADRs when appropriate.

---

## Step 1: Check for session inputs

Look for `.nuff/<item-name>/IMPROVEMENTS.md`, `.nuff/<item-name>/REFLECTIONS.md`, `.nuff/<item-name>/SPEC.md`, and `.nuff/<item-name>/PLAN.md`.

**IMPROVEMENTS.md:** If the file exists and has `##` entry headings, count them and store as `has_improvements = true`. Otherwise `has_improvements = false`.

**REFLECTIONS.md:** If the file exists and has `##` entry headings, count them and store as `has_reflections = true`. Otherwise `has_reflections = false`.

**SPEC.md / PLAN.md:** If either file exists, store `has_artifacts = true`. Otherwise `has_artifacts = false`.

If **all three** flags are false:

- Log `skip | process-improvements — no IMPROVEMENTS.md found`, `skip | process-reflections — no REFLECTIONS.md found`, and `skip | extract-decisions — no SPEC.md or PLAN.md found` via `nuff-state-update` `log-skip`.
- Print: `No improvements, reflections, or session artifacts found for <item-name>. Nothing to process.`
- Consult the `nuff-state-update` skill with `complete-improve`, then append the corresponding `improve-complete` and `phase-complete | improve` session-log entries through `append-session-log`, and commit.
- Exit.

If any flag is true, print a summary:

```
Found <N> improvement(s), <M> reflection(s), and <A> artifact(s) for <item-name>. Processing...
```

## Step 1.5: Parse REFLECTIONS.md

If `has_reflections = true`:

Read `.nuff/<item-name>/REFLECTIONS.md`. Parse each `## <source> — <datetime>` section into a structured entry:

- `source` — the agent and phase context (e.g., "Task 3: add-filter (implementer)", "Verification iteration 1 (verifier)")
- `datetime` — the ISO-8601 timestamp
- `content` — the reflection body (the bullet points under the heading)

Collect all entries as `REFLECTIONS_LIST`.

If `has_reflections = false`: set `REFLECTIONS_LIST = null`.

## Step 1.7: Validate and Promote Session Decisions (via Explore subagent)

Skip this step if `has_artifacts = false`. Set `DECISION_CANDIDATES = null` and `ADR_PROPOSALS = null`.

If `has_artifacts = true`, **spawn an Explore subagent** per [references/decision-validation.md](./references/decision-validation.md), passing paths to existing artifacts (SPEC.md, PLAN.md, STATE.md), the knowledge INDEX.md path, and the item name.

The subagent's primary role is **validation and promotion**, not extraction:

1. Read `## Key Decisions` from STATE.md — these are **pre-captured** structured decisions (D-1, D-2, ...) recorded inline during refine and plan phases
2. Cross-reference against SPEC.md `## Scope Decisions` and PLAN.md `## Summary` — identify any decisions in prose that lack a corresponding D-N entry (missed inline captures)
3. For newly extracted decisions (from prose gaps), apply the standard taxonomy (Architecture / Logic / Scope) and generalizability assessment
4. Check all decisions (pre-captured + newly extracted) against `.nuff/knowledge/INDEX.md` for duplicates/updates/confirmations
5. Return: validated decisions, newly extracted decisions (if any), and ADR proposals

This makes the improve phase a **safety net** rather than the primary capture mechanism. If inline capture during refine/plan worked perfectly, this step finds nothing new. If some decisions were missed, it still catches them.

Wait for the Explore subagent to return. Store new decision candidates as `DECISION_CANDIDATES` and ADR proposals as `ADR_PROPOSALS`. Print each skipped decision: `Decision "<summary>" is item-specific — skipping.`

Log `decision-validated | <N> pre-captured, <M> newly extracted from <artifacts>` via `nuff-state-update` `append-session-log`.

**Parallelism note:** This Explore subagent can be spawned simultaneously with the Step 3.5 and Step 4 Explore subagents since they operate on independent data. Await all three before proceeding to Step 5.

---

## Step 2: Discover AGENTS.md Files

Use the Glob tool to find all `AGENTS.md` files in the repository:

```
**\/AGENTS.md
```

Collect the list as `AGENTS_FILES`. If none are found:

- Log `skip | agents-update — no AGENTS.md files found in repository` via `nuff-state-update` `log-skip`.
- Print: `Warning: No AGENTS.md files found in the repository. User improvements will have nowhere to land.`

If `has_improvements = true` and no AGENTS.md files exist, skip Steps 3-6 (AGENTS.md processing) but still proceed with Steps 3.5, 5.5, and 6.5 (reflection/knowledge processing) if `has_reflections = true`.

## Step 3: Parse Improvement Entries

Skip this step if `has_improvements = false`.

Read `.nuff/<item-name>/IMPROVEMENTS.md`. Parse each `## <datetime> — <title>` section into a structured entry:

- `datetime` — the ISO-8601 timestamp
- `title` — the short title
- `what_missed` — content of **What was missed / done incorrectly**
- `expects_instead` — content of **What the user expects instead**
- `trigger` — content of **Context / trigger**
- `category` — the category tag

Collect all entries as `IMPROVEMENTS_LIST`.

## Step 3.5: Process Reflections into Knowledge Candidates (via Explore subagent)

Skip this step if `REFLECTIONS_LIST` is null.

**Spawn an Explore subagent** to process reflections against the knowledge base. This keeps knowledge file contents out of the orchestrator's context window.

Pass the Explore subagent:

- `REFLECTIONS_LIST` — the parsed reflections from Step 1.5 (source, datetime, content for each)
- Path to `.nuff/knowledge/INDEX.md`
- The knowledge file format, categories (`patterns`, `pitfalls`, `conventions`, `tooling`), and conventions from the `nuff-knowledge` skill

The Explore subagent should:

1. **Assess generalizability:** For each reflection, determine if the insight applies beyond this specific task/item. Skip item-specific reflections (unique discoveries unlikely to recur). Note skipped reflections with reason.
2. **Check for duplicates:** Read `.nuff/knowledge/INDEX.md` if it exists. For each generalizable reflection, search for entries with matching tags or similar titles. If a match exists, read the full knowledge file. Determine if the reflection adds new information or merely confirms existing knowledge.
3. **Create knowledge candidates:** For each new generalizable reflection with no match, propose: category, title (5-8 words, imperative), draft content, tags (2-4 keywords), and file path (`.nuff/knowledge/<category>/<kebab-case-title>.md`).
4. Return a structured result with three sections:
   - **Skipped:** reflection source + reason (item-specific)
   - **Updates:** existing knowledge ID + path + what new information to add
   - **New candidates:** category, title, draft content, tags, proposed file path

Wait for the Explore subagent to return.

- Print each skipped reflection: `Reflection "<source>" is item-specific — skipping.`
- Print each update: `Updated existing knowledge [<id>] with new context.`
- Print each confirmation: `Reflection confirms existing knowledge [<id>] — skipping.`
- Store new candidates as `KNOWLEDGE_CANDIDATES` for presentation in Step 5.5.
- Store updates as `KNOWLEDGE_UPDATES` for application during Step 5.5.

**Parallelism note:** This Explore subagent can be spawned simultaneously with the Step 4 Explore subagent (see below) since they operate on independent data (knowledge files vs. AGENTS.md files). Await both before proceeding to Step 5.

---

## Step 4: Match Improvements to AGENTS.md Files (via Explore subagent)

**Spawn an Explore subagent** to match each improvement to the right AGENTS.md file and section. This keeps AGENTS.md file contents out of the orchestrator's context window.

Pass the Explore subagent:

- `IMPROVEMENTS_LIST` — the parsed improvements from Step 3 (datetime, title, what_missed, expects_instead, trigger, category for each)
- `AGENTS_FILES` — the list of AGENTS.md paths discovered in Step 2
- The scope mapping table (below)

**Scope mapping table:**

| Category / Content Signal                                           | Likely scope                                                         |
| ------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `workflow` — references a nuff phase (refine, plan, code)           | Workflow file for that phase; or nuff plugin AGENTS.md if one exists |
| `command` — references a CLI tool or shell command used during code | `nuff-implementer.md` agent                                          |
| `tool-use` — references which tool or agent was invoked             | Workflow or orchestrator                                             |
| `style` — naming, formatting, code style                            | `nuff-implementer.md` or project root AGENTS.md                      |
| `content` — spec/plan content quality                               | Relevant workflow or root AGENTS.md                                  |
| `other` — generic behavioural rule                                  | Root AGENTS.md (repo-wide)                                           |

The Explore subagent should:

1. **Determine scope** for each improvement using category/content and the scope mapping table.
2. **Read candidate files:** Read the top 1-2 most likely AGENTS.md files. Find existing sections covering the same domain (e.g., "Commands", "Implementation", "Style"). Determine if the improvement fits as a new bullet in an existing section or needs a new section.
3. **Draft proposed addition:** Write a concise rule (1-2 sentences, present-tense imperative). Format as `- <Rule>` for an existing section, or `## <Section Title>\n\n- <Rule>` for a new section.
4. **Check for duplicates:** Keyword-scan the target file for near-identical rules. Flag duplicates.
5. Return a structured list, one entry per improvement: title, target AGENTS.md path, section name (or `new section: <name>`), proposed text, and duplicate flag.

Wait for the Explore subagent to return. Store the result as `IMPROVEMENT_MAPPINGS` for presentation in Step 5.

**Parallelism note:** This Explore subagent can be spawned simultaneously with the Step 3.5 Explore subagent since they operate on independent data. Await both before proceeding to Step 5.

## Step 5: Present Improvements One by One

Use `IMPROVEMENT_MAPPINGS` from Step 4 to present each improvement with its pre-matched target and drafted text. For each improvement entry (in order), present it to the user and ask for approval. If the mapping has `duplicate: true`, skip automatically and print: `Skipped (rule already present): <target path>`.

### 5.1 Display the improvement

Print a block like:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Improvement [N/total]: <title>
Category: <category>
Recorded: <datetime>

What was missed:
  <what_missed>

Proposed addition to: <target AGENTS.md path>
Section: <section name or "new section: <name>">

  <proposed addition text>
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 5.2 Ask for approval

Consult the `ask-user` skill with these options:

- **Apply** — write this addition to the target AGENTS.md
- **Edit** — I want to adjust the text before applying (prompts for revised text)
- **Skip** — do not apply this improvement
- **Change target** — apply to a different AGENTS.md file

### 5.3 Handle responses

**Apply:**

- Apply the proposed text to the target AGENTS.md (see Step 6 for write rules).
- Print: `Applied to <target path>.`

**Edit:**

- Ask: "Enter the revised text for the AGENTS.md addition:"
- Use the provided text instead of the draft.
- Apply to the target AGENTS.md.
- Print: `Applied (edited) to <target path>.`

**Skip:**

- Print: `Skipped.`
- Move to the next improvement.

**Change target:**

- List available AGENTS.md files with their paths.
- Ask the user to select one.
- Regenerate the proposed addition in the context of the chosen file (re-read that file to find the right section).
- Loop back to Step 5.1 with the same improvement and new target.

### 5.4 Autonomous mode

When `autonomous_mode = true`:

- Auto-apply all improvements using the best-effort target selected in Step 4.
- Log each decision: `[AUTONOMOUS] Applied improvement "<title>" to <target path>.`
- Never skip or ask for confirmation.

## Step 5.5: Present Knowledge Candidates

Merge `DECISION_CANDIDATES` into `KNOWLEDGE_CANDIDATES` (decision candidates appear after reflection-based candidates). Skip this step if the merged list is empty or null.

Ensure the knowledge directory structure exists:

```bash
mkdir -p .nuff/knowledge/patterns .nuff/knowledge/pitfalls .nuff/knowledge/conventions .nuff/knowledge/tooling .nuff/knowledge/decisions
```

For each knowledge candidate (in order):

### 5.5.1 Display the candidate

Print a block like:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Knowledge candidate [N/total]: <title>
Source: <agent name> reflection from <task/phase>
Category: <category>
Tags: <tags>

Learning:
  <proposed learning text>

Proposed file: <proposed file path>
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 5.5.2 Ask for approval

Consult the `ask-user` skill with these options:

- **Save** — write this knowledge file
- **Edit** — adjust the content before saving (prompts for revised text)
- **Skip** — discard this candidate
- **Also add to AGENTS.md** — save as knowledge AND add a concise rule to AGENTS.md

### 5.5.3 Handle responses

**Save:**

- Write the knowledge file to the proposed path using the Write tool.
- Print: `Saved to <file path>.`

**Edit:**

- Ask: "Enter the revised learning text:"
- Use the provided text to update the draft content.
- Write the knowledge file.
- Print: `Saved (edited) to <file path>.`

**Skip:**

- Print: `Skipped.`
- Move to the next candidate.

**Also add to AGENTS.md:**

- Write the knowledge file first (with a `## Cross-references` section noting the AGENTS.md target).
- Then follow Steps 4-6 (Match to AGENTS.md, Present, Write Rules) to add a concise rule to the appropriate AGENTS.md file. The AGENTS.md rule should be a one-line summary; the knowledge file holds the full context.
- Print: `Saved to <file path> and added rule to <AGENTS.md path>.`

### 5.5.4 Autonomous mode

When `autonomous_mode = true`:

- Auto-save all knowledge candidates with `confidence: medium`.
- Log each decision: `[AUTONOMOUS] Saved knowledge "<title>" to <path>.`
- Never skip or ask for confirmation.

## Step 5.7: Present ADR Proposals

Skip this step if `ADR_PROPOSALS` is empty or null. For each ADR proposal, display and ask for approval using the templates and options defined in [references/decision-validation.md](./references/decision-validation.md). Options: **Create ADR** (write ADR file + decisions knowledge entry), **Edit**, **Save as knowledge only**, **Skip**. In autonomous mode, auto-save as knowledge entries only — ADRs warrant human review. Log each: `[AUTONOMOUS] Saved ADR proposal "<title>" as knowledge entry.`

If any ADR is created, log `adr-created | <path>` via `nuff-state-update` `append-session-log`.

---

## Step 6: Write Rules for AGENTS.md

When applying an improvement to an AGENTS.md file:

### 6.1 Adding to an existing section

Use the Edit tool to insert the new bullet at the end of the matching section, before the next `##` heading (or end of file). Preserve existing indentation and list style.

### 6.2 Adding a new section

Use the Edit tool to append the new section at the end of the file, with a blank line before the `##` heading.

### 6.3 Never duplicate

Before writing, check if an equivalent rule already exists in the file (keyword scan). If a near-identical rule is found, skip the write and print:

```
Skipped (rule already present): <target path>
```

## Step 6.5: Validate Existing Knowledge

Skip this step if `.nuff/knowledge/INDEX.md` does not exist or if `REFLECTIONS_LIST` is null.

Cross-reference session reflections against existing knowledge entries:

### 6.5.1 Contradiction check (discovery via Explore subagent)

**Spawn an Explore subagent** to cross-reference reflections against existing knowledge. This keeps knowledge file contents out of the orchestrator's context window.

Pass the Explore subagent:

- `REFLECTIONS_LIST` — the parsed reflections
- Path to `.nuff/knowledge/INDEX.md`

The Explore subagent should:

1. Read `.nuff/knowledge/INDEX.md`. For each reflection, check if it contradicts or confirms any existing knowledge entry (match by tags, titles, and topic overlap).
2. For potential contradictions (reflection describes a different behavior, outcome, or recommendation than the knowledge file), read the full knowledge file and extract the contradicting content.
3. For confirmations (reflection describes the same issue or pattern), note the match.
4. Return a structured result:
   - **Contradictions:** knowledge ID, title, path, excerpt, reflection source, reflection content
   - **Confirmations:** knowledge ID, title

Wait for the Explore subagent to return.

**Process contradictions (orchestrator):** For each contradiction, present both the existing knowledge and the contradicting reflection to the user. Consult the `ask-user` skill with these options:

- **Update knowledge** — revise the existing file with new information from the reflection
- **Remove knowledge** — remove from INDEX.md and remove the file
- **Keep as-is** — the contradiction may be context-dependent; preserve both

**Process confirmations:** For each confirmation, print: `Confirmed knowledge [<id>]: "<title>" — no changes.`

### 6.5.2 Regenerate INDEX.md

After all knowledge file operations in this workflow (creation in Step 5.5, updates/removals in Step 6.5.1):

1. Use Glob to find all `.nuff/knowledge/**/*.md` files (excluding `INDEX.md`).
2. Read each file's YAML frontmatter to extract: title, category, tags.
3. Rebuild `.nuff/knowledge/INDEX.md` from scratch following the format defined in the `nuff-knowledge` skill.
4. Write the regenerated INDEX.md using the Write tool.

If no knowledge files exist, write a minimal INDEX.md:

```markdown
# Knowledge Index

No knowledge entries yet. Knowledge accumulates as the nuff workflow processes reflections from agent sessions.
```

---

## Step 7: Commit Changes

After all improvements and knowledge candidates have been processed:

### 7a. Commit AGENTS.md changes

Collect all AGENTS.md files that were modified. If any were modified:

```bash
git add <list of modified AGENTS.md paths>
git commit -m "nuff(<item-name>): apply lessons learned to AGENTS.md"
```

Print: `Committed AGENTS.md updates.`

If no AGENTS.md files were modified (all skipped): print `No AGENTS.md changes to commit.`

### 7b. Commit knowledge files

If any knowledge files were created, updated, or archived during Steps 5.5, 5.7, and 6.5:

```bash
git add .nuff/knowledge/
git commit -m "nuff(<item-name>): update knowledge base (<N> new, <M> updated, <K> removed, <D> decisions)"
```

Print: `Committed knowledge base updates.`

If no knowledge files were changed: print `No knowledge changes to commit.`

### 7c. Ensure root AGENTS.md references knowledge base

This step ensures that ALL AI sessions in this repository (not just nuff) can discover and use accumulated knowledge.

Skip this step if no knowledge files exist (`.nuff/knowledge/INDEX.md` does not exist or is the empty placeholder).

1. Find the root `AGENTS.md` file (repository root). If it does not exist print warning and skip.
2. Check if the file already contains a reference to `.nuff/knowledge/` (search for the string `.nuff/knowledge`).
3. If no reference found, append the following section using the Edit tool:

```markdown
## Knowledge Base

This repository accumulates learnings from previous AI agent sessions in `.nuff/knowledge/`. Before starting work, read `.nuff/knowledge/INDEX.md` for a summary of known patterns, pitfalls, conventions, tooling discoveries, and application-level decisions relevant to this codebase.
```

4. If the section was added, include the root AGENTS.md in the Step 7a commit (or create a separate commit if 7a already ran):

```bash
git add AGENTS.md
git commit -m "nuff(<item-name>): add knowledge base reference to AGENTS.md"
```

Print: `Added knowledge base reference to root AGENTS.md.`

If the reference already exists skip this step.

### 7d. Commit ADR files

If any ADR files were created during Step 5.7:

```bash
git add <ADR directory path>
git commit -m "nuff(<item-name>): add ADR(s) from session decisions"
```

Print: `Committed ADR file(s).`

If no ADR files were created: skip silently.

## Step 8: Update STATE.md and Final Commit

Consult the `nuff-state-update` skill with `complete-improve`, passing the current timestamp.

Then consult `nuff-state-update` with `append-session-log` for:

- `improve-complete | <N applied>/<total> improvements, <K new>/<L total> knowledge entries (<D> decisions), <A> ADR(s)`
- `phase-complete | improve`

### 8.5.1 Sync progress visualization

Consult the `nuff-progress-sync` skill to sync the final status (Improve complete).

### 8.6 Commit

```bash
git add .nuff/<item-name>/
git commit -m "nuff(<item-name>): improve complete"
```

After committing, validate per [nuff-session-management references/git-validation.md](../nuff-session-management/references/git-validation.md).

Print:

```
Improve phase done. <N applied>/<total> improvements applied to AGENTS.md. <K> knowledge entries created/updated (<D> decisions). <A> ADR(s) proposed.
```
