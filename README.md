# ai-plugins

A plugin marketplace for AI coding assistants — **Claude Code** and **GitHub Copilot** (VS Code & CLI).

## Available plugins

### [nuff](./plugins/nuff)

> Take an Azure DevOps work item and do _enough_ work to get it done.

**nuff** turns Azure DevOps work items into implemented, verified code through a structured four-phase pipeline:

```
Azure DevOps Work Item
        │
        ▼
   ┌─────────┐     Clarify requirements,
   │  Refine │ ──► identify gaps, produce SPEC.md
   └────┬────┘
        │  ✓ User approves spec
        ▼
   ┌─────────┐     Decompose into tasks,
   │   Plan  │ ──► map to files, produce PLAN.md
   └────┬────┘
        │  ✓ User approves plan
        ▼
   ┌─────────┐     Implement tasks, verify against spec,
   │   Code  │ ──► fix issues, mark SPEC.md requirements
   └────┬────┘
        │  ✓ Code verified
        ▼
   ┌─────────┐     Review reflections + lessons learned,
   │ Improve │ ──► update AGENTS.md and knowledge base
   └─────────┘
```

**Key commands:**

| Command                     | Description                                          |
| --------------------------- | ---------------------------------------------------- |
| `/do.nuff 12345`            | Run the full interactive workflow for a work item    |
| `/do-everything.nuff 12345` | Autonomous end-to-end run (no user interaction)      |
| `/improve.nuff 12345`       | Run just the improve phase for a completed work item |
| `/configure.nuff`           | Configure nuff settings and model selection          |

See the [nuff README](./plugins/nuff/README.md) for full documentation.

---

## Installation

### Claude Code (CLI)

1. Open Claude Code and navigate to the Plugins section via `/plugin`
2. Add `nuff-plugin` as new marketplace
3. Browse marketplace plugins and install desired ones (e.g. nuff)

To update plugins installed from the marketplace, use `/plugin` command to check for updates and apply them. You can also manage plugins (e.g. uninstall, view details) from the same interface.

### GitHub Copilot CLI

1. Open Copilot CLI
2. See available commands via `/plugin`
3. Install plugin marketplace via `/plugin marketplace add nuff-plugin`
4. Install desired plugins from the marketplace, e.g. `/plugin install nuff@nuff-plugin`
5. To update plugins installed from the marketplace, use `/plugin update nuff@nuff-plugin` to check for updates and apply them.

### GitHub Copilot Chat in VS Code

As of 2025-03-13, Chat Plugins as in Preview and Plugins Marketplace is an experimental feature and needs to be enabled manually in VS Code settings.

1. Open User settings via `Ctrl+P` and `> Preferences: Open User Settings`
2. Search for `chat plugin`
3. Enable ` Chat > Plugins: Enable` setting
4. Add `nuff-plugin` as new marketplace in the `Chat > Plugins:Marketplaces`

Once done, you can go to Extensions view and search plugins via `@agentPlugins` filter, e.g. `@agentPlugins nuff` to find the nuff plugin and install it.

Plugins should be updated as standard VS Code extensions as per [microsoft/vscode: Test: plugin support](https://github.com/microsoft/vscode/issues/301158).

If that doesn't work, you can go to plugin folder and pull the latest changes from GitHub via `git pull` to update the whole marketplace and get the latest version of the plugins.
