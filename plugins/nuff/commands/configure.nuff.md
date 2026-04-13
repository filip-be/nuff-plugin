---
name: configure.nuff
description: Configure nuff framework settings -- model selection, behavior toggles, ADO project defaults
argument-hint: "[key=value ...] [--show]"
---

# configure.nuff - Configuration Manager

You are the nuff configuration manager. Your job is to display the current configuration with field-level provenance (showing which values match defaults vs. which are customised), and either apply flag-based settings or guide the user through an interactive wizard. Follow these steps in order.

**Plugin directory:** All plugin-internal files (`nuff-defaults.json`, skills, agents) are read from the plugin's install directory — NOT from the current working directory. Use the plugin directory for all such file reads.

## Step 1: Parse Arguments

Parse `$ARGUMENTS` for the following:

- `--show` flag: if present, display the effective config and exit without entering the wizard. Store as `MODE=show`.
- `key=value` pairs: any argument containing `=` (and not starting with `--`) is a key-value pair. Collect all such pairs into a list. If any key-value pairs are found, store `MODE=flag`.
- If neither `--show` nor any `key=value` pairs are found, store `MODE=wizard`.

The write target is always `.nuff/config.json` (repo-level config).

## Step 2: Load Config Layers

Load the two config layers using the Read tool.

**Layer 1 - Defaults (required):**

Read `nuff-defaults.json` from the plugin directory.

If this file cannot be read, STOP immediately with the error:

```
Error: Cannot read nuff-defaults.json from the plugin directory. Is the nuff plugin installed correctly?
```

Parse the JSON. Store as `DEFAULTS`.

**Layer 2 - Repo config:**

Read `.nuff/config.json`. **Important:** This file is gitignored, so `Glob` will not find it. Use the `Read` tool directly to check existence and load it — if the file does not exist, `Read` will return an error, which means this is a first-time setup.

**If the file does not exist:** This is a first-time setup. Proceed as follows:

1. Print:
   ```
   No .nuff/config.json found. Starting first-time configuration setup.
   ```
2. Build the initial config object by copying all non-`_`-prefixed keys from `DEFAULTS`, plus a top-level `"schema_version"` field set to `DEFAULTS._schema_version`. Store this as `REPO_CONFIG`.
3. Create the `.nuff/` directory if it does not exist.
4. Write `REPO_CONFIG` to `.nuff/config.json` with 2-space JSON indentation.
5. Set `MODE=wizard` (regardless of arguments — first-time setup always runs the full wizard).
6. Continue to Step 2.3 (gitignore setup), then proceed through the wizard to walk the user through all settings (models, behavior, ADO).

If the file cannot be read, STOP with the error:

```
Error: Cannot read .nuff/config.json.
```

If the file exists but contains invalid JSON, ask the user (consult the `ask-user` skill):

- **Reset to defaults**: Reset it by copying the defaults structure (strip `_`-prefixed keys) as a starting point
- **Abort**: Fix the file manually and re-run

If they choose reset, write the stripped defaults to the file (no `_`-prefixed keys) and set `REPO_CONFIG` to that value. If they choose abort, stop.

Store the parsed config as `REPO_CONFIG`.

## Step 2.3: Ensure config.json is gitignored

Before proceeding, ensure `.nuff/config.json` and visual verification output are excluded from version control via `.nuff/.gitignore`:

1. Check if `.nuff/.gitignore` exists.
2. If it does not exist, create it with the content:
   ```
   config.json
   */.visual-verification/
   ```
3. If it exists but does not contain a line matching `config.json`, append `config.json` to it.
4. If it exists but does not contain a line matching `*/.visual-verification/`, append `*/.visual-verification/` to it.

## Step 2.4: Schema Version Check

Compare the `_schema_version` value in `DEFAULTS` against the `schema_version` field (no underscore prefix) in `REPO_CONFIG`:

- If `REPO_CONFIG` is missing `schema_version` OR `REPO_CONFIG.schema_version !== DEFAULTS._schema_version`:
  - Print: `Note: Config schema version mismatch (config: <repo_value_or_"missing">, current: <defaults_value>). Proceeding with full validation.`
  - Continue to Step 2.5 (full schema validation will run).
- If versions match:
  - Print: `Config schema version OK (v<version>). Skipping schema validation.`
  - Skip Step 2.5 entirely and jump to Step 3.

## Step 2.5: Validate Repo Config Against Schema

After loading both layers, validate `REPO_CONFIG` against the `DEFAULTS` structure:

1. **Unknown key detection:** For every key path in `REPO_CONFIG`, verify the same key path exists in `DEFAULTS`. Collect all unknown key paths.

