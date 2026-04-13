---
name: nuff-reflection-capture
description: "Capture a subagent or verifier `### Reflections:` section into .nuff/<item-name>/REFLECTIONS.md. Use when refine, plan, or code receives reflections that should be preserved for the improve phase."
---

# Nuff Reflection Capture

This skill is the single reference for extracting `### Reflections:` content from a result blob and appending it to `.nuff/<item-name>/REFLECTIONS.md`.

## When to use

Consult this skill whenever a workflow result from a subagent or verifier may contain a `### Reflections:` section. Typical callers:

- `nuff-workflow-refine` after gap analysis
- `nuff-workflow-plan` after codebase analysis or plan gap analysis
- `nuff-workflow-code` after implementer, task gap analysis, or verifier output

## Required inputs

- `item-name` and the `.nuff/<item-name>/` path
- The full result text to inspect
- A source label for the entry heading, such as `Refine gap analysis (gap-analyzer)`
- The current ISO-8601 UTC timestamp

## Procedure

1. Check whether the result contains a `### Reflections:` section.
2. If the section is missing, or the content is exactly `None`, skip silently.
3. Extract everything under `### Reflections:` up to the next `###` heading or the end of the result.
4. If `.nuff/<item-name>/REFLECTIONS.md` does not exist, create it with this header:

   ```markdown
   # Session Reflections

   Agent-generated reflections captured during the nuff session. These are processed during the improve phase.

   ---
   ```

5. Append a new entry using this exact format:

   ```markdown
   ## <source label> — <ISO-8601 UTC timestamp>

   <extracted reflections text>
   ```

## Important notes

- This skill only manages the reflection log format and append behavior.
- The caller decides the source label and when the result is important enough to inspect.
- Keep entries chronological by appending new reflections at the end of the file.
- Do not rewrite or reformat older reflection entries when adding a new one.
