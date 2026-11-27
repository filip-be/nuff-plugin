# nuff-hooks-copilot

Good enough guardrail hooks for **GitHub Copilot CLI only**: they use the Copilot CLI hook format and do not run in other agents. They work on Windows, Linux and macOS.

## Terms

- **Workspace** — the project directory the agent session started in (`COPILOT_PROJECT_DIR`, else the working directory), and everything below it. For a session in a git worktree, that is the worktree root.
- **Agent Dirs** — the agent's own home configuration: `~/.claude`, `~/.claude.json` and `~/.copilot`.
- **Temp** — the OS temp directory (`TEMP` / `TMP` / `TMPDIR`, plus `/tmp` on Linux and macOS), including the agent's scratchpad inside it.
- **Read Roots** — extra directories you let the agent read, listed in `~/.nuff-hooks.json` (see [Read Roots](#read-roots)).
- **Read Zone** — everywhere the agent may read: the Workspace, the `D:\` drive (Windows only), Read Roots, Agent Dirs and Temp.
- **Write Zone** — everywhere the agent may write: the Workspace, Trash, Agent Dirs and Temp.
- **Trash** — the folder where removed files are kept instead of being deleted: `D:\.slop-trash` on Windows, `~/.slop-trash` on Linux and macOS.
- **Removal** — a shell delete command (`rm`, `rmdir`, `unlink`, `shred`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
- **Rewrite** — replacing a Removal, before it runs, with a move of the same targets into Trash.
- **Cloud Session** — a Claude Code session in a sandboxed cloud container (`CLAUDE_CODE_REMOTE=true`). The checks in this plugin also serve Claude Code and relax the Read Zone there, but Copilot CLI never sets that variable, so it changes nothing here.

## What it does

A `preToolUse` hook on `view`, `create`, `edit`, `apply_patch`, `grep`, `rg`, `glob`, `bash` and `powershell`:

| Rule | Behaviour |
| ---- | --------- |
| Read Zone | Reads only from the Workspace, `D:\` (Windows only), the [Read Roots](#read-roots) you configure, the agent's home dirs (`~/.claude`, `~/.copilot`) and the temp dir |
| Write Zone | Writes only to the Workspace (`COPILOT_PROJECT_DIR`), Trash, the agent's home dirs and the temp dir |
| Removal | A standalone `rm` / `Remove-Item` / `del` ... is rewritten into a move to `<Trash>/<workspace>/<session>/<original path>`; other removals are denied with a hint |
| Evasion | `iex`/`eval`, encoded commands, unresolvable variables, UNC paths and nested shells deeper than one level are denied |
| Failure | Any internal error denies the call |

Allowed calls get no decision, so your normal permission prompts still apply.

## Read Roots

To let the agent read elsewhere (say your projects live in `C:\DEV`, or `/opt/src` on Linux), list extra directories in `~/.nuff-hooks.json` (`%USERPROFILE%\.nuff-hooks.json` on Windows):

```json
{
    "readRoots": ["C:\\DEV", "~/src"]
}
```

On Linux and macOS the Read Zone has no default extra root, so system paths such as `/usr` or `/etc` are denied until you list them:

```json
{
    "readRoots": ["/opt/src", "~/src", "/usr/include"]
}
```

- Entries must be absolute paths for the platform (`C:\DEV` or `C:/DEV` on Windows, `/opt/src` on Linux and macOS) or start with `~`; each root covers everything below it. They only widen the Read Zone, never the Write Zone.
- The file is read on every tool call, so changes apply at once. It lives outside the Write Zone on purpose: the agent can't edit it.
- If the file is not valid JSON or any entry is not an absolute path, every configured root is ignored and denied reads say why.

## Prerequisites

- Python 3.10+: on Windows the `py` launcher (`py -3 --version`), on Linux and macOS `python3` on `PATH` (`python3 --version`). The hook uses `py` when it finds it, else `python3`; with neither, every checked call is denied. Copilot runs the hook's `powershell` command on Windows and its `bash` command on Linux and macOS; both follow these same steps.

## Known gaps

It guards against accidents, not a determined agent: scripts that delete files indirectly (`python cleanup.py`), commands piped into a shell, `git clean` / `git reset --hard`, files written by package managers and MCP tools are not checked. `apply_patch` file deletes are denied with a hint to use a shell removal.

A `cd` inside a subshell (`$( … )`, backticks, `( … )`) is treated as if it changed the directory for the rest of the command, so ``cd D:/other && echo `cd D:/work/proj` && touch x`` is checked against the wrong directory.
