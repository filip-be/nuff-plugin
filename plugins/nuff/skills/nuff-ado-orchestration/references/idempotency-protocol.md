# ADO State Transition Idempotency Protocol

Apply this protocol for all `transition-parent-work-item`, `transition-child-task`, and `transition-review-state` operations.

## Before transitioning

1. **Read current ADO state** via MCP `wit_get_work_item` (or CLI `az boards work-item show --id <id> --project <project> --output json`).
2. **Compare** current ADO state with the target state:
   - If **already in target state**: skip the transition, update STATE.md via `nuff-state-update` with `set-ado-state` to match, log `info | ado-already-in-state — #<id> already <state>`.
   - If **in a different state**: proceed with the transition.

## After successful transition

1. Call `nuff-state-update` with `set-ado-state` (or `set-ado-child-task-state`) to persist the new state.
2. For critical transitions (review state): optionally re-read ADO state to confirm the transition took effect.

## After failed transition

1. Log `ado-fail | <operation> — <error message>`.
2. Re-read current ADO state if possible.
3. Update STATE.md with the **actual** ADO state (not the intended target), so STATE.md reflects reality even on failure.
4. Continue workflow (all ADO failures remain non-blocking).

## Why this matters

Without read-before-write, these scenarios cause STATE.md/ADO drift:
- Session killed after ADO transition but before `nuff-state-update` — resume re-attempts transition
- ADO state manually changed outside nuff — nuff attempts an invalid transition
- Network failure during `nuff-state-update` — STATE.md lags behind ADO
