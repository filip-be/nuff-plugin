---
name: ask-user
description: Ask the user a question with structured options. Use this skill whenever you need user input, a decision, or confirmation during any nuff phase.
---

# Ask User Question

This skill wraps the platform-specific "ask question" tool so that nuff agents work consistently across different AI coding harnesses.

## Why this skill exists

Different platforms expose different built-in tools for presenting questions to the user. Rather than hard-coding a specific tool name in every agent prompt, agents should use this skill as the single reference for how to ask questions.

## Platform tools

Use whichever built-in tool is available in your current environment:

| Platform               | Tool name             |
| ---------------------- | --------------------- |
| Claude Code (CLI)      | `AskUserQuestion`     |
| GitHub Copilot CLI     | `ask_user`            |
| VS Code (Copilot Chat) | `vscode_askQuestions` |

If none of these are available, check your environment for an equivalent interactive question/prompt tool. The key requirement is that the tool presents **structured options** to the user (not a plain text menu) and returns the user's selection.

## When to use

Use this skill at every decision point where user input is needed:

- Choosing between options (e.g., keep vs. discard, approve vs. revise)
- Confirming before proceeding (e.g., spec approval, plan approval)
- Gathering input (e.g., clarification questions during refine)
- Presenting issues that need a user decision (e.g., verification failures)
- Any point where the agent should NOT make an autonomous decision

## How to use

1. **Always use the built-in tool** — never print a plain text menu and wait for freeform input. The structured tool keeps the interaction loop unbroken and ensures consistent UX.
2. **Provide clear options** — each option should have a short label and a description of what happens if chosen.
3. **Always include a freeform escape hatch** — every question must have an option like "Other", "None of these", or "I'll type it manually" so the user is never forced into predefined choices. Some platform tools add this automatically (e.g., Claude Code's `AskUserQuestion` always includes "Other"); for tools that don't, add it as an explicit option.
4. **Group related questions** — when you have multiple related questions (e.g., scope decisions), batch them logically rather than asking one at a time.
5. **Use multiSelect when appropriate** — if the user can pick more than one option (e.g., selecting which issues to fix), enable multi-select.
6. **Respect user overrides** — if the user says "proceed", "that's fine", or similar, accept their decision and move on. Don't re-ask the same question.

## Autonomous mode

When `autonomous_mode = true` (invoked via `do-everything.nuff`), do NOT call the platform question tool. Instead, self-answer every decision point:

1. Reason through the options internally.
2. Apply the decision heuristics table below.
3. Before committing, briefly ask yourself: "Am I missing context here? Is there a safer path?" Log that self-check.
4. Print: `[AUTONOMOUS] <question summary> → <chosen option> | reason: <one-line reasoning>`
5. Append to STATE.md session log: `- <ISO-8601 UTC timestamp> | autonomous-decision | <question-summary>: <chosen-option>`
6. Continue immediately — do not wait for input.

> **Vigilance note:** Resist overconfidence. If a situation is genuinely ambiguous with no clear best-effort path, stop and print `[AUTONOMOUS-BLOCKED] <explanation>` — this is preferable to silently making a wrong assumption.

### Autonomous decision heuristics

**General decisions:**

| Situation                                | Autonomous decision                                                   |
| ---------------------------------------- | --------------------------------------------------------------------- |
| Multiple active items found              | Choose the most recently updated item                                 |
| No active items and no argument provided | `[AUTONOMOUS-BLOCKED]` — work item context required                   |
| Config file missing                      | Silently pull defaults — no prompt                                    |
| Config schema version mismatch           | Silently auto-migrate — no prompt                                     |
| Missing required artifact                | Proceed; log `[AUTONOMOUS] artifact missing, proceeding with caution` |
| Uncommitted artifact detected            | Auto-approve and commit                                               |
| Resume confirmation                      | Auto-continue                                                         |
| Item already complete                    | Skip re-verify; print summary and exit                                |
| Gap analyzer findings (non-blocking)     | Log findings, proceed                                                 |
| Gap analyzer pushback trigger            | Log `[AUTONOMOUS-PUSHBACK-OVERRIDDEN]`; proceed                       |
| Proceed to next phase?                   | Auto-proceed                                                          |
| Clarification questions during refine    | Best-effort answer from work item + repo; log reasoning               |

**Approval gates (SPEC, PLAN, verification):** Consult [references/autonomous-approval-criteria.md](./references/autonomous-approval-criteria.md) for concrete thresholds.
