---
name: pr-description
description: Write a pull request description as a short opener with the full agent-written text collapsed below it. Use every time you are about to create a pull request or edit a pull request's description.
---

# PR description

Agent-written pull request descriptions are accurate but long, so reviewers skip them. Keep the full text, fold it away, and put a short statement of what the change is for on top.

## Terms

- **Opener** — the one to three sentences at the top of the description that say what the change is for. Once posted it belongs to the user.
- **Collapsed description** — your full description, folded into a `<details>` block whose `<summary>` names the model that wrote it.

## Layout

Top to bottom:

1. The **Opener**: one paragraph of one to three sentences. No headings, no lists.
2. Issue-closing keywords (e.g. `Closes #12`), when the change closes an issue.
3. The **Collapsed description**: everything else you would normally write, including screenshots and your attribution footer.

```markdown
The row page showed its single row as a one-row table. It now shows the row as a list of labelled fields.

Closes #12

<details>
<summary>Claude Opus 5.5 Medium</summary>

## Summary

- ...

## Test plan

- ...

🤖 Generated with ...

</details>
```

- The `<summary>` is your own model's name, followed by your reasoning effort level when you know it. Leave the effort out when you don't.
- Keep the blank line after `</summary>` and the one before `</details>`; without them the Markdown inside the block does not render.

## The Opener

Take it from the first source that exists:

1. Description text the user explicitly gave you — use it word for word.
2. The user's original request and intent in this session.
3. The plan or issue the work is based on (e.g. `.agents/PLAN.md`).
4. The commit messages and the diff.

Say what the change is for, not how it was made. Do not ask the user to approve the Opener: create the pull request right away, then show the Opener in your report and name the source it came from.

## Pull request templates

If the repository has a pull request template (e.g. `.github/PULL_REQUEST_TEMPLATE.md`), the template wins. Fill in its sections and checkboxes and leave them visible, put the Opener above them, and move only your long prose into the Collapsed description.

## Editing an existing description

The Collapsed description is the top-level `<details>` block whose `<summary>` names a model. Regenerate that block and nothing else.

- **Everything outside it is frozen.** That covers the Opener, text the user added, other `<details>` blocks, and template sections and checkboxes — even ones you filled in when the pull request was created. When a frozen part no longer matches the change, leave it as it is and tell the user it looks stale, with a suggested replacement.
- **No such block.** Treat the existing text as the user's: keep it untouched, append the Collapsed description below it, add no Opener, and say so in your report.
- **No such block, and the existing text ends with an agent attribution footer.** It is agent-written: write an Opener and replace the old text with a fresh Collapsed description.
