---
name: improve.nuff
description: Review lessons learned from a nuff work item and apply approved improvements to AGENTS.md and .nuff/knowledge/
argument-hint: "[work-item-id-or-name]"
---

# improve.nuff - Improve Phase Orchestrator

You are running the **improve phase** for a nuff work item. Your job is to process lessons learned captured in `IMPROVEMENTS.md` and reusable learnings captured in `REFLECTIONS.md`, then persist approved updates into `AGENTS.md` files and `.nuff/knowledge/`.

**Plugin directory:** All plugin-internal files (skills, etc.) are read from the plugin's install directory — NOT from the current working directory. Repo-specific files (`.nuff/`, `AGENTS.md`, etc.) are read from the current working directory.

## Step 1: Detect Work Item Context

Use the same work item detection logic as `do.nuff` Step 1:

1. **Explicit argument**: If `$ARGUMENTS` is non-empty, parse as a numeric ID, ADO URL, or plain text item name.
2. **Branch name**: Run `git branch --show-current` and extract the nuff item name from the branch.
3. **Existing STATE.md**: Search `.nuff/*/STATE.md` for active items. If multiple, ask the user to pick one.

If no work item can be found, ask: "No active work item found. Please provide a work item ID or name."

## Step 2: Load Context

1. Read `.nuff/<item-name>/STATE.md` (if it exists). Extract the current phase.
2. If STATE.md does not exist, print: `No STATE.md found for <item-name>. Run /do.nuff first.` and exit.
3. Check whether `.nuff/<item-name>/IMPROVEMENTS.md` and `.nuff/<item-name>/REFLECTIONS.md` exist.
4. If neither file exists, print: `No improvements or reflections recorded for <item-name>.` and exit.
5. If only one exists, continue. The improve workflow can process improvements-only, reflections-only, or both.

## Step 3: Load Configuration

Use the `Read` tool directly on `.nuff/config.json` to load it as `CONFIG` (this file is gitignored, so `Glob` will not find it). If `Read` returns an error (file not found), proceed without model config (improve phase does not spawn model-specific subagents).

## Step 4: Execute Improve Workflow

Consult the `nuff-workflow-improve` skill and execute its steps inline.

Important: do not narrow the workflow to `IMPROVEMENTS.md` only. The workflow may update `AGENTS.md`, create or revise `.nuff/knowledge/` entries, regenerate `.nuff/knowledge/INDEX.md`, or do all of these depending on what artifacts exist.

Pass the following context:
- `<item-name>` — the work item name
- `autonomous_mode` — `false` (improve.nuff is always interactive)

## Step 5: Report

After the workflow completes, print a summary:

- **Work item:** `<item-name>`
- **Improvements processed:** `<N applied>/<total>` or `none`
- **Knowledge updates:** `<N new>/<M updated>/<K removed>` or `none`
- **AGENTS.md files updated:** list of modified files (or `none`)
- **Knowledge files updated:** list of modified `.nuff/knowledge/` files (or `none`)
