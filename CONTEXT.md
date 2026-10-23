# Nuff plugins

Plugins and skills that help an AI agent do just enough work, of good enough quality, with as little human babysitting as possible.

## Grilling

**Grilling session**:
A relentless interview that maps a plan as a design tree and asks the user every open decision, round by round, until nothing is silently assumed.
_Avoid_: Interview, Q&A

**Vibe check**:
The moment the user confirms that they and the agent have reached a shared understanding and the grilling session can end.
_Avoid_: Sign-off, approval

## Adversarial review

**Adversarial review**:
An independent check of the agent's work by a Reviewer from a different model provider, looped until the Reviewer has no Findings left or the round cap is hit.
_Avoid_: Peer review, second opinion

**Reviewer**:
The independent, read-only model that performs an Adversarial review; it never changes the work itself.
_Avoid_: Critic, judge, verifier

**Plan review**:
An Adversarial review of a plan after its Vibe check, looking for decisions that are still missing or contradictory.
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
One Reviewer pass plus the agent's response to its Findings; a review stops after three rounds and hands what is left to the user.
_Avoid_: Iteration, pass

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
The folder `D:\.slop-trash`, where removed files are kept instead of being deleted.
_Avoid_: recycle bin, bin

**Read Zone**:
Everywhere an agent may read: the `D:\` drive, Agent Dirs and Temp.

**Write Zone**:
Everywhere an agent may write: the Workspace, Trash, Agent Dirs and Temp.

**Removal**:
A shell delete command (`rm`, `rmdir`, `unlink`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
_Avoid_: delete, cleanup

**Rewrite**:
Replacing a Removal, before it runs, with a move of the same targets into Trash.
_Avoid_: redirect, intercept
