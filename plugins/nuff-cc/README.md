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

### Status line

The **Open in VS Code** band in the Claude desktop app doubles as a status line: the figures on the left, the button on the right.

```
173k · ◷ 53m                                   [Open in VS Code]
```

Desktop app only, hidden while a survey shows. The terminal has its own `statusLine`; the VS Code extension shows the cache itself.

| Segment | Means | Colour |
| ------- | ----- | ------ |
| `173k` | input tokens the next request re-sends (thousands, rounded down; under 1k the plain number) | default; `warning` at ≥ 300k or ≥ 60% of the context window; `error` at ≥ 500k or ≥ 80% |
| `◷ 53m` | whole minutes left on the main conversation's prompt cache | `success` above 15m; `warning` from 15m down to `<1m` |
| `◷ <1m` | under a minute left | `warning` |
| `cold` | the cache has expired or was invalidated | `error` |
| `cache off` | `DISABLE_PROMPT_CACHING` is set | dim |

Each segment is hidden until it has a figure; ` · ` separates them only when both show.

How the figures move:

- **Context** — seeded at session start (so a resumed or forked session shows it at once), then updated after every main-thread turn.
- **Cache countdown** — restarts at 60 minutes whenever a main-thread model request reads or writes the prompt cache. Subagent requests are ignored: they cache a different prefix. While a turn runs, the last countdown stays. The line redraws once a minute.
- **Resume / fork** — the countdown starts from the resumed transcript's last response; `cold` when Claude Code says the cache has likely expired.
- **`/clear`** — both segments hide until new figures arrive.
- **`/compact`** — `cold` until the next request that caches.
- **Model switch** — `cold` (the new model has no cache of this conversation); restoring the model on resume does not count.

Limitations:

- The TTL is **hardcoded to 60 minutes**, not detected. It is 5 minutes with an API key, Bedrock, Vertex or Foundry, once a subscription draws on usage credits, or with `FORCE_PROMPT_CACHING_5M`; Enterprise is unverified. In those cases the countdown is too optimistic (a resume still turns `cold` correctly, since it uses Claude Code's own TTL).
- Claude Code reports the real `cache_ttl` only when the model is switched, so the mod cannot learn it up front.
- Rewind is not handled: the countdown keeps running from the last request.

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