2. **Type mismatch detection:** For every leaf value in `REPO_CONFIG` where the key path exists in `DEFAULTS`, verify the type matches (string=string, bool=bool, object=object). Collect all mismatches.

3. **Model alias validation:** For every key under `models.*` in `REPO_CONFIG` that holds a string value, check if the value exists as a key in `DEFAULTS._aliases`. If it does NOT match a known alias AND does NOT match any of these patterns: `claude-*`, `gpt-*`, `gemini-*` -- collect as a warning.

4. **Report violations:** If any violations were found, print all of them as warnings:

```
Schema validation warnings:
- Unknown key path: `models.refine` (not in defaults schema)
- Type mismatch: `behavior.auto_commit` expected boolean, got string
- Unknown model alias: `models.implementer` = "unknown-model"
```

Then ask the user (consult the `ask-user` skill):

- **Fix interactively** -- enter the wizard for invalid keys
- **Ignore** -- proceed with defaults for invalid keys (invalid keys are dropped from the merge)

If the user chooses **Fix interactively**, enter the wizard (Step 8) pre-focused on the sections with invalid keys.

If the user chooses **Ignore**, remove invalid key paths from `REPO_CONFIG` before proceeding.

## Step 3: Compute Provenance

For each leaf key in `REPO_CONFIG` (excluding `_`-prefixed keys and `schema_version`), compare its value against the same key path in `DEFAULTS`:

- If the value **equals** the default value: mark as `default`
- If the value **differs** from the default: mark as `custom`

"Equals" means strict equality for primitives. For nested objects, compare leaf values individually.

## Step 5: Display Config with Provenance

Display the configuration as a markdown table. Only include non-`_`-prefixed keys from `REPO_CONFIG` (skip `schema_version`, `_comment`, `_version`, `_aliases`, and any other `_`-prefixed fields):

```
## Current Configuration

| Setting | Value | Status |
|---------|-------|--------|
| **Refine phase** | | |
| models.gap_analyzer.refine | claude-opus | default |
| **Plan phase** | | |
| models.codebase_analyzer.plan | claude-sonnet | default |
| models.gap_analyzer.plan | claude-opus | default |
| **Code phase** | | |
| models.implementer | claude-sonnet | default |
| models.gap_analyzer.code | claude-sonnet | default |
| models.verifier | claude-sonnet | default |
| behavior.auto_commit | true | default |
| behavior.verbose | false | default |
| behavior.branch_prefix | nuff | default |
| behavior.visual_verification | false | default |
| ado.project | | default |
```

(Actual values and statuses reflect the real current state.)

After the table, display the legend:

```
Status: **default** = same as nuff-defaults.json | **custom** = user-defined value
```

## Step 6: Branch by Mode

**If `MODE=show`:** Stop here. Configuration display is complete. Do not proceed to wizard or flag-based write.

**If `MODE=flag`:** Go to Step 7 (Flag-Based Write).

**If `MODE=wizard`:** Go to Step 8 (Interactive Wizard).

## Step 7: Flag-Based Write

For each `key=value` pair collected in Step 1:

**1. Parse the dot-notation key.** Split on `.` to get segments (e.g., `models.implementer` -> section `models`, field `implementer`; `models.gap_analyzer.refine` -> section `models`, subsection `gap_analyzer`, field `refine`).

**2. Validate the key exists in defaults.** Traverse `DEFAULTS` using the key path segments. If the key path does not resolve to a defined value in defaults, warn and skip:

```
Warning: Unknown config key: `<key>`. Skipping.
```

**3. Type coercion.** Look up the default value for this key:

- If the default is a boolean: coerce the provided value -- `"true"` -> `true`, `"false"` -> `false`. Any other string is invalid; warn and skip.
- If the default is a string: keep the value as a string.
- If the default is a number: attempt `parseFloat` / `parseInt` as appropriate.

**4. Model alias validation** (for keys under `models.*`):

Check if the value matches a known alias (exists as a key in `DEFAULTS._aliases`). If it does NOT match a known alias AND does NOT match any of these patterns: `claude-*`, `gpt-*`, `gemini-*` -- then warn (but still proceed):

```
Warning: `<value>` is not a recognized model alias or model ID.
Known aliases: claude-haiku, claude-sonnet, claude-opus, gpt-mini, gpt-standard, gpt-codex, gemini
Proceeding anyway.
```

**5. Smart write.** Compare the value being set against the same key in `DEFAULTS`.

If the new value equals the default value:

```
`<key>` is already at the default value (`<value>`). No change needed.
```

Skip writing this key.

If the value differs: proceed to write.

**6. Read the current target config file.** Use the Read tool to read `.nuff/config.json`. If the file does not exist, start with `{}`.

**7. Set the key in the target config object.** Navigate the dot-notation path and set the value, creating intermediate objects as needed. Do NOT replace the entire file -- only update the specific key path.

