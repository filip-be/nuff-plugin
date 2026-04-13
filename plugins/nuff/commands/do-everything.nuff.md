---
name: do-everything.nuff
description: Fully autonomous nuff workflow - runs Refine → Plan → Code → Improve end-to-end without user interaction
argument-hint: "[work-item-id-or-link]"
---

# do-everything.nuff - Autonomous Orchestrator

You are running in **autonomous mode**. Your job is to complete the full nuff workflow (Refine → Plan → Code → Improve) for the given work item without any user interaction. You will self-answer all decision points, log your reasoning, and proceed to completion.

## Autonomous mode rules

Before starting, internalize these rules — they override the interactive behavior described in `do.nuff`:

1. **Never invoke the `ask-user` skill interactively.** When a workflow step says to consult `ask-user`, follow the autonomous mode section of that skill instead — reason internally, log the decision, and continue.
2. **Stay vigilant.** Before every autonomous decision, briefly consider whether you are missing context or making an unsafe assumption. Log that self-check.
3. **Print a `[AUTONOMOUS-BLOCKED]` message and stop** if a situation is genuinely ambiguous with no safe best-effort path. Stopping cleanly is better than silently making a wrong decision.
4. **Log everything.** Every autonomous decision must be printed as `[AUTONOMOUS] <decision>` and appended to the STATE.md session log.

## How to run

Set `autonomous_mode = true` in your working context, then follow all instructions in `do.nuff` exactly — every step, every sub-step, in order. The `ask-user` skill will handle all decision points autonomously based on the heuristics in its autonomous mode section.

Pass `$ARGUMENTS` through unchanged to the work item detection logic in Step 1 of `do.nuff`.

## Start banner

Before executing Step 1, print:

```
╔══════════════════════════════════════════════════════════╗
║           nuff — AUTONOMOUS MODE                         ║
║  Running full Refine → Plan → Code → Improve             ║
║  All decisions logged to STATE.md session log            ║
╚══════════════════════════════════════════════════════════╝
```

Then proceed with `do.nuff` Step 1.
