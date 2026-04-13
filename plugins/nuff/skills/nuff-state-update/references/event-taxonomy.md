# Session Log Event Taxonomy

Standard event types for `## Session Log` entries. Use these exact values in the `<event-type>` field.

Format: `- <ISO-8601> | <event-type> | <details>`

## Lifecycle Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `session-start` | New or resumed session | `via do.nuff` |
| `session-end` | Clean session termination | `all phases done` |
| `phase-start` | Phase entered | `refine` |
| `phase-complete` | Phase finished successfully | `refine` |
| `task-start` | Task implementation began | `task 3 update-query` |
| `task-complete` | Task finished | `task 3 (a1b2c3d)` |
| `artifact-committed` | Git commit for artifact | `SPEC.md` |

## Subagent Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `subagent-start` | Subagent spawned | `implementer task 3 (sonnet)` |
| `subagent-complete` | Subagent returned successfully | `implementer task 3 — success` |
| `subagent-fail` | Subagent returned failure | `implementer task 3 — test failure after 3 attempts` |

## Verification Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `verification-start` | Verification iteration began | `iteration 2` |
| `verification-complete` | Verification finished | `iteration 1 — 0 issues` |

## External System Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `ado-success` | ADO operation succeeded | `transition #1069944 → In Progress` |
| `ado-fail` | ADO operation failed | `transition #1069944 — 401 Unauthorized` |
| `git-fail` | Git operation failed | `commit — empty staging area` |

## Informational Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `spec-deviation` | SPEC.md updated with approved deviations | `2 requirements updated to match approved implementation` |
| `skip` | Operation skipped (with reason) | `create-child-tasks — no ADO project configured` |
| `env-warn` | Environment limitation | `visual-verification — no Playwright available` |
| `info` | Informational note | `task 3 gap-check — passed, no findings` |
| `warning` | Non-blocking concern | `branch mismatch: fix-scaffolding-migration` |

## Decision Events

| Type | Meaning | Example details |
|------|---------|----------------|
| `user-decision` | User made a choice | `approve SPEC.md` |
| `user-restart` | User triggered restart | `re-opened for v2` |
| `autonomous-decision` | Autonomous mode self-answered | `approve SPEC.md — 0 pushback triggers` |
| `autonomous-blocked` | Autonomous mode cannot proceed | `gap ambiguity exceeds threshold` |
| `decision-captured` | Structured decision recorded in Key Decisions | `D-3: Use polling over WebSockets [Architecture]` |
| `decision-validated` | Decisions validated/promoted during improve | `4 pre-captured, 1 newly extracted from SPEC.md` |
| `adr-proposed` | ADR proposal generated | `ADR-0005: Use polling over WebSockets` |
| `adr-created` | ADR file written | `docs/adr/0005-use-polling.md` |
