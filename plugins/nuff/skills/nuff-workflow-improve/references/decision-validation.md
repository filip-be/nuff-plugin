# Decision Validation — Explore Subagent Instructions

This reference defines how the Explore subagent validates pre-captured decisions from STATE.md, catches any missed decisions from artifact prose, and proposes knowledge entries and ADRs.

## Inputs

- Paths to session artifacts: `SPEC.md`, `PLAN.md`, `STATE.md` (whichever exist in `.nuff/<item-name>/`)
- Path to `.nuff/knowledge/INDEX.md` (may not exist)
- The item name

## Procedure

### 1. Read pre-captured decisions from STATE.md

Read the `## Key Decisions` section from STATE.md. Parse each `### D-N:` entry into structured format:

- `id` — D-1, D-2, etc.
- `title` — the short title
- `class` — Architecture | Logic | Scope
- `decision` — what was decided
- `rationale` — why this option was chosen
- `alternatives` — what was considered and rejected
- `source` — which phase and context (e.g., `refine — gap clarification`)

Collect all entries as `PRE_CAPTURED_DECISIONS`. If the section is empty or has no `### D-N:` entries, set `PRE_CAPTURED_DECISIONS = []`.

### 2. Cross-reference against artifact prose

Read only the decision-bearing sections of each artifact:

- **SPEC.md:** `## Scope Decisions`, `## Edge Cases`, `## Out of Scope`
- **PLAN.md:** `## Summary` (contains architectural decisions and ordering rationale)
- **STATE.md:** `## Session Log` entries with event type `user-decision`

For each decision found in prose, check if a corresponding D-N entry exists in `PRE_CAPTURED_DECISIONS` (match by topic/content, not exact wording). If a prose decision has no matching D-N entry, extract it as a **newly discovered decision** using the taxonomy below.

### 3. Classify newly extracted decisions

For any decisions found in prose that were NOT pre-captured, classify each as:

| Class | Scope | Examples |
|-------|-------|---------|
| **Architecture** | Technology choices, system design, component boundaries, API design, data model shape | "Use polling over WebSockets"; "API versioning via URL path prefix" |
| **Logic** | Business rule interpretations, edge case handling, validation approaches | "Reject orders with zero quantity instead of silently filtering"; "Use ceiling rounding for tax" |
| **Scope** | Intentional exclusions and accepted trade-offs with lasting impact | "Soft-delete deferred — hard-delete with audit log chosen for v1" |

**Skip** decisions that are purely process-related (how the agent should work, which tool to use, commit strategy). Those belong in `IMPROVEMENTS.md` territory.

### 4. Assess generalizability

For each decision (both pre-captured and newly extracted), determine if it sets precedent beyond this specific work item:

- **Keep:** Decisions that establish a pattern, constrain future work, or reveal a preference the team should respect
- **Skip:** Decisions that are purely item-local (e.g., "field X should be nullable for this migration" with no broader implication)

Note each skipped decision with a reason.

### 5. Check for duplicates and updates

If `.nuff/knowledge/INDEX.md` exists:

1. Search for entries with overlapping tags or similar titles across **all** categories (not just `decisions`)
2. If a match exists, read the full knowledge file
3. Determine: does the new decision **add** information, **supersede** the existing entry, or merely **confirm** it?
4. For supersession: flag the existing entry for update (include both old and new content)
5. For confirmation: note the match and skip

### 6. Detect ADR directory

Search for an existing ADR directory using Glob:

```
docs/**/adr/*
docs/**/decisions/*
adr/*
doc/**/adr/*
```

If found:
- Read the 2-3 most recent ADR files to infer the numbering scheme (e.g., `0001-title.md`, `ADR-001.md`) and section format
- Record the directory path and next available number

If not found: set `adr_directory = null`. Do not propose creating an ADR directory.

### 7. Propose ADRs

Only propose an ADR when **all three** conditions are met:

1. The decision is classified as **Architecture**
2. It has **cross-cutting impact** (affects 3+ components or establishes a pattern for future development)
3. An ADR directory was detected (`adr_directory != null`)

For each ADR proposal, use the project's existing format if detected. Otherwise use this default template:

```markdown
# <N>. <Title>

Date: <YYYY-MM-DD>

## Status

Proposed

## Context

<Why this decision was needed. What forces were at play. Reference the work item.>

## Decision

<What was decided and why this option was chosen over alternatives.>

## Consequences

<What becomes easier or harder as a result. Known trade-offs.>
```

### 8. Create knowledge candidates

For each generalizable decision (not proposed as an ADR, or proposed as ADR + knowledge):

- **category:** `decisions`
- **title:** Imperative, 5-8 words (e.g., "Use polling over WebSockets for updates")
- **tags:** 2-4 keywords
- **file path:** `.nuff/knowledge/decisions/<kebab-case-title>.md`
- **content draft:** Use the standard knowledge file sections:
  - `## Context` — what prompted this decision, which artifact/section it came from
  - `## Learning` — the decision statement and rationale
  - `## Application` — when this decision applies, what future work should do
  - `## Cross-references` — link to ADR if one was proposed, link to source work item

### 9. Return structured result

Return three sections:

**Skipped:**
- Decision summary + reason (item-specific, duplicate, or confirmation of existing)

**Knowledge candidates:**
- category, title, tags, proposed file path, draft content, source (D-N reference or artifact and section)
- For updates to existing entries: existing knowledge path + what to change
- Note which candidates came from pre-captured decisions vs. newly extracted from prose

**ADR proposals:**
- title, context, decision, consequences, proposed file path, source artifact and section
- Only present if `adr_directory != null` and architecture-level decisions with cross-cutting impact were found

## Display Templates

Decision knowledge candidates use the same display format as Step 5.5.1 in the improve workflow, with `Source: <D-N> (<artifact> — <section>)` for pre-captured decisions or `Source: <artifact> — <section> (extracted from prose)` for newly discovered decisions.

### ADR proposal display

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
ADR proposal [N/total]: <title>
Source: <D-N or artifact> — <section>
ADR directory: <detected path>

Context:
  <why this decision was needed>

Decision:
  <what was decided>

Consequences:
  <expected impact>

Proposed file: <adr-directory>/<next-number>-<kebab-title>.md
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### ADR proposal approval options

- **Create ADR** — write the ADR file + create a corresponding `.nuff/knowledge/decisions/` entry
- **Edit** — adjust content before creating
- **Save as knowledge only** — write only the knowledge entry (no ADR file)
- **Skip** — discard

### Autonomous mode (ADR proposals)

Auto-save as knowledge entries only. ADRs are formal project artifacts that warrant human review. Log: `[AUTONOMOUS] Saved ADR proposal "<title>" as knowledge entry.`
