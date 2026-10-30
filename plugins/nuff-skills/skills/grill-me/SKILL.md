---
name: grill-me
description: A relentless interview to sharpen a plan or design, which also creates docs (ADR's and glossary) as we go.
disable-model-invocation: true
---

Run a `/grilling` session, using the `/domain-modeling` skill.

Once the user confirms the shared understanding, run `/adversarial-review plan`. Do not act on the plan until the review ends.

Once there are no gaps and the plan has been approved by human, write it to .agents/PLAN.md. End the plan with this as its last step, word for word:

> **Final step:** when the implementation is done, run `/adversarial-review code`. Only after it ends, report the work as done or create a PR.
