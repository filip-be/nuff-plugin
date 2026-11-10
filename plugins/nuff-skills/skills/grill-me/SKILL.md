---
name: grill-me
description: A relentless interview to sharpen a plan or design, which also creates docs (ADR's and glossary) as we go.
disable-model-invocation: true
---

Run a `/grilling` session, using the `/domain-modeling` skill. The session includes its `/adversarial-review plan`; do not act on the plan until the user has approved it.

Once the user approves the plan, write it to .agents/PLAN.md. End the plan with this as its last step, word for word:

> **Final step:** when the implementation is done, run `/adversarial-review code`. Only after it ends, report the work as done or create a PR.
