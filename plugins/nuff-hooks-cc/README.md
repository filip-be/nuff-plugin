# nuff-hooks-cc

Good enough guardrail hooks for **Claude Code only**: they use Claude Code's hook format and do not run in other agents. They work on Windows, Linux and macOS.

## Terms

- **Workspace** — the project directory the agent session started in (`CLAUDE_PROJECT_DIR`, else the working directory), and everything below it. For a session in a git worktree, that is the worktree root.
- **Agent Dirs** — the agent's own home configuration: `~/.claude`, `~/.claude.json` and `~/.copilot`.
- **Temp** — the OS temp directory (`TEMP` / `TMP` / `TMPDIR`, plus `/tmp` on Linux and macOS), including the agent's scratchpad inside it.
- **Read Roots** — extra directories you let the agent read, listed in `~/.nuff-hooks.json` (see [Read Roots](#read-roots)).
- **Cloud Session** — a Claude Code session that runs in a sandboxed, throwaway cloud container rather than on your machine: Claude Code on the web (claude.ai/code) and chats on claude.ai that run code. Claude Code marks it with `CLAUDE_CODE_REMOTE=true`; the hooks also recognise it by `CLAUDE_CODE_REMOTE_SESSION_ID` or `CLAUDE_CODE_CONTAINER_ID`.
- **Cloud Dirs** — extra directories Claude Code gives a Cloud Session in `CLAUDE_ADDITIONAL_DIRECTORIES`, such as a web chat's `/mnt/user-data` (uploads and outputs).
- **Read Zone** — everywhere the agent may read: the Workspace, the `D:\` drive (Windows only), Read Roots, Agent Dirs and Temp. In a Cloud Session: everywhere.
- **Write Zone** — everywhere the agent may write: the Workspace, Trash, Agent Dirs and Temp, plus the Cloud Dirs in a Cloud Session.
- **Trash** — the folder where removed files are kept instead of being deleted: `D:\.slop-trash` on Windows, `~/.slop-trash` on Linux and macOS.
- **Removal** — a shell delete command (`rm`, `rmdir`, `unlink`, `shred`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
- **Rewrite** — replacing a Removal, before it runs, with a move of the same targets into Trash.

## What it does

A `PreToolUse` hook on `Read`, `Write`, `Edit`, `NotebookEdit`, `Glob`, `Grep`, `Bash` and `PowerShell`:

| Rule | Behaviour |
| ---- | --------- |
| Read Zone | Reads only from the Workspace, `D:\` (Windows only), the [Read Roots](#read-roots) you configure, the agent's home dirs (`~/.claude`, `~/.copilot`) and the temp dir |
| Write Zone | Writes only to the Workspace (`CLAUDE_PROJECT_DIR`), Trash, the agent's home dirs and the temp dir |
| Removal | A standalone `rm` / `Remove-Item` / `del` ... is rewritten into a move to `<Trash>/<workspace>/<session>/<original path>`; other removals are denied with a hint |
| Evasion | `iex`/`eval`, encoded commands, unresolvable variables, UNC paths and nested shells deeper than one level are denied |
| Failure | Any internal error denies the call |

Allowed calls get no decision, so your normal permission prompts still apply.

## Cloud Sessions

A Cloud Session already runs in a sandbox that is thrown away afterwards, so the hooks guard it lightly:

- **Reads are not checked**: any path may be read, including system paths and paths behind variables the hook can't resolve.
- **Writes** still stay in the Write Zone, which also covers the [Cloud Dirs](#terms) (a web chat writes its deliverables to `/mnt/user-data/outputs`).
- **Removals** are still moved to Trash (`~/.slop-trash`, inside the container), and the evasion rules still apply.

Nothing needs to be configured: the hooks read the variables above and `CLAUDE_ADDITIONAL_DIRECTORIES`, which Claude Code sets in the session. `~/.nuff-hooks.json` is not needed there.

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

- Python 3.10+: on Windows the `py` launcher (`py -3 --version`), on Linux and macOS `python3` on `PATH` (`python3 --version`). The hook uses `py` when it finds it, else `python3`; with neither, every checked call is denied.
- On Windows, Git Bash. Claude Code runs this hook's command in Git Bash; without Git Bash it falls back to PowerShell, where the command does not parse, so every checked call is blocked.

## Known gaps

It guards against accidents, not a determined agent: scripts that delete files indirectly (`python cleanup.py`), commands piped into a shell, `git clean` / `git reset --hard`, files written by package managers and `@`-mentioned files are not checked.

A `cd` inside a subshell (`$( … )`, backticks, `( … )`) is treated as if it changed the directory for the rest of the command, so ``cd D:/other && echo `cd D:/work/proj` && touch x`` is checked against the wrong directory.
