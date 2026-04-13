---
name: do.nuff
description: Orchestrate the nuff workflow - detects work item context and routes to the correct phase
argument-hint: "[work-item-id-or-link] [--phase refine|plan|code|improve]"
---

# do.nuff - Orchestrator

You are the nuff orchestrator. Your job is to detect work item context, read or create STATE.md, and route to the correct phase agent via subagent delegation. Follow these steps in order.

**Plugin directory:** All plugin-internal files (`nuff-defaults.json`, skills, agents) are read from the plugin's install directory — NOT from the current working directory. Use the plugin directory for all such file reads. Repo-specific files (`.nuff/config.json`, `.nuff/<item-name>/STATE.md`, etc.) are read from the current working directory as usual.

## Step 1: Initialize Session

Consult the `nuff-session-management` skill with `initialize-session`. This handles:

1. **Work item detection** — parse arguments, check branch name, scan existing STATE.md files
2. **Configuration validation** — check/create `.nuff/config.json`, schema version, alias resolution, platform detection
3. **STATE.md lifecycle** — create or read STATE.md, auto-create feature branch, commit
4. **ADO process template detection** — one-time via `nuff-ado-orchestration`

For detailed step-by-step logic, the session management skill consults its [references/init-steps.md](../skills/nuff-session-management/references/init-steps.md).

After initialization, you have: `<item-name>`, `CONFIG`, `CONFIG_MODELS`, `PLATFORM`, `user_instruction`, `autonomous_mode`, and the active `phase`.

## Step 2: Resume Detection

Consult the `nuff-session-management` skill with `resume-session`. This handles:

1. **Artifact consistency check** — verify required files exist for claimed phase
2. **Uncommitted artifact detection** — find in-progress work
3. **Code progress detection** — count completed vs total tasks
4. **Branch check** — warn if not on expected nuff branch
5. **Resume status presentation** — banner, progress table, confirmation

For detailed step-by-step logic, the session management skill consults its [references/resume-detection.md](../skills/nuff-session-management/references/resume-detection.md).

Skip this step for fresh starts (new STATE.md just created, or phase is `refine` with no artifacts).

## Step 3: Initialize Progress Visualization

Consult the `nuff-progress-sync` skill to create the initial progress items from the STATE.md `## Tasks` section. This is the first sync -- it will create all phase-level items with their current statuses.

## Step 4: Execute Phase Workflow Inline

Determine the active phase. If a `--phase` override was provided in Step 1, use that value instead of the phase from STATE.md. If the override differs from STATE.md, update STATE.md to reflect the overridden phase before routing.

If the override differs from the stored phase, consult `nuff-state-update` with `update-frontmatter` to update `phase` and `updated`, then update the `## Current Phase` heading to match.

### Resumption Context

If this is a resumption (detected in Step 2), print a structured context block before proceeding:

```
RESUMPTION CONTEXT for <item-name>:
  Phase: <phase>
  Progress: <progress details>
  Uncommitted files: <list or "none">
  Resume from: <specific task/step description>
```

### Complexity Assessment (informational)

Before starting, log an informational complexity note based on the work item: `info | complexity — simple` (single file / <=2 requirements), `info | complexity — standard` (3-5 requirements, single architectural layer), or `info | complexity — complex` (6+ requirements or cross-layer). This does not change workflow behavior — it enables future analysis of which components add value at which complexity levels.

### Execute Phase Workflow

Determine the active phase (considering any override).

**CRITICAL: You ARE the orchestrator executing the workflow steps. Do NOT delegate to a separate agent or do an identity switch. Consult the `nuff-workflow-<phase>` skill and execute its steps inline. When the workflow says to spawn a subagent (nuff-\*), you MUST follow the subagent model routing rule from the `nuff-subagent-execution` skill — on Copilot, NEVER call `runSubagent` (VS Code Chat) or `task` (Copilot CLI) with agent names like `nuff-implementer` — those are NOT registered. ALWAYS use `@nuff-copilot-subagent-<alias>` wrappers instead.**

