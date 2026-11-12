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
2. Closing keywords, when the change closes an issue or a work item (e.g. `Closes #12` for a GitHub issue, `Fixes AB#12` for an Azure DevOps work item).
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

## Screenshots

A screenshot shows a reviewer in seconds what paragraphs of text cannot. **Whenever the change has a visible result, try to add screenshots when you create the pull request** — do not wait to be asked.

- A visible result is anything a person can look at: a page, a dialog, a chart, a rendered document, terminal output.
- If you can run the changed thing and capture it, do so. Show the changed part, and add a before/after pair when the difference is the point.
- For terminal output, paste the output as a code block instead of an image.
- Put screenshots at the top of the Collapsed description, each with one line saying what it shows.
- Never commit the screenshot files. Save them outside the repository, or leave them untracked.
- Do not upload a screenshot that shows secrets or personal data. Tell the user where the file is instead.
- Never hold back the pull request for a screenshot. If you could not capture one, or cannot upload an image to the pull request, create it without and tell the user in your report which screenshots are worth adding, and where the files are if you captured any.

### Uploading on GitHub

Plain `git` cannot attach an image, but the GitHub CLI can. Write each screenshot into the description as an ordinary Markdown image with a local path, and pass the same file with `--attach`. `gh` uploads the file and replaces the path with the uploaded link, wherever the image sits in the description.

In the description, at the top of the Collapsed description:

```markdown
The row page after the change:

![Row shown as a list of labelled fields](./after.png)
```

The command:

```bash
gh pr create --title "..." --body-file body.md --attach ./after.png
```

- Repeat `--attach` for each file. `gh pr edit <number>` takes the same flag.
- A path in the body file is relative to that file's folder.
- Pass `--attach` without checking first whether your `gh` supports it. What you do when the command fails depends on whether the pull request exists:
  - **Not created (or not updated)**, whatever the reason — unknown flag, no push access, rejected file: remove the image references from the description and run the command again without `--attach`.
  - **Created (or updated)** — `gh` printed its URL but exited with an error, because some uploads failed: do not run the command again. Edit the description to remove any image reference that still has a local path.
- With no `gh` at all, create the pull request without the images.
- In each of these cases, report which screenshots are missing, where the files are and the error `gh` gave. When the error was an unknown `--attach` flag, or `gh` is missing, add that a current GitHub CLI would let you attach them.

## Pull request templates

If the repository has a pull request template (e.g. `.github/PULL_REQUEST_TEMPLATE.md`), the template wins. Fill in its sections and checkboxes and leave them visible, put the Opener above them, and move only your long prose into the Collapsed description.

## Editing an existing description

The Collapsed description is the top-level `<details>` block whose `<summary>` names a model. Regenerate that block and nothing else.

- **Everything outside it is frozen.** That covers the Opener, text the user added, other `<details>` blocks, and template sections and checkboxes — even ones you filled in when the pull request was created. When a frozen part no longer matches the change, leave it as it is and tell the user it looks stale, with a suggested replacement.
- **Screenshots already in the block stay.** Carry their uploaded image links over as they are. Capture and attach new screenshots only when the visible result has changed since.
- **No such block.** Treat the existing text as the user's: keep it untouched, append the Collapsed description below it, add no Opener, and say so in your report.
- **No such block, and the existing text ends with an agent attribution footer.** It is agent-written: write an Opener and replace the old text with a fresh Collapsed description.
