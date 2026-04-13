# SPEC.md Template

Use this template when the refine workflow writes `.nuff/<item-name>/SPEC.md`. Replace every placeholder with item-specific content.

```markdown
# SPEC.md

**Item:** <item-name>
**Work Item:** [#<id>] <title> <- include only if ADO mode; omit for manual input
**Refined:** <current ISO-8601 UTC timestamp>

## Overview

[1-2 paragraph summary of what this work item delivers and why it exists. Written in plain language. State the goal, the affected system/user, and the value delivered.]

## Requirements

[Numbered list of specific, testable requirements using MUST/SHOULD language. Each requirement must be concrete enough that you could write a pass/fail test for it.]

1. The system MUST [specific behavior]
2. When [condition], the system MUST [specific behavior]
3. ...

## Acceptance Criteria

[Per-requirement acceptance criteria. Use Given/When/Then for complex behavioral requirements; use checklist format for simple capability requirements. Apply discretion.]

**Format selection guide:**

- Use Given/When/Then when: user interactions trigger a behavior, multiple system states are involved, the "when" matters, there is complex conditional logic
- Use checklist when: simple capability ("the system can X"), no meaningful trigger condition, configuration or infrastructure requirement, multiple independent properties must all be true

### AC[N]: [Requirement name or short description]

[Either:]

- **Given** [initial condition]
- **When** [trigger]
- **Then** [expected outcome]
- **And** [additional outcome]

[Or:]

- [ ] [Specific verifiable condition]
- [ ] [Specific verifiable condition]

## Out of Scope

[Mandatory section. Explicit list of things NOT covered by this work item. Every approved spec should have at least 2-3 out-of-scope items to prevent scope creep.]

- [What is explicitly excluded]
- [What is deferred to a later work item or phase]
- ...

## Edge Cases

[Specific scenarios and their expected behaviors, derived from clarification. Focus on the 2-4 most important edge cases.]

- **[Scenario name]:** [Expected behavior -- specific, not "handled gracefully"]
- ...

## Scope Decisions

[Record of any pushback raised and user responses. Include acknowledged risks. If no pushback was raised, write "None -- requirements were clear and well-scoped."]

- [Decision made during clarification]
- **Acknowledged Risk:** [Any concern the user accepted and overrode]

## Documentation Impact

[Which documentation files need updating as a result of this work item, and what needs to change. If none, explain why (e.g., internal-only change, no user-facing behavior added or removed). If no documentation exists in the repo, note that.]

- **`path/to/doc.md`** — [what needs updating and why]
- **No update needed:** [reason]

## Open Questions

[Any questions that could not be resolved during clarification. Should be empty for an approved spec. If empty, write "None -- all questions resolved during clarification."]

- [Question that remains unresolved]
```