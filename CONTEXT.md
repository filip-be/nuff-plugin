# Nuff plugins

Plugins and skills that help an AI agent do just enough work, of good enough quality, with as little human babysitting as possible.

## Workflow

**Workflow**:
The three steps day-to-day work goes through: a Grilling session that ends in an approved Plan file, implementation of that plan ending in a Code review, and a pull request led by an Opener.
_Avoid_: Pipeline, phases

**Plan file**:
The plan a Grilling session saves to `.agents/PLAN.md` once its Plan review has ended; implementation starts from it alone, so the context can be cleared before implementing.
_Avoid_: Spec, PLAN

## Grilling

**Grilling session**:
A relentless interview that maps a plan as a design tree and asks the user every open decision, round by round, until nothing is silently assumed.
_Avoid_: Interview, Q&A

**Vibe check**:
The moment the user confirms the shared understanding, presented only once the grilling has no open questions left and the Plan review has ended; it ends the grilling session.
_Avoid_: Sign-off, approval

## Adversarial review

**Adversarial review**:
An independent check of the agent's work by a Reviewer — from a different model provider when available — looped until the Reviewer has no Findings left or only Open findings remain.
_Avoid_: Peer review, second opinion

**Reviewer**:
The independent, read-only model that performs an Adversarial review: a different provider's model via the GitHub Copilot CLI when available, otherwise a subagent of the agent's own host, chosen by the user. It never changes the work itself.
_Avoid_: Critic, judge, verifier

**Plan review**:
An Adversarial review of a plan as soon as its open questions are settled, before its Vibe check, looking for decisions that are still missing or contradictory.
_Avoid_: Grilling review

**Code review**:
An Adversarial review of committed changes before they are presented, checking them against the requirements, the plan and any Divergences.
_Avoid_: Final code review, PR review

**Review brief**:
The single document the agent hands to the Reviewer: original requirements, the plan or decisions, and any Divergences.
_Avoid_: Context, handoff

**Finding**:
A gap, miss or risk raised by the Reviewer that would change the outcome; style, naming and taste never qualify.
_Avoid_: Comment, nitpick, issue

**Divergence**:
A decision taken during implementation that departs from the original plan.
_Avoid_: Deviation, change of plan

**Review round**:
One Reviewer pass plus the agent's response to its Findings. There is no round limit.
_Avoid_: Iteration, pass

**Open finding**:
A Finding the Reviewer maintained after the agent rejected it, with neither side having a new argument; it ends the review and goes to the user to decide.
_Avoid_: Deadlock, unresolved issue

## PR description

**Opener**:
The one to three sentences at the top of a pull request description that say what the change is for, taken from the user's own words or drafted by the agent from the user's original request. Once posted it belongs to the user.
_Avoid_: Summary, TL;DR, intro

**Collapsed description**:
The agent's full pull request description, folded away below the Opener under a label naming the model that wrote it.
_Avoid_: Details, body, AI section

## Guardrail hooks

Hooks that keep an honest agent from accidentally reading, writing or deleting outside the places it should touch.

**Workspace**:
The project directory the agent session started in, and everything below it. For a session in a git worktree, that is the worktree root.
_Avoid_: project, repo, root

**Agent Dirs**:
The agent's own home configuration directories (e.g. `~/.claude`, `~/.copilot`).
_Avoid_: config dirs, home

**Temp**:
The OS temp directory, including the agent's scratchpad inside it.
_Avoid_: scratch

**Trash**:
The folder where removed files are kept instead of being deleted: `D:\.slop-trash` on Windows, `~/.slop-trash` on Linux and macOS.
_Avoid_: recycle bin, bin

**Read Roots**:
Extra directories the user lets agents read, listed under `readRoots` in `~/.nuff-hooks.json` (e.g. `C:\DEV` on a Windows machine whose projects are not on `D:\`, or `/opt/src` on Linux). The file sits outside the Write Zone, so an agent can't widen its own Read Zone.
_Avoid_: allowlist, read paths

**Cloud Session**:
A Claude Code session running in a sandboxed, throwaway cloud container (Claude Code on the web, claude.ai chats that run code), marked by `CLAUDE_CODE_REMOTE=true`. Its extra directories (`CLAUDE_ADDITIONAL_DIRECTORIES`, e.g. `/mnt/user-data`) are its **Cloud Dirs**.
_Avoid_: remote session, web session

**Read Zone**:
Everywhere an agent may read: the Workspace, the `D:\` drive (Windows only), Read Roots, Agent Dirs and Temp. In a Cloud Session: everywhere.

**Write Zone**:
Everywhere an agent may write: the Workspace, Trash, Agent Dirs and Temp, plus the Cloud Dirs in a Cloud Session.

**Removal**:
A shell delete command (`rm`, `rmdir`, `unlink`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
_Avoid_: delete, cleanup

**Rewrite**:
Replacing a Removal, before it runs, with a move of the same targets into Trash.
_Avoid_: redirect, intercept
