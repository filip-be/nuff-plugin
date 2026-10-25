---
name: grill-me
description: A relentless interview to sharpen a plan or design, which also creates docs (ADR's and glossary) as we go.
disable-model-invocation: true
---

Run a `/grilling` session, using the `/domain-modeling` skill.

Once the user confirms the shared understanding, run `/adversarial-review plan`. Do not act on the plan until the review ends.

Once there are no gaps and the plan has been approved by human, write it to .agents/PLAN.md.
