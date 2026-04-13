# Resume Detection — Detailed Steps

This reference covers Steps 2.5 and 2.6 from the original orchestrator. Run after STATE.md is read.

## Step 2.5: Resume Detection

### 2.5.1: Artifact Consistency Check

Validate STATE.md phase claims against required artifacts using Glob and `git status --porcelain`:

| Phase    | Required Artifacts                                          |
| -------- | ----------------------------------------------------------- |
| refine   | (none required)                                             |
| plan     | SPEC.md must exist and be committed                         |
| code     | SPEC.md and PLAN.md must exist and be committed             |
| improve  | SPEC.md and PLAN.md must exist and be committed             |
| complete | SPEC.md and PLAN.md should exist (with requirement markers) |

For each required artifact, check:
1. File existence via Glob for `.nuff/<item-name>/<artifact>`
2. Committed status via `git status --porcelain .nuff/<item-name>/<artifact>`

If a required artifact is missing: `Warning: STATE.md says phase is '<phase>' but <artifact> is missing.` Ask user: "Reset to `<earlier phase>` or proceed anyway?"

### 2.5.2: Uncommitted Artifact Detection

```bash
git status --porcelain .nuff/<item-name>/
```

For each uncommitted artifact file (SPEC.md, PLAN.md):
- Ask user: `Uncommitted: <file> -- Approve and commit, or restart?`
- If approve: `git commit -m "nuff(<item-name>): commit in-progress <artifact>"`
- If restart: proceed to phase (which will overwrite)

### 2.5.3: Code Progress Detection

If current phase is `code`:
1. Read PLAN.md, count total task headings
2. Count tasks with `[done: ...]` markers
3. Fallback: check `git log --oneline | grep "nuff(<item-name>): task"`
4. Store as `<done>/<total> tasks`

### 2.5.4: Branch Check

```bash
git branch --show-current
```

If branch does NOT contain `nuff-<item-name>`:
`Warning: Expected branch matching 'nuff-<item-name>' but on '<branch>'. Commits will land here.`

Do NOT auto-switch branches.

### 2.5.5: Manual Change Detection

```bash
git log --oneline -1 -- ".nuff/<item-name>/"
git diff <hash> -- ".nuff/<item-name>/" --stat
```

If changes: `Note: .nuff/<item-name>/ files were modified outside nuff since the last commit.`

### 2.5.6: Other In-Progress Items

Scan `.nuff/*/STATE.md` files. For each non-complete item that isn't the current one:
`Also in progress: <other-item-name> (<phase>)`

## Step 2.6: Present Resume Status

Runs if this is a resumption (STATE.md exists, non-refine phase, no `--phase` override, or uncommitted artifacts exist). Skip for fresh starts.

### 2.6.1: Resume Banner

`Resuming: <item-name> -- Phase: <phase> (<progress details>)`

Progress details by phase:
- **refine:** "SPEC.md in progress" or "SPEC.md ready"
- **plan:** "PLAN.md in progress" or "PLAN.md ready"
- **code:** "N/M tasks complete" or "verification in progress"
- **complete:** "all phases done"

### 2.6.2: Progress Summary

```
  Refine:  [done] / in progress / pending
  Plan:    [done] / in progress / pending
  Code:    [done] / 3/5 tasks / verifying / pending
```

### 2.6.3: Confirmation Prompt

Ask user (consult `ask-user`):
- **Continue**: reference specific task or phase
- **Restart**: start phase fresh

### 2.6.4: Restart Handling

If restart: proceed to phase without resumption context. Log `user-restart | phase: <phase>`.

### 2.6.5: Autonomous Mode Handling

If `autonomous_mode = true`: print resume banner, skip confirmation, auto-continue.

### 2.6.6: Completed Item Handling

If phase is `complete`:
- Print completion status
- Ask: "Re-run verification?"
- If yes: set phase override to `code`
