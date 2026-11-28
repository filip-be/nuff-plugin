# ai-plugins

A plugin marketplace for AI coding assistants — **Claude Code** and **GitHub Copilot** (VS Code & CLI).

## Available plugins

### [nuff-skills](./plugins/nuff-skills)

Set of skills that are good enough to improve your day-to-day AI workflow (initially, a fork of [mattpocock/skills](https://github.com/mattpocock/skills)).

The workflow is three steps: **grill** the idea into a plan, **implement** it, open the **PR**. Before you see the plan or the finished code, a model from a different provider reviews it in a loop until it has no findings left.

See the [nuff-skills README](./plugins/nuff-skills/README.md) for the workflow diagram, the review loops, when to clear the context and the full list of skills.

### nuff-hooks-cc / nuff-hooks-copilot

Good enough guardrail hooks, one plugin per platform: [nuff-hooks-cc](./plugins/nuff-hooks-cc/README.md) for Claude Code, [nuff-hooks-copilot](./plugins/nuff-hooks-copilot/README.md) for GitHub Copilot CLI. Install only the one matching your agent.

- reads only from the Workspace, `D:\` (Windows only) and the roots you list in `~/.nuff-hooks.json` (plus the agent's home dirs and the temp dir)
- writes only to the Workspace (plus Trash, the agent's home dirs and the temp dir)
- file removals are rewritten into moves to Trash: `D:\.slop-trash` on Windows, `~/.slop-trash` on Linux and macOS

Works on Windows (with the `py` launcher), Linux and macOS (with `python3`); needs Python 3.10+.

### nuff-pulse-cc / nuff-pulse-copilot

Hooks that record each agent session's state (working, needs you, done, error, ended) to `~/.nuff-pulse/sessions/`, one plugin per platform: [nuff-pulse-cc](./plugins/nuff-pulse-cc/README.md) for Claude Code, [nuff-pulse-copilot](./plugins/nuff-pulse-copilot/README.md) for GitHub Copilot (CLI, app and VS Code's agent host). Their reader is [nuff-pulse](./apps/nuff-pulse/README.md), a small Windows widget that lists your active local sessions with state, context size and prompt-cache countdown; download it from the [releases](https://github.com/filip-be/nuff-plugin/releases) (tags `apps/nuff-pulse/v*`).

### nuff-cc

Good enough mods for **Claude Code only**: a desktop status line (context size, prompt-cache countdown, session cost); **Dumb swimmer**, which nudges the agent toward a stopping point from 300k tokens of context and, from 500k, to write a handoff and offer you `/clear`; and opening the current session's folder in VS Code from an **Open in VS Code** button above the prompt (desktop app) or `/vscode`.

See the [nuff-cc README](./plugins/nuff-cc/README.md) for details.

### [nuff](./plugins/nuff) (deprecated)

> [!WARNING]
> **nuff is deprecated.** Its four-phase Azure DevOps pipeline (Refine → Plan → Code → Improve, with `/do.nuff` and `/do-everything.nuff`) turned out to be too heavy for day-to-day work. It has been replaced by [nuff-skills](#nuff-skills), which keeps the useful parts (sharpening the plan with you, independent review of plan and code) in three lightweight steps.

The plugin is still listed in the marketplace for existing users. See the [nuff README](./plugins/nuff/README.md) for its documentation.

## Installation

The marketplace is named `nuff-plugin` and lives in the GitHub repository `filip-be/nuff-plugin`. Add it, then install the plugins you want from it:

```
/plugin marketplace add filip-be/nuff-plugin
/plugin install nuff-skills@nuff-plugin
```
