---
name: adversarial-review
description: Independent review of a plan or of finished code by a Reviewer model — from a different provider via the GitHub Copilot CLI when available, otherwise a host subagent. Use `plan` mode as soon as a plan's open questions are settled, before presenting it for approval. Use `code` mode every time an implementation is finished, before reporting it as done or creating a PR. Run it unprompted; never offer it as an option.
---

# Adversarial review

Have the work checked by an independent Reviewer — from a different model provider when available — before the human sees it. The goal is to catch real gaps, not to polish: the result has to be good enough for the human to review with less effort — not perfect.

Mode comes from the arguments (`plan` or `code`). If none is given, infer it: a plan whose open questions are settled, not yet presented → `plan`; committed implementation about to be presented → `code`.

**Run it unprompted** at both trigger points. Never offer the review to the user as an option or ask whether to run it. The user sees the plan or the implementation summary only after the review ends, together with its outcome.

## Terms

- **Reviewer** — the independent, read-only model. It never changes the work.
- **Review brief** — the one document handed to the Reviewer.
- **Finding** — a gap, miss or risk that would change the outcome. Style, naming and taste never qualify.
- **Divergence** — a decision taken during implementation that departs from the plan.
- **Review round** — one Reviewer pass plus your response to its Findings. Maximum **3** rounds per review.

## 1. Pick the Reviewer model

| Your own model | Reviewer `--model` |
| -------------- | ------------------ |
| GPT (OpenAI) | `claude-opus-5.5` |
| Anything else (Claude, Gemini, …) | `gpt-6.1-sol` |

Reasoning effort is always `medium`.

## 2. Check the engine

Run `copilot --version`. If it fails, or any Reviewer call below fails (non-zero exit, auth error, unknown model, empty output, or the host's permission system blocks the command), **stop and ask the user** using the built-in tool for asking questions. Show the error and offer three choices:

- **Fix and retry** — the user fixes Copilot CLI, you retry.
- **Host subagent** — run the Reviewer as a subagent of your own host (see [Host subagent Reviewer](#host-subagent-reviewer)).
- **Skip** — skip the adversarial review.

Never switch engines and never skip without the user's choice. Once the user picks the host subagent, use it for every later review in this session without checking Copilot or asking again.

## 3. Write the Review brief

Create a file in the system temp directory — **never inside the repository**. It contains, in order:

1. The full content of the Reviewer instructions for the mode: [plan-reviewer.md](./references/plan-reviewer.md) or [code-reviewer.md](./references/code-reviewer.md).
2. **Requirements** — the original request, in the user's own words where possible.
3. Mode-specific content:
   - `plan`: the plan — every decision reached in the session, with the reason when one was given.
   - `code`: the implementation plan, every Divergence with its reason, the default branch name and the review range `<merge-base>..HEAD`.

Before a `code` review, commit all changes. Compute the base with `git merge-base HEAD <default-branch>`.

## 4. Run the Reviewer

Generate one new UUID per review (not per round) and reuse it in every round so the Reviewer keeps its memory. Run from the workspace root and pipe the brief as the prompt:

```bash
copilot --model <model> --reasoning-effort medium \
  --experimental --assisted-approval --allow-all-tools \
  --no-ask-user --silent --session-id=<uuid> < <brief-file>
```

PowerShell: `Get-Content <brief-file> -Raw | copilot <same flags>`.

A run can take several minutes — allow for a long timeout. The Reviewer ends its answer with `VERDICT: CLEAN` or `VERDICT: FINDINGS` followed by numbered Findings.

### Host subagent Reviewer

Used only when the user chose it in step 2.

- **Model** — a strong model at **high** effort, e.g. Claude Opus or GPT Sol. Never the top, most expensive tier (e.g. Claude Fable, GPT Astra).
- **Prompt** — tell the subagent to read the Review brief file and follow it, working in the workspace root and staying read-only.
- **Later rounds** — send the follow-up to the same subagent when your host can resume it (Claude Code: `SendMessage` to that agent). Otherwise start a fresh subagent per round with the full brief plus every earlier round's Findings and your responses.

Everything else — brief, rounds, verdict, triage — stays the same.

## 5. Respond to Findings

`VERDICT: CLEAN` ends the review. Otherwise handle every Finding, then start the next round by writing a follow-up file with your response to each Finding (by number) and running the same command with the same `--session-id` (or the same subagent, as above).

### `plan` mode

Triage each Finding before involving the user:

- **Already settled** in the session → drop it; tell the Reviewer where it was settled.
- **A fact** you can look up (code, files, tools) → find it yourself and pass the answer on.
- **A real decision** → take it back to the user as the next grilling round. When that round leaves no open questions, pass the user's decisions to the Reviewer as the next review round.

Tell the user briefly which Findings you dropped or answered yourself, and why.

### `code` mode

For each Finding either **fix it** (and commit) or **reject it** with a concrete reason. In the follow-up, list fixes with their commits and rejections with their reasons. The Reviewer re-checks and either accepts or maintains each one.

## 6. Finish

After `VERDICT: CLEAN` or after round 3, report to the user in chat:

- Reviewer engine (Copilot CLI or host subagent), model and number of rounds.
- What was fixed or decided because of the review.
- Findings you rejected and the Reviewer accepted.
- **Open** — anything unresolved after round 3, with both sides' positions, for the user to decide.

In `plan` mode, this report goes with the plan you present for approval. Do not act on the plan while open Findings remain unless the user says so.
