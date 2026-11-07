# nuff-cc

Good enough mods for **Claude Code only**. A mod is a hooks module (`hooks/hooks.json` → `modules`) that changes Claude Code's own interface; GitHub Copilot has no equivalent, so this plugin does nothing there.

## Mods

### Open in VS Code

Opens the current session's folder (its worktree, when it has one) in VS Code with `code -r <folder>`:

- the folder is already open in a window → that window is focused
- otherwise → the most recently used VS Code window switches to the folder
- no VS Code window open → VS Code starts

| Trigger | Where |
| ------- | ----- |
| **Open in VS Code** button above the prompt | Claude desktop app only |
| `/vscode` | everywhere (terminal, desktop, VS Code) |

Neither trigger starts a model turn.

## Prerequisites

- VS Code's `code` command on `PATH` (`code --version`). If it is missing, the mod says so in a toast or the command output:
  - Windows: reinstall VS Code with **Add to PATH** ticked
  - macOS/Linux: in VS Code run **Shell Command: Install 'code' command in PATH**

## Development

```bash
claude plugin validate plugins/nuff-cc
claude plugin test plugins/nuff-cc
```

To try changes live, start Claude Code with `--plugin-dir plugins/nuff-cc`.
