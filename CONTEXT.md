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
