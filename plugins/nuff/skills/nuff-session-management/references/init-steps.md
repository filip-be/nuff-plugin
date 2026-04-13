# Session Initialization — Detailed Steps

## Step 1: Detect Work Item Context

Determine the work item using these methods in priority order. Stop at the first method that produces a result.

### Parse Arguments

First, check if `$ARGUMENTS` contains a `--phase` flag. If so, extract the phase value (must be one of: refine, plan, code, improve) and store it as the **phase override**. Remove the `--phase <value>` portion from arguments before proceeding.

### Method 1: Explicit Argument

If `$ARGUMENTS` (after removing any `--phase` flag) is provided and non-empty, parse it as follows:

**1. Check for a leading work item reference** (numeric ID or ADO URL) at the start of the string:

- **Leading numeric ID** (e.g., `12345` or `12345 some instructions`): Extract the numeric ID as `work_item_id`. Derive item name as `wi-12345`. Any remaining text after the ID is the `user_instruction`.
- **Leading ADO URL** (starts with or contains "dev.azure.com"): Extract the URL as `work_item_url`, parse the numeric ID from its path. Derive item name as `wi-<id>`. Any remaining text after the URL is the `user_instruction`.
- **Plain text string** (no leading numeric ID or URL): Treat the entire string as a `user_instruction`. Derive the item name by converting the first 5-7 words to lowercase kebab-case (replace spaces with hyphens, strip non-alphanumeric characters except hyphens). No `work_item_id`.

In all cases, trim whitespace from `user_instruction`. If it is empty after trimming, set `user_instruction = null`.

### Method 2: Branch Name

Run:

```bash
git branch --show-current
```

Attempt to extract the item name from the branch name using these patterns:

1. **Simple match**: Branch is exactly `nuff-<item-name>` -- extract `<item-name>`.
2. **Prefix match**: Branch has a prefix before `nuff-`, such as `copilot/nuff-<item-name>` or `user/nuff-<item-name>` -- extract `<item-name>` from after `nuff-`.
3. **Suffix handling**: Branch has a random suffix added by Copilot, such as `nuff-<item-name>-gHd123` or `copilot/nuff-<item-name>-xYz789`. To resolve:
   - List existing directories in `.nuff/` using the Glob tool (pattern: `.nuff/*/STATE.md`).
   - Match the branch's `nuff-` segment against existing directory names.
   - If exactly one directory matches as a prefix of the branch segment, use that directory name as the item name.
   - If no existing directories match, take the full segment after `nuff-` as the item name.

The general regex pattern to extract the nuff segment is: match `nuff-([a-z0-9][a-z0-9-]*)` anywhere in the branch name. Then attempt fuzzy matching against existing `.nuff/*/` directories to strip random suffixes.

### Method 3: Existing STATE.md

If neither Method 1 nor Method 2 produced a result:

1. Search for active work items by looking for `.nuff/*/STATE.md` files using the Glob tool.
2. Read each STATE.md and check the YAML frontmatter `phase:` field.
3. Filter to items where `phase` is NOT `complete`.

Then:

- **If exactly one active item found**: Use that item name.
- **If multiple active items found**: Ask the user (consult the `ask-user` skill) to choose from the list, showing their current phase and last updated timestamp.
- **If no active items found**: Ask the user: "No active work item found. Please provide a work item ID, ADO link, or item name to get started."

## Step 1.5: Validate Configuration

### 1.5.1: Check for config file

Check if `.nuff/config.json` (repo-level config) exists and parses as valid JSON. **Important:** This file is gitignored, so `Glob` will not find it. Use the `Read` tool directly on `.nuff/config.json` to check existence — if the file does not exist, `Read` will return an error.

### 1.5.2: Route to configuration if missing

If the config file does not exist:

Print: `No nuff configuration found. Configuration is required before proceeding.`

Then ask the user (consult the `ask-user` skill):

- **Configure now** -- Run the configuration wizard to set up models and preferences
- **Use defaults** -- Copy built-in defaults to `.nuff/config.json` and proceed

If **Configure now**: Follow the `/configure.nuff` command instructions. After configuration completes, print `Configuration complete. Resuming workflow...` and continue.

If **Use defaults**:

1. Read `nuff-defaults.json` from the plugin directory.
2. Create `.nuff/` directory if it does not exist.
3. Write a new `.nuff/config.json` containing only the non-`_`-prefixed keys from defaults, plus a top-level `"schema_version"` field set to the value of `_schema_version` from defaults.
4. Ensure `.nuff/.gitignore` exists and contains `config.json` (create or append as needed).
5. Print `Default configuration written to .nuff/config.json. Run /configure.nuff to customize.`

### 1.5.3: Load config for phase routing

Load `nuff-defaults.json` from the plugin directory as `DEFAULTS` and `.nuff/config.json` from CWD as `CONFIG`.

**Schema version check:** Compare `CONFIG.schema_version` against `DEFAULTS._schema_version`:

- If they match: proceed to alias resolution.
- If they differ (or `schema_version` is missing):
  1. Print: `Config schema version mismatch (config: <value>, current: <defaults_value>). Checking for differences...`
  2. Perform structural diff: collect missing keys, type mismatches, unknown keys.
  3. Display differences if found.
  4. Ask user: **Fix automatically** or **Skip**.
  5. If fix: apply changes, update `schema_version`, write back.

**Alias resolution:** Resolve model alias strings in `CONFIG.models` to actual model IDs using `DEFAULTS._aliases`. Store as `CONFIG_MODELS`.

### 1.5.4: Detect platform and set subagent spawning method

Detect the current platform by checking which tools are available:

- If the `Agent` tool is available with a `model` parameter (Claude Code): set `PLATFORM=claude-code`
- If `runSubagent` (VS Code Copilot Chat) or `task` (Copilot CLI) is available: set `PLATFORM=copilot`

**CRITICAL — Subagent model routing rule:** On Copilot, NEVER call `runSubagent`/`task` with agent names like `nuff-implementer` — those are NOT registered. ALWAYS use `@nuff-copilot-subagent-<alias>` wrappers. Consult the `nuff-subagent-execution` skill for the full mapping.

## Step 2: Initialize or Read STATE.md

### If `.nuff/<item-name>/STATE.md` exists

Read it. Extract the `phase` field from YAML frontmatter. Consult `nuff-state-update` with `update-frontmatter` to refresh the `updated` timestamp.

### If `.nuff/<item-name>/STATE.md` does NOT exist

Consult `nuff-state-update` with `initialize-state` to create it using the canonical initial template.

### Auto-create branch (if applicable)

**Before committing STATE.md**, if a work item ID/link was provided AND the current branch does NOT contain `nuff-`:

```bash
git checkout -b nuff-<item-name>
```

### Commit STATE.md

```bash
git add .nuff/<item-name>/STATE.md
git commit -m "nuff(<item-name>): initialize"
```

After committing, validate per [git-validation.md](./git-validation.md).

### Session Log

Consult `nuff-state-update` with `append-session-log` for `session-start | via do.nuff`.

## Step 2b: Detect ADO Process Template

Runs ONLY when both conditions are true:
1. ADO is configured (`CONFIG.ado.project` non-empty)
2. STATE.md does NOT have `ado_process_template` in frontmatter

If either is false, skip (log `skip | detect-process-template` with reason).

Consult `nuff-ado-orchestration` with `detect-process-template`.
