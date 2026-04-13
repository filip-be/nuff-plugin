# Ablation Testing Guidance

Nuff's multi-agent architecture includes verification at multiple layers. As models improve, some layers may become overhead rather than load-bearing. Use this guide to test.

## Configurable Components

### Task-check gap analysis (`behavior.task_gap_check`)

Controls whether `nuff-gap-analyzer` runs in task-check mode after each implementer completes.

| Value | Behavior |
|-------|----------|
| `always` (default) | Run gap-check after every task |
| `complex-only` | Skip if task has <= 2 files AND approach <= 5 lines |
| `never` | Skip all task-checks; rely on bulk verification only |

**How to test:** Set `"task_gap_check": "never"` in `.nuff/config.json`. Run 3-5 work items of varying complexity. Compare:
- Verification iteration count (primary metric — did skipping task-checks cause more verification loops?)
- Issue severity in verification reports (did task-check catch things the verifier missed?)
- Total execution time

**When to disable:** If verification iteration counts are unchanged across 3+ items AND verifier catches all issues that task-check would have flagged.

## Future Candidates

These components are not yet configurable but may warrant testing:

- **Codebase analyzer**: For repos with thorough AGENTS.md/README and stable structure, the planner may find files directly. Monitor plan quality (file target accuracy) with and without.
- **Refine gap analysis for simple items**: For single-file bug fixes classified as "simple," gap analysis overhead (4+ minutes) may exceed the fix itself. Consider auto-skipping for simple items.

## Tracking Results

Record ablation experiments in `.nuff/knowledge/tooling/ablation-results.md` so future sessions can reference what was tested and the outcome.
