---
name: nuff-knowledge
description: Defines the knowledge file format, categories, INDEX.md structure, and lifecycle rules for .nuff/knowledge/. Consult this skill when creating, updating, or removing knowledge entries during the improve phase.
---

# Nuff Knowledge Management

This skill defines the format and conventions for knowledge files in `.nuff/knowledge/`. Knowledge files capture contextual learnings from agent reflections and user improvements — pitfalls, patterns, conventions, tooling discoveries, and application-level decisions that make future work items easier.

## Knowledge File Format

Each knowledge file lives in `.nuff/knowledge/<category>/` and follows this structure:

```markdown
---
title: "<Short imperative title>"
category: <patterns | pitfalls | conventions | tooling | decisions>
source_type: <reflection | improvement>
source_item: <item-name that generated this knowledge>
created: <ISO-8601 UTC>
updated: <ISO-8601 UTC>
confidence: <high | medium | low>
tags: [<tag1>, <tag2>, ...]
---

## Context

[1-3 sentences describing the situation where this learning was discovered. Include the work item name, phase, and agent that surfaced it.]

## Learning

[The actual knowledge — what to do, what to avoid, or what pattern to follow. Written as actionable guidance.]

## Application

[When and where this knowledge applies. Bullet list of specific situations or triggers.]

## Cross-references

[Links to related AGENTS.md rules or other knowledge files. "None" if standalone.]
```

## Categories

| Category      | Description                                                   | Examples                                                                                      |
| ------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `patterns`    | Reusable codebase patterns that future tasks should follow    | Barrel exports for shared types; service registration pattern; test file co-location          |
| `pitfalls`    | Non-obvious gotchas and problems to avoid                     | Parallel tasks sharing files causes merge conflicts; EF commands need `--no-build`            |
| `conventions` | Project conventions discovered during work (not in AGENTS.md) | API controllers use kebab-case routes; test files live next to source files                   |
| `tooling`     | Tool/environment discoveries worth remembering                | Playwright requires `npx playwright install` before first run; NuGet restore needs auth token |
| `decisions`   | Application-level decisions about logic, architecture, or technology choices | Use polling over WebSockets for updates; API versioning via URL path prefix; hard-delete with audit log over soft-delete |

## File Naming

- Derive from title: lowercase kebab-case, max 50 characters
- Strip common words (the, a, an, is, are, for, to, in, of, with)
- Examples:
  - Title: "Always pass --no-build to EF commands" → `always-pass-no-build-ef-commands.md`
  - Title: "Parallel tasks must not share files" → `parallel-tasks-must-not-share-files.md`

## INDEX.md Format

`.nuff/knowledge/INDEX.md` is auto-generated during the improve phase. It provides a token-efficient summary for consumption by refine and plan workflows.

```markdown
# Knowledge Index

## Patterns

- [Co-locate test files with source files](patterns/co-locate-test-files.md) (tags: patterns, testing, file-organization)

## Pitfalls

- [Sync-time Removed item filtering](pitfalls/sync-time-removed-item-filtering.md) (tags: sync, removed-items, execution-insights)
- [Scan all construction sites before adding required properties](pitfalls/required-properties-cascade.md) (tags: construction, required-properties, cascade)

## Conventions

- [API naming uses kebab-case](conventions/api-naming-kebab-case.md) (tags: api, naming)

## Tooling

- [Playwright requires install before first run](tooling/playwright-requires-install-before-first-run.md) (tags: testing, playwright)

## Decisions

- [Use polling over WebSockets for updates](decisions/use-polling-over-websockets.md) (tags: architecture, real-time, polling)
```

Rules for INDEX.md:

- One line per entry: `- [<title>](<file>) (tags: <comma-separated tags>)`
- Grouped by category
- Sorted alphabetically within each category
- Regenerated from scratch each time — never manually edited

## Knowledge Lifecycle

### Creation

- Knowledge candidates are generated from agent reflections during the improve phase
- Only generalizable learnings become knowledge files (item-specific observations are skipped)
- New entries start with `confidence: medium` (unless user explicitly endorses → `high`)

### Validation

- When a session reflection contradicts existing knowledge, present both to the user:
  - **Update knowledge** — revise the existing file with new information
  - **Remove knowledge** — delete the file and remove from INDEX.md
  - **Keep as-is** — the contradiction may be context-dependent

### Removal

- Knowledge is removed only when actively contradicted by session evidence and approved by the user
- INDEX.md is regenerated after removal

### Promotion to AGENTS.md

- During the improve phase, user can choose "Also add to AGENTS.md" for any knowledge candidate
- This creates both a knowledge file (with full context) and a concise AGENTS.md rule
- The knowledge file's Cross-references section links to the AGENTS.md rule location

## Directory Structure

```
.nuff/knowledge/
  INDEX.md              # Auto-generated summary (never manually edit)
  patterns/             # Reusable codebase patterns
  pitfalls/             # Gotchas and problems to avoid
  conventions/          # Project conventions
  tooling/              # Tool/environment discoveries
  decisions/            # Application logic and architecture decisions
```

## Important Rules

- **Knowledge files are git-tracked.** They are shared with the team via version control.
- **INDEX.md is auto-generated.** Never edit it manually — it is rebuilt from scratch during each improve phase.
- **One learning per file.** Do not combine multiple unrelated learnings in a single file.
- **Keep learnings actionable.** Each file should answer "what should I do?" not just "what happened?"
- **Deduplicate before creating.** Always check INDEX.md for existing entries with matching tags or similar titles before creating a new file.