**Context to carry into each workflow:** The workflow files reference context variables. Before executing, ensure you have:

- `<item-name>` -- the work item name detected in Step 1
- `.nuff/<item-name>/STATE.md` -- the path to STATE.md
- `CONFIG` -- the user config loaded in Step 1
- `CONFIG_MODELS` -- the resolved models section (aliases expanded to actual model IDs)
- `PLATFORM` -- the detected platform (`claude-code` or `copilot`)
- `user_instruction` -- the plain text string provided by the user in Step 1 (if any); `null` otherwise
- `autonomous_mode` -- `true` when invoked via `do-everything.nuff`; when set, the `ask-user` skill self-answers all decision points

Use this routing table:
| Phase    | Action |
| -------- | ------------------------------------------------------------------------------------- |
| refine   | Consult the `nuff-workflow-refine` skill and execute its steps inline |
| plan     | Consult the `nuff-workflow-plan` skill and execute its steps inline |
| code     | Consult the `nuff-workflow-code` skill and execute its steps inline |
| improve  | Consult the `nuff-workflow-improve` skill and execute its steps inline |
| complete | Report: "Work item `<item-name>` is **complete**. All phases finished." |

Consult the `nuff-state-update` skill with `append-session-log` for `phase-start | <phase>`.

## Step 5: Phase Loop

After the phase completes:

1. Read `.nuff/<item-name>/STATE.md` to confirm the phase was updated by the workflow (check that the `phase` frontmatter field has advanced to the next phase). Each workflow is responsible for updating its own STATE.md fields (frontmatter, Current Phase heading, Tasks checklist, Phase History table, and session log). The orchestrator should NOT duplicate these updates.
2. **Progressive summarization:** Summarize the completed phase in 2-3 sentences (key decisions, artifacts produced, commit hashes). Full details are now in committed artifacts — reference SPEC.md/PLAN.md/STATE.md for subsequent phases rather than recalling conversation history.
3. **Update progress visualization:** Consult the `nuff-progress-sync` skill to sync the updated STATE.md to the runtime task list.
4. Determine the next phase:

| Completed       | Next     |
| --------------- | -------- |
| refine          | plan     |
| plan            | code     |
| code            | improve  |
| improve         | complete |

5. If the next phase is `complete` (i.e., the workflow already set `phase: complete` in STATE.md):
  - Verify STATE.md phase is `complete`. If the workflow did not set it (e.g., an older workflow version), consult `nuff-state-update` with `complete-improve` as a fallback.
  - If session log does not already contain a `phase-complete | all phases done` entry, consult `nuff-state-update` with `append-session-log`.
   - Print: "All phases complete for `<item-name>`."
   - Proceed to Step 6.

6. If there is a next phase, first print a context window advisory:

   > **Context window note:** If your conversation is getting long, you can clear context and continue fresh. Run `/clear`, then `/do.nuff --phase <next phase>` (nuff will auto-resume from STATE.md).

   Then ask the user (consult the `ask-user` skill):
   - "Phase `<completed>` is done. Proceed to `<next phase>`?"
   - Options: **Yes, proceed** / **No, stop here**
  - If yes: consult `nuff-state-update` with `update-frontmatter` to set `phase: <next phase>` and refresh `updated`, then consult `nuff-state-update` with `append-session-log` for `phase-start | <next phase>`, and go back to Step 4 to execute the next phase workflow.
   - If no: print current status. Remind the user they can continue later by running `/clear` then `/do.nuff --phase <next phase>`. Exit.

## Step 6: Report Status

After the workflow completes (either all phases done or user stopped), provide a summary:

- **Work item:** `<item-name>`
- **Phase:** `<current phase>` (note if overridden via `--phase`)
- **Work item ID:** `<id>` (if known)
- **Work item URL:** `<url>` (if known)
- **Artifacts:** List all files in `.nuff/<item-name>/` using Bash:

```bash
ls -1 .nuff/<item-name>/ 2>/dev/null || echo "(no artifacts yet)"
```

- **STATE.md location:** `.nuff/<item-name>/STATE.md`
