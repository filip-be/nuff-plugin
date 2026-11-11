# nuff-skills

Set of skills that are good enough to improve your AI workflow.

## Skills

| Name | Description | Auto-discovered |
| ---- | ----------- | --------------- |
| grill-me | Grilling session that also builds your project's domain model, sharpening terminology and updating CONTEXT.md and ADRs inline. | No |
| grilling | Interview the user relentlessly about a plan, decision, or idea until every branch of the design tree is resolved and the plan has passed adversarial review. The reusable interview primitive behind grill-me. | Yes |
| domain-modelling | Actively build and sharpen a project's domain model — challenge terms against the glossary, stress-test with edge-case scenarios, and update CONTEXT.md and ADRs inline. | Yes |
| challenge-domain-model | Challenge existing domain model, CLAUDE.md/AGENTS.md and README files structure | No |
| adversarial-review | Independent review of a plan or of finished code by a model from a different provider (via GitHub Copilot CLI) when available, else a host subagent, looped until no real gaps remain or 3 rounds pass. grilling runs it before presenting the plan; grill-me puts the code review in the plan as its final step. | Yes |
| pr-description | Write pull request descriptions as a short opener with the full agent-written text collapsed below it in a `<details>` block labelled with the model's name. | Yes |
| implement-plan | Implement the plan defined in `.agents/PLAN.md` (e.g. one written by grill-me). | No |