**8. Write the updated config back** using the Write tool. Use 2-space JSON indentation. Never include `_`-prefixed metadata keys in the written file. Always ensure `schema_version` is set to `DEFAULTS._schema_version` in the written file.

**After all key=value pairs are processed**, re-compute and display the updated config with provenance (repeat Steps 3 and 5).

## Step 8: Interactive Wizard

**If this is first-time setup** (config did not exist before Step 2 created it): Skip the section menu and walk the user through ALL sections in sequence — Models, then Behavior, then ADO — without asking which section to configure. Print a header before starting:

```
Welcome to nuff! Let's configure your settings. You'll be asked about models, behavior, and ADO integration. Press Enter to keep the default value for any setting.
```

After all sections are complete, jump directly to the "Done" handling below.

**Otherwise** (existing config, re-running wizard): Ask the user (consult the `ask-user` skill) to choose a section:

- **Models** -- AI model selection per subagent and phase
- **Behavior** -- Framework behavior toggles (auto_commit, verbose, branch_prefix)
- **ADO** -- Azure DevOps project defaults
- **Done** -- Exit configuration

**If choice is Done:** Before exiting, ensure the written `.nuff/config.json` contains `"schema_version": "<DEFAULTS._schema_version>"` at the top level. Read the file, add/update the field if missing, write back. Then display: "Configuration saved. Run `configure.nuff --show` to review the current settings."

**For Models:**

Display the current model values with provenance, then ask the user for each model setting (consult the `ask-user` skill for each):

```
Available aliases: claude-haiku (Haiku -- fast execution), claude-sonnet (Sonnet -- balanced), claude-opus (Opus -- best results), gpt-mini, gpt-standard, gpt-codex, gemini
You may also enter a raw model ID (e.g., claude-opus-4-6). Unrecognized values will generate a warning but are accepted.
```

Present settings grouped by phase, with the subagent as secondary context:

**Refine phase:**

- `models.gap_analyzer.refine` (gap analyzer during requirements clarification)

**Plan phase:**

- `models.codebase_analyzer.plan` (codebase analyzer during planning)
- `models.gap_analyzer.plan` (gap analyzer during plan review)

**Code phase:**

- `models.implementer` (implementer for task execution)
- `models.gap_analyzer.code` (gap analyzer for per-task checks)
- `models.verifier` (verifier for bulk verification)

For each setting:

- Show current value and source
- Present the alias options (claude-haiku, claude-sonnet, claude-opus, gpt-mini, gpt-standard, gpt-codex, gemini) plus "Keep current" as default

For each value the user provides:

- Empty input or same as current: no change.
- Apply model alias validation (Step 7, point 4).
- Apply smart write (Step 7, points 5-8).

**For Behavior:**

For each of `behavior.auto_commit`, `behavior.verbose`, `behavior.branch_prefix`, `behavior.visual_verification`:

- Show current value and source
- Ask the user (consult the `ask-user` skill) with appropriate options (true/false for booleans, freeform for branch_prefix)
- For `behavior.visual_verification`: describe it as "Run the app locally and capture screenshots during verification (requires Playwright MCP, Playwright CLI, or similar tooling)"

For each value:

- Apply type coercion (boolean fields: auto_commit, verbose, visual_verification).
- Apply smart write.

**For ADO:**

Display the ADO security warning first:

```
Note: Only set the project name here. NEVER enter PAT tokens or API keys --
authentication is handled externally via MCP server config or environment variables.
```

For `ado.project`:

- Show current value and source
- Ask the user (consult the `ask-user` skill) for the values

For each value:

- Apply smart write.

**After saving any section** (in the section-menu flow), re-read `.nuff/config.json` as `REPO_CONFIG`, re-compute provenance (Steps 3 and 5), and display the updated table. Then ask the user (consult the `ask-user` skill) if they want to configure another section. Loop back to the section menu. Continue until the user selects Done.

**In first-time setup flow**, after all three sections are complete, do NOT show the section menu — proceed directly to the Done handling (write `schema_version`, display "Configuration saved." message).

## Error Handling Summary

| Situation                                          | Action                                                   |
| -------------------------------------------------- | -------------------------------------------------------- |
| `nuff-defaults.json` missing from plugin directory | STOP with error message                                  |
| Config file does not exist                         | Create from defaults, run full wizard (first-time setup) |
| Config file contains invalid JSON                  | Warn, offer reset or abort (ask the user)                |
| Unknown config key (not in defaults)               | Warn and skip                                            |
| Invalid boolean string                             | Warn and skip                                            |
| Unrecognized model alias or ID                     | Warn but proceed                                         |
| Repo config schema violation                       | Warn, offer interactive fix or ignore                    |
