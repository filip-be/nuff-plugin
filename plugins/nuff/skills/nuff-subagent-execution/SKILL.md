---
name: nuff-subagent-execution
description: Platform-aware subagent spawner for nuff workflows. Use this skill whenever you need to spawn a subagent with a specific model alias. Abstracts the difference between Claude Code (native Agent tool) and GitHub Copilot (dedicated model-specific agents).
---

# Nuff Subagent Execution

This skill handles subagent spawning in a platform-aware way. Different platforms require different approaches to run a subagent with a specific model — this skill provides the single reference for how to do it correctly.

## Why this skill exists

GitHub Copilot does not support specifying a model at agent invocation time. Instead, each Copilot agent has its model baked into its frontmatter. To run a nuff subagent with a given model on Copilot, you must invoke a pre-configured wrapper agent (`nuff-copilot-subagent-<alias>`) that sets the model and then delegates to the actual agent instructions passed as arguments.

Claude Code supports the `model` parameter on the Agent tool directly, so no wrapper is needed.

## Platform routing

| Platform               | Tool / Method                                                                                                    |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Claude Code (CLI)      | `Agent` tool with `subagent_type` set to the agent name and `model` set to the expanded model ID                |
| VS Code (Copilot Chat) | `runSubagent` tool — invoke `nuff-copilot-subagent-<alias>` with the full agent instructions as the argument     |
| GitHub Copilot CLI     | `task` tool — invoke `nuff-copilot-subagent-<alias>` with the full agent instructions as the argument            |

## Model alias → Copilot agent mapping

| Alias          | Copilot agent                        | Model             | Multiplier |
| -------------- | ------------------------------------ | ----------------- | ---------- |
| `haiku`        | `nuff-copilot-subagent-haiku`        | Claude Haiku 4.5  | 0.33×      |
| `sonnet`       | `nuff-copilot-subagent-sonnet`       | Claude Sonnet 4.6 | 1×         |
| `opus`         | `nuff-copilot-subagent-opus`         | Claude Opus 4.6   | 3×         |
| `gpt-mini`     | `nuff-copilot-subagent-gpt-mini`     | GPT-5 mini        | 0×         |
| `gpt-standard` | `nuff-copilot-subagent-gpt-standard` | GPT-5.2           | 1×         |
| `gpt-codex`    | `nuff-copilot-subagent-gpt-codex`    | GPT-5.3-Codex     | 1×         |
| `gemini`       | `nuff-copilot-subagent-gemini`       | Gemini 3.1 Pro    | 1×         |

> Multipliers are per-request costs relative to the Copilot base rate. Source: [GitHub Copilot model multipliers](https://docs.github.com/en/copilot/concepts/billing/copilot-requests#model-multipliers)

### Copilot model enforcement limitation

**Copilot silently ignores the `model:` frontmatter in a subagent if that model is more expensive than the current session model.** For example, if the user's session runs on GPT-5.4 (1×) and a subagent is configured for Claude Opus 4.6 (3×), Copilot will run the subagent on GPT-5.4 instead — with no error or indication.

**Before invoking a Copilot wrapper agent**, compare the alias multiplier against the session model multiplier:

- If the subagent multiplier is **greater than** the session model multiplier → print a warning before invoking:

  ```
  ⚠ Warning: nuff is configured to run <agent-name> on <alias> (<multiplier>×), but your
  Copilot session model has a lower multiplier. Copilot will ignore the model setting and
  run this subagent on your session model instead. Results may differ from the intended
  model capability. To fix this, either start your session on a higher-multiplier model
  or lower the model for <config-key> in your .nuff/config.json.
  ```

- If the subagent multiplier is **equal to or less than** the session model multiplier → proceed silently. Do NOT print anything about multipliers, model enforcement, or "no warning needed". Just invoke the wrapper agent with zero commentary about this check.

If you cannot determine the session model multiplier, err on the side of printing the warning for `opus` alias (3×) whenever the session appears to be a non-premium model.

## Model alias → Claude Code model ID mapping

| Alias          | Claude Code model ID | Notes                                      |
| -------------- | -------------------- | ------------------------------------------ |
| `haiku`        | `haiku`              | Fast/cheap, Haiku                          |
| `sonnet`       | `sonnet`             | Balanced, Sonnet                           |
| `opus`         | `opus`               | Most capable, Opus                         |
| `gpt-mini`     | `haiku`              | Fast/cheap, Haiku used instead of GPT      |
| `gpt-standard` | `opus`               | Balanced, Opus used instead of GPT         |
| `gpt-codex`    | `sonnet`             | Code-optimized, Sonnet used instead of GPT |
| `gemini`       | `sonnet`             | Sonnet used instead of GPT                 |

## How to use

When a nuff workflow says to spawn a subagent with a specific model alias:

1. **Determine the resolved model alias** from `CONFIG_MODELS` (e.g., `CONFIG_MODELS.implementer` → `"sonnet"`).

2. **On Claude Code:**

   Use the Agent tool directly with the resolved model ID:

   ```
   Agent tool:
     subagent_type: <agent-name>   (e.g., "nuff-implementer")
     model: <resolved-model-id>    (e.g., "sonnet")
     prompt: <full agent prompt>
   ```

3. **On GitHub Copilot:**

   Invoke the appropriate wrapper agent, passing the full agent instructions from the agent's `.md` file as the argument:

   ```
   @nuff-copilot-subagent-<alias> <full content of the agent instructions + all input data>
   ```

   Example: to run `nuff-implementer` with `sonnet` alias:

   ```
   @nuff-copilot-subagent-sonnet [paste full nuff-implementer instructions here, followed by the task-specific input]
   ```

## Important notes

- **Always resolve the alias first.** `CONFIG_MODELS` stores aliases (e.g., `"sonnet"`), not raw model IDs. Look up the alias in the mapping table above before calling the Agent tool or selecting the Copilot wrapper.
- **On Copilot, NEVER call `runSubagent` (VS Code Chat) or `task` (Copilot CLI) with bare nuff agent names** (e.g., `nuff-implementer`, `nuff-ado-process-retriever`, `nuff-verifier`, `nuff-gap-analyzer`, `nuff-codebase-analyzer`). These agents are NOT registered in the Copilot plugin and are NOT available via `runSubagent`/`task`. The ONLY way to spawn them on Copilot is through the `nuff-copilot-subagent-<alias>` wrappers listed above, passing the agent's full instructions as the argument.
- **On Claude Code, the `model` parameter is optional.** If `CONFIG_MODELS` returns a raw model ID that is not in the alias table, pass it through directly to the Agent tool.
- **On Copilot, if the alias is not in the mapping table**, fall back to `nuff-copilot-subagent-sonnet` and log a warning.
