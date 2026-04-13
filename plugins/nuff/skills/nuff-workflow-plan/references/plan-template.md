# PLAN.md Template

Use this template when the plan workflow writes `.nuff/<item-name>/PLAN.md`. Replace every placeholder with item-specific content derived from the spec, codebase analysis, and dependency review.

```markdown
# PLAN.md

**Item:** <item-name>
**Spec:** SPEC.md (refined <timestamp from SPEC.md Refined: field>)
**Planned:** <current ISO-8601 UTC timestamp>

## Summary

[1-2 paragraph overview of the implementation approach. Describe which layers are touched,
the general ordering strategy (e.g., "bottom-up: types first, then components, then wiring"),
and any architectural decisions made during planning. If SPEC.md had open questions that the
user chose to proceed with, note which tasks are affected by the ambiguity.]

## Tasks

### Task 1: [Short descriptive name]

- **Files:** `path/to/file.ts` [Modify]
- **Approach:** [2-5 sentences describing what to change and why]
- **Dependencies:** None
- **Parallel:** Yes
- **Requirements:** REQ-1, REQ-2

### Task 2: [Short descriptive name]

- **Files:** `path/to/another.ts` [Modify], `path/to/new.ts` [Create]
- **Approach:** [What to change and how it relates to previous tasks]
- **Dependencies:** Task 1
- **Parallel:** No
- **Requirements:** REQ-3

[... additional tasks ...]

## Execution Order

### Chapter 1 (parallel)

Tasks with no dependencies -- can start immediately and run concurrently.

- Task 1: [name]
- Task 2: [name]

### Chapter 2 (parallel, after Chapter 1 completes)

- Task 3: [name]
- Task 4: [name]

### Chapter 3 (sequential, after Chapter 2 completes)

- Task 5: [name]

## Verification Notes

[Hints for the verify agent: specific commands to run, expected behaviors to check,
or SPEC.md requirements that are pure verification with no implementation task.]

- [e.g., Run `npm run build` to verify no TypeScript errors after changes]
- [e.g., Run `npm run lint` for code style]
- [e.g., REQ-N is a verification requirement -- verified by [specific check]]
```