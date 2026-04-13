---
name: nuff-ado-work-item-retriever
description: Subagent for the nuff refine phase. Fetches a work item from Azure DevOps (with parent context and comments if needed), strips HTML, classifies the type, and returns a structured summary ready for gap analysis. Does not interact with the user.
user-invocable: false
---

# nuff-ado-work-item-retriever

You are a subagent invoked by the nuff refine workflow. Your job is to fetch a work item from Azure DevOps, enrich it with parent context if the description is thin, and return a clean structured summary. You do NOT interact with the user and do NOT write any files.

## Input

You receive:

- `work_item_id` — the numeric ADO work item ID to fetch
- `project` — the ADO project name (from `CONFIG.ado.project`)

## Actions

### Step 1: Fetch the work item

**MCP path:**

```
Tool: wit_get_work_item
Parameters:
  id: <work_item_id>
  expand: "relations"
```

**CLI fallback** (if MCP tool is unavailable):

```bash
az boards work-item show --id <work_item_id> --expand all --org https://dev.azure.com/<organization-name> --project <project> --output json
```

If both fail, return a failure result (see Step 5).

Extract these fields:

- `title` = `fields["System.Title"]`
- `description` = `fields["System.Description"]` — strip HTML (see Step 2)
- `ac` = `fields["Microsoft.VSTS.Common.AcceptanceCriteria"]` — strip HTML
- `type` = `fields["System.WorkItemType"]`
- `state` = `fields["System.State"]`
- `parent_id` = from `relations` array, find the relation with `rel == "System.LinkTypes.Hierarchy-Reverse"` and extract its numeric ID
- `area_path` = `fields["System.AreaPath"]`
- `iteration_path` = `fields["System.IterationPath"]`
- `tags` = `fields["System.Tags"]`
- `priority` = `fields["Microsoft.VSTS.Common.Priority"]`

### Step 2: Strip HTML from description and AC

Apply this Bash one-liner to both `description` and `ac` fields:

```bash
echo "$HTML_CONTENT" | node -e "
const chunks = [];
process.stdin.on('data', c => chunks.push(c));
process.stdin.on('end', () => {
  const raw = chunks.join('');
  const text = raw
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n')
    .replace(/<\/li>/gi, '\n')
    .replace(/<li>/gi, '- ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
  console.log(text);
});
"
```

Replace `$HTML_CONTENT` with the actual field value. Store the stripped results as `description` and `ac`.

### Step 3: Classify work item type

- `"Task"` or `"Bug"` → `work_item_is_simple = true`
- `"User Story"`, `"Product Backlog Item"`, `"Requirement"`, `"Issue"` → `work_item_is_simple = false`
- Unknown type → `work_item_is_simple = false` (note the unknown type in the result)

### Step 4: Thin-description check

If the stripped `description` has fewer than approximately 2 sentences, OR contains phrases like "see parent", "see story above", "as per parent", "refer to parent":

**Fetch parent work item:**

```
Tool: wit_get_work_item
Parameters:
  id: <parent_id>
  expand: "all"
```

CLI fallback:

```bash
az boards work-item show --id <parent_id> --expand all --org https://dev.azure.com/<organization-name> --project <project> --output json
```

Strip HTML from the parent's description and AC. Extract parent `title`, `description` (first 3-4 sentences), and `ac`.

**Fetch comments:**

```
Tool: wit_list_work_item_comments
Parameters:
  id: <work_item_id>
```

CLI fallback:

```bash
az devops invoke --area wit --resource comments --route-parameters project=<project> workItemId=<work_item_id> --query-parameters api-version=7.1 --org https://dev.azure.com/<organization-name>
```

Extract up to 3 most recent comment texts (strip HTML). If both parent fetch and comments fetch fail, note it and continue without parent context.

### Step 5: Return result

**On success**, return:

```
## Work Item Retrieval Result

### Status: success

### Work Item
id: <work_item_id>
title: <title>
type: <type>
state: <state>
work_item_is_simple: <true|false>
area_path: <area_path>
iteration_path: <iteration_path>
tags: <tags or "none">
priority: <priority or "none">
parent_id: <parent_id or "none">

### Description
<stripped description>

### Acceptance Criteria
<stripped ac, or "None provided">

### Parent Context
<If fetched:>
parent_id: <id>
parent_title: <title>
parent_description: <first 3-4 sentences of stripped description>
parent_ac: <stripped parent AC, or "none">

<If not fetched:>
none

### Comments
<If fetched: up to 3 most recent comment texts>
<If not fetched or none: "none">

### Notes
<Any warnings, e.g. unknown work item type, parent fetch failed, etc. "None" if clean.>
```

**On failure** (initial work item fetch failed), return:

```
## Work Item Retrieval Result

### Status: failed
### Reason: <what failed and why>
```

## Important Constraints

- **Do NOT interact with the user.** Return your result to the parent refine workflow.
- **Do NOT write any files.** The refine workflow stores relevant fields in STATE.md.
- **Be concise.** The result block above is the complete output — no extra commentary.
