# nuff-hooks-cc

Good enough guardrail hooks for **Claude Code only**: they use Claude Code's hook format and do not run in other agents.

## Terms

- **Workspace** — the project directory the agent session started in (`CLAUDE_PROJECT_DIR`, else the working directory), and everything below it. For a session in a git worktree, that is the worktree root.
- **Agent Dirs** — the agent's own home configuration: `~/.claude`, `~/.claude.json` and `~/.copilot`.
- **Temp** — the OS temp directory (`TEMP` / `TMP`), including the agent's scratchpad inside it.
- **Read Roots** — extra directories you let the agent read, listed in `~/.nuff-hooks.json` (see [Read Roots](#read-roots)).
- **Read Zone** — everywhere the agent may read: the Workspace, the `D:\` drive, Read Roots, Agent Dirs and Temp.
- **Write Zone** — everywhere the agent may write: the Workspace, Trash, Agent Dirs and Temp.
- **Trash** — the folder `D:\.slop-trash`, where removed files are kept instead of being deleted.
- **Removal** — a shell delete command (`rm`, `rmdir`, `unlink`, `shred`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
- **Rewrite** — replacing a Removal, before it runs, with a move of the same targets into Trash.

## What it does

A `PreToolUse` hook on `Read`, `Write`, `Edit`, `NotebookEdit`, `Glob`, `Grep`, `Bash` and `PowerShell`:

| Rule | Behaviour |
| ---- | --------- |
| Read Zone | Reads only from the Workspace, `D:\`, the [Read Roots](#read-roots) you configure, the agent's home dirs (`~/.claude`, `~/.copilot`) and the temp dir |
| Write Zone | Writes only to the Workspace (`CLAUDE_PROJECT_DIR`), `D:\.slop-trash`, the agent's home dirs and the temp dir |
| Removal | A standalone `rm` / `Remove-Item` / `del` ... is rewritten into a move to `D:\.slop-trash\<workspace>\<session>\<original path>`; other removals are denied with a hint |
| Evasion | `iex`/`eval`, encoded commands, unresolvable variables, UNC paths and nested shells deeper than one level are denied |
| Failure | Any internal error denies the call |

Allowed calls get no decision, so your normal permission prompts still apply.

## Read Roots

To let the agent read outside `D:\` (say your projects live in `C:\DEV`), list extra directories in `~/.nuff-hooks.json` (`%USERPROFILE%\.nuff-hooks.json`):

```json
{
    "readRoots": ["C:\\DEV", "~/src"]
}
```

- Entries must be absolute paths (`C:\DEV`, `C:/DEV`) or start with `~`; each root covers everything below it. They only widen the Read Zone, never the Write Zone.
- The file is read on every tool call, so changes apply at once. It lives outside the Write Zone on purpose: the agent can't edit it.
- If the file is not valid JSON or any entry is not an absolute path, every configured root is ignored and denied reads say why.

## Prerequisites

- Windows, with the `py` launcher and Python 3.10+ (`py -3 --version`)

## Known gaps

It guards against accidents, not a determined agent: scripts that delete files indirectly (`python cleanup.py`), commands piped into a shell, `git clean` / `git reset --hard`, files written by package managers and `@`-mentioned files are not checked.

A `cd` inside a subshell (`$( … )`, backticks, `( … )`) is treated as if it changed the directory for the rest of the command, so ``cd D:/other && echo `cd D:/work/proj` && touch x`` is checked against the wrong directory.
