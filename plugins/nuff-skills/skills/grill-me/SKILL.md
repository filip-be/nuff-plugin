---
name: grill-me
description: A relentless interview to sharpen a plan or design, which also creates docs (ADR's and glossary) as we go.
disable-model-invocation: true
---

Run a `/grilling` session, using the `/domain-modeling` skill. The session includes its `/adversarial-review plan`.

As soon as the plan review ends, write the plan to .agents/PLAN.md without asking first. Only then present the plan and ask the user to approve it, in a single approval step. Writing the file is not acting on the plan: do not implement anything until the user has approved it. If the user asks for changes, update .agents/PLAN.md and ask again.

End the plan with this as its last step, word for word:

> **Final step:** when the implementation is done, run `/adversarial-review code`. Only after it ends, report the work as done or create a PR.
