# AI Agents Guide

This repository is a plugin marketplace configured for **both Claude Code and GitHub Copilot**. When creating or modifying a plugin, changes must be reflected in multiple files across both platforms.

## Marketplace definitions

The marketplace (the list of available plugins) is defined in two places that must stay in sync:

| Platform | File |
| -------- | ---- |
| Claude Code | `.claude-plugin/marketplace.json` |
| GitHub Copilot | `.github/plugin/marketplace.json` |

Each file lists the plugins available in this marketplace. When adding or removing a plugin, update both files.

**Key difference:** the Copilot marketplace includes a top-level `"pluginRoot": "./plugins/"` field and uses bare plugin names as `source` values (e.g., `"nuff"`), while the Claude marketplace uses full relative paths (e.g., `"./plugins/nuff"`).

## Plugin definitions

Each plugin has two definition files that must have matching metadata (`name`, `version`, `description`, `author`, `homepage`, `repository`, `license`, `keywords`):

| Platform | File |
| -------- | ---- |
| Claude Code | `plugins/<plugin>/.claude-plugin/plugin.json` |
| GitHub Copilot | `plugins/<plugin>/plugin.json` |

### Agent lists — the one difference

The `agents` field is intentionally different between the two files:

- **Claude Code** (`plugins/<plugin>/.claude-plugin/plugin.json`) expects an **explicit list of agent file paths**:
  ```json
  "agents": [
    "./agents/nuff-codebase-analyzer.md",
    "./agents/nuff-gap-analyzer.md",
    "./agents/nuff-implementer.md",
    "./agents/nuff-verifier.md"
  ]
  ```

- **Copilot** (`plugins/<plugin>/plugin.json`) expects a **list of directory paths** where agents live:
  ```json
  "agents": [
    "agents/",
    "agents/copilot/"
  ]
  ```

When adding or removing an agent file, update the explicit list in `.claude-plugin/plugin.json`. The Copilot `plugin.json` only needs updating if agents are moved to a new directory.

## Checklist for common operations

### Adding a new plugin

- [ ] Add entry to `.claude-plugin/marketplace.json`
- [ ] Add entry to `.github/plugin/marketplace.json`
- [ ] Create `plugins/<plugin>/.claude-plugin/plugin.json` with explicit agent file list
- [ ] Create `plugins/<plugin>/plugin.json` with agent directory list

### Adding a new agent to an existing plugin

- [ ] Add the agent file path to `plugins/<plugin>/.claude-plugin/plugin.json` → `agents` array
- [ ] No change needed in `plugins/<plugin>/plugin.json` unless the agent lives in a new directory

### Updating plugin metadata (version, description, etc.)

- [ ] Update `plugins/<plugin>/.claude-plugin/plugin.json`
- [ ] Update `plugins/<plugin>/plugin.json` — must match

## Tools reference — canonical sources

Plugin instructions (agents, skills, commands) reference platform-specific tool names. These names change across releases, so **always verify tool names against the canonical sources below** before authoring or reviewing any instruction file.

> **Rule:** Every tool name used in a plugin `.md` file must exist in the corresponding platform's canonical tool list. When updating plugin files, cross-check tool names against these sources and fix any that are stale or incorrect.

### Claude Code

Canonical reference: <https://code.claude.com/docs/en/tools-reference>

Current tool names (verify against the link above):

`Agent`, `AskUserQuestion`, `Bash`, `CronCreate`, `CronDelete`, `CronList`, `Edit`, `EnterPlanMode`, `EnterWorktree`, `ExitPlanMode`, `ExitWorktree`, `Glob`, `Grep`, `ListMcpResourcesTool`, `LSP`, `NotebookEdit`, `PowerShell`, `Read`, `ReadMcpResourceTool`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`, `TaskOutput`, `TaskStop`, `TaskUpdate`, `TodoWrite`, `ToolSearch`, `WebFetch`, `WebSearch`, `Write`

### GitHub Copilot (VS Code extension)

Built-in tools are defined in VS Code source — check the `tools.ts` file for the registered list and individual files for details:
<https://github.com/microsoft/vscode/tree/main/src/vs/workbench/contrib/chat/common/tools/builtinTools>

Key built-in tool IDs (verify against the source above):

`vscode_editFile`, `vscode_askQuestions`, `manage_todo_list`, `runSubagent`, `task_complete`

Copilot also exposes shell and file tools via the CLI runtime — see the CLI reference below.

### GitHub Copilot CLI

Canonical reference: <https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference>

Current tool names (verify against the link above):

| Category | Tools |
| --- | --- |
| Shell | `bash`, `powershell`, `read_bash`, `read_powershell`, `write_bash`, `write_powershell`, `stop_bash`, `stop_powershell`, `list_bash`, `list_powershell` |
| File operations | `view`, `create`, `edit`, `apply_patch` |
| Search | `grep`, `rg`, `glob` |
| Agent/Task | `task`, `read_agent`, `list_agents`, `update_todo`, `task_complete` |
| User interaction | `ask_user`, `report_intent`, `show_file`, `skill` |
| Other | `web_fetch`, `exit_plan_mode`, `store_memory`, `fetch_copilot_cli_documentation`, `lsp`, `sql` |
