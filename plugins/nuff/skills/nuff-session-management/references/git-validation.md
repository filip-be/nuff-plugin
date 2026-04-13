# Git Operation Validation Protocol

Run these checks after every `git add` + `git commit` sequence in any nuff workflow.

## After `git add`

Run `git diff --cached --name-only` and verify:
- All expected files appear in the staged list.
- No unexpected files are staged.

If a file is missing from staged output: re-stage it explicitly and check again.

## After `git commit`

Run `git log -1 --format="%h %s"` and verify:
- A commit hash is returned (the commit actually happened).
- The commit message matches what was intended.

## On failure

1. Log `git-fail | <operation> — <reason>` to the session log via `nuff-state-update` with `append-session-log`.
2. For staging failures: retry once with explicit file paths.
3. For commit failures (hook error, empty staging, etc.): present the error to the user with options:
   - **Retry** — re-stage and re-commit
   - **Skip commit** — continue without committing (note: artifacts uncommitted)
   - **Stop** — halt the workflow for manual intervention
4. **Never silently proceed past a failed commit.** An uncommitted artifact is invisible to resume detection and will be lost on context reset.
