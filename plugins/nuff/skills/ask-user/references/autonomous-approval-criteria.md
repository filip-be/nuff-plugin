# Autonomous Approval Gate Criteria

Concrete thresholds for autonomous mode self-approval at each phase gate. Consult this reference only when `autonomous_mode = true`.

## SPEC.md Approval (refine phase)

**Auto-approve** if ALL of these are true:
- Zero pushback triggers of type `scope-too-broad` or `contradictions`
- Max 2 unresolved `vague-terms` gaps (with self-provided measurable alternatives)
- Max 1 unresolved `missing-edge-cases` gap (with self-selected default behavior)
- All requirements (REQ-N) have at least one acceptance criterion
- All acceptance criteria are testable (have a pass/fail condition)

**Auto-block** (`[AUTONOMOUS-BLOCKED]`) if ANY of these are true:
- Any `contradictions` pushback trigger detected
- More than 3 `vague-terms` gaps remain after self-resolution
- Any gap where the resolution fundamentally changes scope (e.g., "include feature X or not?")
- Work item type is PBI/User Story with more than 5 requirements (complex scope benefits from human judgment)

## PLAN.md Approval (plan phase)

**Auto-approve** if ALL of these are true:
- 100% requirement coverage (every REQ-N referenced by at least one task)
- Zero `uncovered` findings from gap-analyzer plan review
- No tasks with vague approach descriptions (flagged by gap-analyzer as "approach vagueness")
- Dependency graph has no circular dependencies between chapters

**Auto-block** (`[AUTONOMOUS-BLOCKED]`) if ANY of these are true:
- Any REQ-N not covered by any task
- Gap-analyzer flagged out-of-scope creep
- More than 8 tasks total (complex decomposition benefits from human review)

## Verification Approval (code phase)

**Auto-approve** if ALL of these are true:
- All technical checks pass (build, test, lint — no FAIL results)
- All REQ-N marked SATISFIED in verification report
- Zero `error` severity issues

**Auto-approve with `[AUTONOMOUS-UNRESOLVED]`** if:
- 3 verification iterations completed
- Only `warning` severity issues remain (all `error` issues resolved)

**Auto-block** (`[AUTONOMOUS-BLOCKED]`) if:
- Any `error` severity issue persists after 3 verification iterations
- Technical checks (build/test) still failing after 3 iterations

## Logging

All autonomous approval decisions must include rationale:
```
<timestamp> | autonomous-decision | <gate-name> — <criteria-summary>
```

Example: `autonomous-decision | approve SPEC.md — 0 contradictions, 1 vague-term (self-resolved), 3 REQs all with testable ACs`
