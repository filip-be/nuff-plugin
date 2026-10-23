# nuff-hooks-cc

Good enough guardrail hooks for **Claude Code only** (the GitHub Copilot CLI flavour is [nuff-hooks-copilot](../nuff-hooks-copilot/README.md)). Terms are defined in the [glossary](../../CONTEXT.md#guardrail-hooks).

## What it does

A `PreToolUse` hook on `Read`, `Write`, `Edit`, `NotebookEdit`, `Glob`, `Grep`, `Bash` and `PowerShell`:

| Rule | Behaviour |
| ---- | --------- |
| Read Zone | Reads only from `D:\`, the agent's home dirs (`~/.claude`, `~/.copilot`) and the temp dir |
| Write Zone | Writes only to the Workspace (`CLAUDE_PROJECT_DIR`), `D:\.slop-trash`, the agent's home dirs and the temp dir |
| Removal | A standalone `rm` / `Remove-Item` / `del` ... is rewritten into a move to `D:\.slop-trash\<workspace>\<session>\<original path>`; other removals are denied with a hint |
| Evasion | `iex`/`eval`, encoded commands, unresolvable variables, UNC paths and nested shells deeper than one level are denied |
| Failure | Any internal error denies the call |

Allowed calls get no decision, so your normal permission prompts still apply.

## Prerequisites

- Windows, with the `py` launcher and Python 3.10+ (`py -3 --version`)

## Known gaps

It guards against accidents, not a determined agent: scripts that delete files indirectly (`python cleanup.py`), commands piped into a shell, `git clean` / `git reset --hard`, files written by package managers and `@`-mentioned files are not checked.

A `cd` inside a subshell (`$( … )`, backticks, `( … )`) is treated as if it changed the directory for the rest of the command, so ``cd D:/other && echo `cd D:/work/proj` && touch x`` is checked against the wrong directory.
