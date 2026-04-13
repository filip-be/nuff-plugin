---
name: nuff-self-improvement
description: Capture user corrections, reminders, and "do it differently" feedback as structured improvement entries. Activate whenever the user says something like "remember that...", "don't do it that way", "always use...", "you should have...", or any phrase that signals the agent missed something or should behave differently.
---

# Nuff Self-Improvement

This skill captures user feedback — corrections, reminders, and behavioral preferences — and writes them as structured entries to `.nuff/<item-name>/IMPROVEMENTS.md` so they can be reviewed in future phases and improve agent behaviour over time.

## When to Activate

Activate this skill when the user's message contains signals such as:

| Signal phrase / pattern  | Examples                                                                 |
| ------------------------ | ------------------------------------------------------------------------ |
| Explicit reminder        | "remember that…", "always remember…", "keep in mind…"                    |
| Correction               | "don't do it that way", "you shouldn't have…", "that was wrong because…" |
| Behavioral directive     | "always use X", "never do Y", "when Z happens, do W instead"             |
| Preference statement     | "I prefer…", "we always do it this way…", "in this project we…"          |
| Implicit dissatisfaction | "why did you…?", "that's not what I meant", "next time…"                 |

If in doubt, activate the skill. A false positive (recording something that didn't need recording) is less harmful than missing a genuine improvement signal.

## Inputs

- **`ITEM_NAME`** — the current work item name (e.g., `wi-12345` or `add-login-page`). Read from STATE.md frontmatter (`item_name`) or derive from context.
- **`USER_MESSAGE`** — the verbatim correction/reminder provided by the user.
- **`CONTEXT`** — optional: the action the agent took that triggered the feedback (what was produced / what the agent did).

## Output

Append a new entry to `.nuff/<item-name>/IMPROVEMENTS.md`. Create the file if it does not exist.

## Entry Format

Each entry is appended to the file as a Markdown section:

```markdown
## <ISO-8601 UTC datetime> — <short-title>

**What was missed / done incorrectly:**
<Concise description of what the agent did wrong or failed to consider. One to three sentences.>

**What the user expects instead:**
<Exact behaviour, rule, command, or approach the user stated. Quote the user's wording verbatim where possible.>

**Context / trigger:**
<The action or output that prompted this feedback, e.g., "agent ran `dotnet ef` without `--no-build` flag during code phase".>

**Category:** `<one of: command | workflow | style | tool-use | content | other>`
```

### Categories

| Category   | Use when…                                                                |
| ---------- | ------------------------------------------------------------------------ |
| `command`  | The user corrects a specific CLI command or flag                         |
| `workflow` | The user corrects the order of steps or a phase behaviour                |
| `style`    | The user corrects naming, formatting, or coding style choices            |
| `tool-use` | The user corrects which tool was used or how it was invoked              |
| `content`  | The user corrects the content of a generated artefact (spec, plan, code) |
| `other`    | Doesn't fit the above categories                                         |

## Step-by-Step Instructions

### Step 1: Detect the improvement signal

Read the user's message. Identify:

- What the agent did (or was about to do) that the user is correcting.
- What the user wants the agent to do instead (or always remember).
- The category that best fits.

### Step 2: Resolve ITEM_NAME

1. Check if a STATE.md is loaded in context. If yes, read `item_name` from its frontmatter.
2. If not, check the current git branch using `git branch --show-current` and extract the item name using the same patterns as `do.nuff` Step 1 Method 2.
3. If still unresolvable, use `general` as ITEM_NAME (entries land in `.nuff/general/IMPROVEMENTS.md`).

### Step 3: Compose the entry

Fill in all fields of the entry format using information from the user's message and the current session context. Be specific and actionable — future agent instances will read these entries to avoid repeating the same mistakes.

- **Short title**: 5–8 words summarising the improvement. Use present-tense imperative style, e.g., "Always pass `--no-build` to ef commands".
- **What was missed**: Describe the agent's actual behaviour or omission concisely.
- **What the user expects instead**: Paraphrase or quote the user's instruction. Prefer verbatim quotes.
- **Context / trigger**: Name the specific action, phase, command, or artefact that caused the feedback.
- **Category**: Pick the most specific matching category.

### Step 4: Write the entry

Append the entry to `.nuff/<item-name>/IMPROVEMENTS.md`. Create the file (and any parent directories) if they do not already exist.

File header (only written once, when creating a new file):

```markdown
# Improvements

This file records agent behaviour corrections and user reminders captured during nuff sessions. Each entry describes what was missed and what should be done differently in the future.

---
```

### Step 5: Acknowledge the user

After writing:

- Confirm the improvement was recorded with a brief message, e.g.:  
  `Noted and recorded in .nuff/<item-name>/IMPROVEMENTS.md.`
- **Do NOT re-ask** whether to record it — the user's message was already the instruction.
- **Do NOT pause the workflow** — if this skill was activated mid-task, resume immediately after acknowledging.

## Example

Given user message: _"Hey, remember that when restoring the database you must always use `--no-interactive` flag, never use the interactive mode"_

The skill writes the following entry to `.nuff/wi-4821/IMPROVEMENTS.md`:

```markdown
## 2026-03-13T14:22:00Z — Always pass `--no-interactive` to database restore

**What was missed / done incorrectly:**
Agent ran the database restore command without the `--no-interactive` flag, causing the command to block waiting for user input.

**What the user expects instead:**
"when restoring the database you must always use `--no-interactive` flag, never use the interactive mode"

**Context / trigger:**
Agent executed `db restore` during the code phase without `--no-interactive`.

**Category:** `command`
```
