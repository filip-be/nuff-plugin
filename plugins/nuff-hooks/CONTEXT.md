# nuff-hooks

Guardrail hooks that keep an honest coding agent (Claude Code, GitHub Copilot CLI) from accidentally reading, writing or deleting outside the places it should touch.

## Language

### Places

**Workspace**:
The project directory the agent session started in, and everything below it. For a session in a git worktree, that is the worktree root.
_Avoid_: project, repo, root

**Agent Dirs**:
The agent's own home configuration directories (e.g. `~/.claude`, `~/.copilot`).
_Avoid_: config dirs, home

**Temp**:
The OS temp directory, including the agent's scratchpad inside it.
_Avoid_: scratch

**Trash**:
The folder `D:\.slop-trash`, where removed files are kept instead of being deleted.
_Avoid_: recycle bin, bin

**Read Zone**:
Everywhere an agent may read: the `D:\` drive, Agent Dirs and Temp.

**Write Zone**:
Everywhere an agent may write: the Workspace, Trash, Agent Dirs and Temp.

### Actions

**Removal**:
A shell delete command (`rm`, `rmdir`, `unlink`, `del`, `erase`, `rd`, `Remove-Item` and its aliases) aimed at files or directories. Git commands and overwriting moves are not Removals.
_Avoid_: delete, cleanup

**Rewrite**:
Replacing a Removal, before it runs, with a move of the same targets into Trash.
_Avoid_: redirect, intercept
